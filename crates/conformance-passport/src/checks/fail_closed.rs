use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use br_auth_contract::{SealedBearer, bearer_token_kv_key};
use br_util_nats_fabric::KvKey;

use crate::endpoint::Resolution;
use crate::outcome::{CheckId, CheckOutcome};

use super::CheckContext;

pub async fn run_wrong_seal_key(ctx: &CheckContext<'_>) -> CheckOutcome {
    let id = CheckId::WrongSealKeyFailsClosed;
    let expected =
        "a bearer sealed under the WRONG key resolves to anonymous (200), never a wrong identity";

    let wrong_seeder = match ctx.harness.wrong_key_seeder().await {
        Ok(seeder) => seeder,
        Err(e) => {
            return CheckOutcome::fail(
                id,
                expected,
                "opening a wrong-key publisher failed",
                format!("{e}"),
            );
        }
    };
    let seed = match wrong_seeder.seed(ctx.namespace, "wrong_key").await {
        Ok(seed) => seed,
        Err(e) => {
            return CheckOutcome::fail(id, expected, "wrong-key seeding failed", format!("{e}"));
        }
    };

    let kv_key = match KvKey::new(bearer_token_kv_key(&seed.raw)) {
        Ok(key) => key,
        Err(e) => {
            return CheckOutcome::fail(id, expected, "building the kv key failed", format!("{e}"));
        }
    };
    if ctx.harness.pl_get_raw(&kv_key).await.is_none() {
        return CheckOutcome::fail(
            id,
            expected,
            "the wrong-key seed is absent from PUBLISHED_LANGUAGE",
            "cannot prove the anonymity came from an AEAD-open failure rather than a missing key — the seed must be present and found",
        );
    }

    let resolution = match ctx.endpoint.resolve_bearer(&seed.raw).await {
        Ok(resolution) => resolution,
        Err(e) => {
            return CheckOutcome::fail(id, expected, "the endpoint call failed", format!("{e}"));
        }
    };

    match resolution {
        Resolution::Anonymous => CheckOutcome::pass(id, expected, resolution.label()),
        Resolution::Resolved(_) => CheckOutcome::fail(
            id,
            expected,
            resolution.label(),
            "the subject opened an envelope sealed under a different key — AEAD must fail closed to anonymous",
        ),
    }
}

pub async fn run_tampered_envelope(ctx: &CheckContext<'_>) -> CheckOutcome {
    let id = CheckId::TamperedEnvelopeFailsClosed;
    let expected = "a correctly-sealed bearer whose stored ciphertext is then tampered resolves to anonymous (200)";

    let seed = match ctx.seeder.seed(ctx.namespace, "tampered").await {
        Ok(seed) => seed,
        Err(e) => return CheckOutcome::fail(id, expected, "seeding failed", format!("{e}")),
    };

    let kv_key = match KvKey::new(bearer_token_kv_key(&seed.raw)) {
        Ok(key) => key,
        Err(e) => {
            return CheckOutcome::fail(id, expected, "building the kv key failed", format!("{e}"));
        }
    };

    let stored = match ctx.harness.pl_get_raw(&kv_key).await {
        Some(bytes) => bytes,
        None => {
            return CheckOutcome::fail(
                id,
                expected,
                "the seeded sealed bearer was not found in PUBLISHED_LANGUAGE",
                "pl_get_raw returned None for a freshly seeded key",
            );
        }
    };

    let tampered = match tamper_ciphertext(&stored) {
        Ok(bytes) => bytes,
        Err(detail) => return CheckOutcome::fail(id, expected, "tampering failed", detail),
    };
    ctx.harness.pl_put_raw(&kv_key, &tampered).await;

    let resolution = match ctx.endpoint.resolve_bearer(&seed.raw).await {
        Ok(resolution) => resolution,
        Err(e) => {
            return CheckOutcome::fail(id, expected, "the endpoint call failed", format!("{e}"));
        }
    };

    match resolution {
        Resolution::Anonymous => CheckOutcome::pass(id, expected, resolution.label()),
        Resolution::Resolved(_) => CheckOutcome::fail(
            id,
            expected,
            resolution.label(),
            "the subject opened a tampered ciphertext — the AEAD tag must fail closed to anonymous",
        ),
    }
}

fn tamper_ciphertext(stored: &[u8]) -> std::result::Result<Vec<u8>, String> {
    let mut sealed: SealedBearer = serde_json::from_slice(stored)
        .map_err(|e| format!("stored value is not a SealedBearer: {e}"))?;
    let mut ciphertext = STANDARD
        .decode(&sealed.ciphertext)
        .map_err(|e| format!("stored ciphertext is not base64-std: {e}"))?;
    if ciphertext.is_empty() {
        return Err("stored ciphertext is empty; nothing to tamper".to_string());
    }
    ciphertext[0] ^= 0xff;
    sealed.ciphertext = STANDARD.encode(&ciphertext);
    serde_json::to_vec(&sealed)
        .map_err(|e| format!("re-encoding the tampered envelope failed: {e}"))
}
