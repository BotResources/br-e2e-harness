use std::sync::OnceLock;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use br_util_nats_fabric::KvKey;
use uuid::Uuid;

use super::catalogue::Vector;

pub(super) const FROZEN: &str = include_str!("../../vectors/passport-wire-v1.json");

const WIRE_VERSION: u64 = 1;

#[derive(Debug, Clone)]
pub struct WireVector {
    pub name: String,
    pub token: String,
    pub kv_key: KvKey,
    pub actor_id: Uuid,
    pub token_id: Uuid,
    pub value: Vec<u8>,
}

#[derive(Debug)]
pub struct FrozenWire {
    seal_key_b64: String,
    vectors: Vec<WireVector>,
}

impl FrozenWire {
    pub fn seal_key_b64(&self) -> &str {
        &self.seal_key_b64
    }

    pub fn get(&self, vector: Vector) -> &WireVector {
        self.vectors
            .iter()
            .find(|candidate| candidate.name == vector.name())
            .unwrap_or_else(|| {
                panic!(
                    "the frozen vector file has no entry named {:?}; regenerate it with `make -C conformance-subjects/identity-passport vectors`",
                    vector.name()
                )
            })
    }

    pub fn names(&self) -> Vec<&str> {
        self.vectors.iter().map(|v| v.name.as_str()).collect()
    }
}

pub fn frozen_wire() -> &'static FrozenWire {
    static PARSED: OnceLock<FrozenWire> = OnceLock::new();
    PARSED.get_or_init(|| {
        parse(FROZEN).unwrap_or_else(|detail| {
            panic!("the embedded passport wire vectors are unusable: {detail}")
        })
    })
}

pub fn seal_key_b64() -> String {
    frozen_wire().seal_key_b64().to_string()
}

pub(super) fn parse(raw: &str) -> std::result::Result<FrozenWire, String> {
    let root: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| format!("the vector file is not JSON: {e}"))?;
    match root.get("version").and_then(serde_json::Value::as_u64) {
        Some(WIRE_VERSION) => {}
        Some(other) => {
            return Err(format!(
                "the vector file declares wire version {other}, this battery reads {WIRE_VERSION}"
            ));
        }
        None => return Err("the vector file declares no integer \"version\"".to_string()),
    }
    let seal_key_b64 = string_at(&root, "seal_key_b64")?;
    let entries = root
        .get("vectors")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "the vector file has no \"vectors\" array".to_string())?;
    if entries.is_empty() {
        return Err("the vector file carries no vector".to_string());
    }

    let mut vectors = Vec::with_capacity(entries.len());
    for entry in entries {
        vectors.push(parse_vector(entry)?);
    }
    super::twins::assert_every_twin_is_its_faithful_plus_the_declared_mutation(&vectors)?;
    Ok(FrozenWire {
        seal_key_b64,
        vectors,
    })
}

fn parse_vector(entry: &serde_json::Value) -> std::result::Result<WireVector, String> {
    let name = string_at(entry, "name")?;
    let token = string_at(entry, "token")?;
    let kv_key = KvKey::new(string_at(entry, "kv_key")?)
        .map_err(|e| format!("vector {name}: unusable kv key: {e}"))?;
    assert_kv_key_carries_the_lib_digest(&kv_key, &token)
        .map_err(|detail| format!("vector {name}: {detail}"))?;
    let actor_id = uuid_at(entry, "actor_id").map_err(|e| format!("vector {name}: {e}"))?;
    let token_id = uuid_at(entry, "token_id").map_err(|e| format!("vector {name}: {e}"))?;
    let value = STANDARD
        .decode(string_at(entry, "value_b64")?)
        .map_err(|e| format!("vector {name}: value_b64 is not base64-std: {e}"))?;
    Ok(WireVector {
        name,
        token,
        kv_key,
        actor_id,
        token_id,
        value,
    })
}

fn string_at(value: &serde_json::Value, field: &str) -> std::result::Result<String, String> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("missing string field {field:?}"))
}

fn uuid_at(value: &serde_json::Value, field: &str) -> std::result::Result<Uuid, String> {
    string_at(value, field)?
        .parse()
        .map_err(|e| format!("{field} is not a uuid: {e}"))
}

fn assert_kv_key_carries_the_lib_digest(
    kv_key: &KvKey,
    token: &str,
) -> std::result::Result<(), String> {
    let digest = br_core_auth::bearer_token_key(token);
    match kv_key.as_str().strip_suffix(digest.as_str()) {
        Some(prefix) if !prefix.is_empty() => Ok(()),
        _ => Err(format!(
            "kv key {} is not a prefix followed by br_core_auth::bearer_token_key = {digest}",
            kv_key.as_str()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectors::catalogue::EVERY_VECTOR;

    #[test]
    fn a_vector_file_of_another_wire_version_is_rejected() {
        let other = FROZEN.replacen(
            "\"version\": 1",
            &format!("\"version\": {}", WIRE_VERSION + 1),
            1,
        );
        assert_ne!(other, FROZEN);
        let err = parse(&other).expect_err("a foreign wire version must be rejected");
        assert!(err.contains("wire version"), "{err}");
        assert!(parse(&FROZEN.replacen("\"version\": 1", "\"nope\": 1", 1)).is_err());
    }

    #[test]
    fn the_embedded_file_parses_and_every_kv_key_matches_the_lib_digest() {
        let wire = frozen_wire();
        assert!(!wire.names().is_empty());
        assert_eq!(
            STANDARD
                .decode(wire.seal_key_b64())
                .expect("the seal key is base64-std")
                .len(),
            32
        );
    }

    #[test]
    fn every_declared_vector_exists_in_the_frozen_file() {
        for vector in EVERY_VECTOR {
            assert_eq!(frozen_wire().get(vector).name, vector.name());
        }
    }

    #[test]
    fn every_frozen_entry_is_declared_in_rust() {
        let declared: Vec<&str> = EVERY_VECTOR.iter().map(|v| v.name()).collect();
        for name in frozen_wire().names() {
            assert!(
                declared.contains(&name),
                "the frozen file carries {name:?}, which no Vector variant names"
            );
        }
        assert_eq!(declared.len(), frozen_wire().names().len());
    }

    #[test]
    fn scenarios_needing_their_own_key_do_not_collide() {
        let distinct = [
            Vector::FaithfulHuman,
            Vector::FaithfulHumanSecond,
            Vector::FaithfulService,
            Vector::Revoked,
            Vector::KvError,
            Vector::WrongKey,
            Vector::TamperedCiphertextFaithful,
            Vector::TamperedNonceFaithful,
            Vector::UnreadableFaithful,
        ];
        let mut keys: Vec<&str> = distinct
            .iter()
            .map(|v| frozen_wire().get(*v).kv_key.as_str())
            .collect();
        keys.sort_unstable();
        let total = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), total, "two scenarios would share one kv key");
    }

    #[test]
    fn the_two_faithful_human_vectors_carry_distinct_identities() {
        let first = frozen_wire().get(Vector::FaithfulHuman);
        let second = frozen_wire().get(Vector::FaithfulHumanSecond);
        assert_ne!(first.actor_id, second.actor_id);
        assert_ne!(first.token_id, second.token_id);
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
    fn the_frozen_sha256_vector_of_abc_is_the_lib_digest() {
        let kv_key = KvKey::new(
            "identity/bearer_tokens/ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        )
        .expect("valid kv key");
        assert!(assert_kv_key_carries_the_lib_digest(&kv_key, "abc").is_ok());
    }
}
