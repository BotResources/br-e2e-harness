use std::path::PathBuf;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use br_test_harness::run_once;
use br_util_nats_fabric::KvKey;
use uuid::Uuid;

use crate::error::{ConformanceError, Result};
use crate::harness::PassportHarness;

pub const BEARER_SEAL_KEY_LEN: usize = 32;

pub const SEAL_KEY: [u8; BEARER_SEAL_KEY_LEN] = [
    0x1f, 0x2e, 0x3d, 0x4c, 0x5b, 0x6a, 0x79, 0x88, 0x97, 0xa6, 0xb5, 0xc4, 0xd3, 0xe2, 0xf1, 0x00,
    0x0f, 0x1e, 0x2d, 0x3c, 0x4b, 0x5a, 0x69, 0x78, 0x87, 0x96, 0xa5, 0xb4, 0xc3, 0xd2, 0xe1, 0xf0,
];

pub const WRONG_SEAL_KEY: [u8; BEARER_SEAL_KEY_LEN] = [0x99; BEARER_SEAL_KEY_LEN];

const SEAL_TIMEOUT: Duration = Duration::from_secs(30);

pub fn seal_key_b64() -> String {
    STANDARD.encode(SEAL_KEY)
}

pub fn wrong_seal_key_b64() -> String {
    STANDARD.encode(WRONG_SEAL_KEY)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealVariant {
    Faithful,
    TamperedCiphertext,
    Unreadable,
}

impl SealVariant {
    fn anchor_flags(self) -> &'static [&'static str] {
        match self {
            SealVariant::Faithful => &[],
            SealVariant::TamperedCiphertext => &["--tamper", "ciphertext"],
            SealVariant::Unreadable => &["--unreadable"],
        }
    }
}

#[derive(Debug, Clone)]
pub struct SealedSeed {
    pub raw: String,
    pub user_id: Uuid,
    pub token_id: Uuid,
    pub kv_key: KvKey,
}

pub struct SealedSeeder {
    anchor: PathBuf,
    key_b64: String,
}

impl SealedSeeder {
    pub fn new(anchor: impl Into<PathBuf>, key_b64: String) -> Self {
        Self {
            anchor: anchor.into(),
            key_b64,
        }
    }

    pub async fn seed(
        &self,
        harness: &PassportHarness,
        namespace: &str,
        label: &str,
    ) -> Result<SealedSeed> {
        let raw = format!("brk_{label}_{namespace}_{}", Uuid::now_v7().simple());
        let user_id = Uuid::now_v7();
        let token_id = Uuid::now_v7();
        let (kv_key, value) = self
            .render(&raw, user_id, token_id, SealVariant::Faithful)
            .await?;
        harness.pl_put_raw(&kv_key, &value).await;
        Ok(SealedSeed {
            raw,
            user_id,
            token_id,
            kv_key,
        })
    }

    pub async fn overwrite(
        &self,
        harness: &PassportHarness,
        seed: &SealedSeed,
        variant: SealVariant,
    ) -> Result<()> {
        let (kv_key, value) = self
            .render(&seed.raw, seed.user_id, seed.token_id, variant)
            .await?;
        if kv_key != seed.kv_key {
            return Err(ConformanceError::Seed(format!(
                "the anchor placed the {variant:?} variant at {} instead of the seeded key {}",
                kv_key.as_str(),
                seed.kv_key.as_str()
            )));
        }
        harness.pl_put_raw(&kv_key, &value).await;
        Ok(())
    }

    pub async fn revoke(&self, harness: &PassportHarness, seed: &SealedSeed) -> Result<()> {
        harness.pl_retract(&seed.kv_key).await
    }

    async fn render(
        &self,
        raw: &str,
        user_id: Uuid,
        token_id: Uuid,
        variant: SealVariant,
    ) -> Result<(KvKey, Vec<u8>)> {
        let actor = format!("human:{user_id}");
        let token_id = token_id.to_string();
        let mut args = vec![
            "seal",
            "--key",
            &self.key_b64,
            "--token",
            raw,
            "--actor",
            &actor,
            "--token-id",
            &token_id,
        ];
        args.extend_from_slice(variant.anchor_flags());

        let output = run_once(&self.anchor.to_string_lossy(), &args, &[], SEAL_TIMEOUT)
            .await
            .map_err(ConformanceError::Seed)?;
        if !output.status.success() {
            return Err(ConformanceError::Seed(format!(
                "the anchor rejected the {variant:?} seal (status {}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }

        let (kv_key, value) = parse_anchor_line(&output.stdout)?;
        assert_kv_key_carries_the_lib_digest(&kv_key, raw)?;
        Ok((kv_key, value))
    }
}

fn parse_anchor_line(stdout: &[u8]) -> Result<(KvKey, Vec<u8>)> {
    let text = std::str::from_utf8(stdout)
        .map_err(|e| ConformanceError::Seed(format!("the anchor emitted non-utf8 stdout: {e}")))?;
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let line = lines
        .next()
        .ok_or_else(|| ConformanceError::Seed("the anchor emitted no seal line".to_string()))?;
    if lines.next().is_some() {
        return Err(ConformanceError::Seed(format!(
            "the anchor must emit exactly one seal line, got:\n{text}"
        )));
    }

    let parsed: serde_json::Value = serde_json::from_str(line)
        .map_err(|e| ConformanceError::Seed(format!("the seal line is not JSON: {e} ({line})")))?;
    let kv_key = field(&parsed, "kv_key")?;
    let value_b64 = field(&parsed, "value_b64")?;
    let kv_key = KvKey::new(kv_key)
        .map_err(|e| ConformanceError::Seed(format!("the anchor's kv key is unusable: {e}")))?;
    let value = STANDARD
        .decode(value_b64)
        .map_err(|e| ConformanceError::Seed(format!("value_b64 is not base64-std: {e}")))?;
    Ok((kv_key, value))
}

fn field(parsed: &serde_json::Value, name: &str) -> Result<String> {
    parsed
        .get(name)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ConformanceError::Seed(format!("the seal line has no string {name:?}")))
}

fn assert_kv_key_carries_the_lib_digest(kv_key: &KvKey, raw: &str) -> Result<()> {
    let digest = br_core_auth::bearer_token_key(raw);
    match kv_key.as_str().strip_suffix(digest.as_str()) {
        Some(prefix) if !prefix.is_empty() => Ok(()),
        _ => Err(ConformanceError::Seed(format!(
            "the anchor's kv key {} is not a prefix followed by br_core_auth::bearer_token_key = {digest}",
            kv_key.as_str()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_key_is_thirty_two_bytes_and_base64_round_trips() {
        let decoded = STANDARD.decode(seal_key_b64()).expect("base64-std decodes");
        assert_eq!(decoded, SEAL_KEY.to_vec());
        assert_eq!(decoded.len(), BEARER_SEAL_KEY_LEN);
    }

    #[test]
    fn the_wrong_key_differs_from_the_correct_one() {
        assert_ne!(SEAL_KEY, WRONG_SEAL_KEY);
        assert_ne!(seal_key_b64(), wrong_seal_key_b64());
    }

    #[test]
    fn each_variant_maps_to_its_own_anchor_flags() {
        assert!(SealVariant::Faithful.anchor_flags().is_empty());
        assert_eq!(
            SealVariant::TamperedCiphertext.anchor_flags(),
            &["--tamper", "ciphertext"]
        );
        assert_eq!(SealVariant::Unreadable.anchor_flags(), &["--unreadable"]);
    }

    #[test]
    fn the_anchor_line_is_parsed_into_the_key_and_the_exact_bytes() {
        let stdout = br#"{"kv_key":"identity/bearer_tokens/ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad","value_b64":"eyJhIjoxfQ=="}
"#;
        let (kv_key, value) = parse_anchor_line(stdout).expect("the anchor line parses");
        assert_eq!(
            kv_key.as_str(),
            "identity/bearer_tokens/ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(value, br#"{"a":1}"#.to_vec());
        assert_kv_key_carries_the_lib_digest(&kv_key, "abc")
            .expect("the frozen sha256 vector of \"abc\" is the lib digest");
    }

    #[test]
    fn a_kv_key_whose_digest_is_not_the_lib_digest_is_rejected() {
        let kv_key = KvKey::new("identity/bearer_tokens/deadbeef").expect("valid kv key");
        assert!(assert_kv_key_carries_the_lib_digest(&kv_key, "abc").is_err());
    }

    #[test]
    fn a_bare_digest_with_no_prefix_is_rejected() {
        let kv_key = KvKey::new(br_core_auth::bearer_token_key("abc")).expect("valid kv key");
        assert!(assert_kv_key_carries_the_lib_digest(&kv_key, "abc").is_err());
    }

    #[test]
    fn more_than_one_anchor_line_is_rejected() {
        let stdout =
            b"{\"kv_key\":\"k\",\"value_b64\":\"\"}\n{\"kv_key\":\"k\",\"value_b64\":\"\"}\n";
        assert!(parse_anchor_line(stdout).is_err());
    }

    #[test]
    fn an_empty_anchor_stdout_is_rejected() {
        assert!(parse_anchor_line(b"").is_err());
        assert!(parse_anchor_line(b"\n \n").is_err());
    }

    #[test]
    fn a_seal_line_missing_a_field_is_rejected() {
        assert!(parse_anchor_line(br#"{"kv_key":"identity/bearer_tokens/x"}"#).is_err());
        assert!(parse_anchor_line(br#"{"value_b64":"eyJhIjoxfQ=="}"#).is_err());
    }
}
