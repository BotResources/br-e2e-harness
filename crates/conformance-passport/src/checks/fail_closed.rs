use crate::endpoint::Resolution;
use crate::outcome::{CheckId, CheckOutcome};
use crate::vectors::Vector;

use super::CheckContext;

pub async fn run_wrong_seal_key(ctx: &CheckContext<'_>) -> CheckOutcome {
    let id = CheckId::WrongSealKeyFailsClosed;
    let expected =
        "a bearer sealed under the WRONG key resolves to anonymous (200), never a wrong identity";

    let seed = ctx.seed(Vector::WrongKey).await;
    if ctx.harness.pl_get_raw(&seed.kv_key).await.is_none() {
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

pub async fn run_tampered_ciphertext(ctx: &CheckContext<'_>) -> CheckOutcome {
    run_corrupted_envelope(
        ctx,
        CheckId::TamperedEnvelopeFailsClosed,
        "a bearer that resolved, then replaced at its own key by that exact envelope with byte 0 of its ciphertext flipped, resolves to anonymous (200)",
        Vector::TamperedCiphertextFaithful,
        Vector::TamperedCiphertextCorrupt,
        "the subject opened a flipped ciphertext — the AEAD tag must fail closed to anonymous",
    )
    .await
}

pub async fn run_tampered_nonce(ctx: &CheckContext<'_>) -> CheckOutcome {
    run_corrupted_envelope(
        ctx,
        CheckId::TamperedNonceFailsClosed,
        "a bearer that resolved, then replaced at its own key by that exact envelope with byte 0 of its nonce flipped, resolves to anonymous (200)",
        Vector::TamperedNonceFaithful,
        Vector::TamperedNonceCorrupt,
        "the subject opened an envelope whose nonce no longer matches the tag — the AEAD must fail closed to anonymous",
    )
    .await
}

pub async fn run_unreadable_envelope(ctx: &CheckContext<'_>) -> CheckOutcome {
    run_corrupted_envelope(
        ctx,
        CheckId::UnreadableEnvelopeFailsClosed,
        "a bearer that resolved, then replaced at its own key by that exact envelope plus one unknown field, resolves to anonymous (200)",
        Vector::UnreadableFaithful,
        Vector::UnreadableCorrupt,
        "the subject accepted an envelope carrying an unknown field — the parse must fail closed to anonymous",
    )
    .await
}

async fn run_corrupted_envelope(
    ctx: &CheckContext<'_>,
    id: CheckId,
    expected: &'static str,
    faithful: Vector,
    corrupt: Vector,
    on_resolved: &'static str,
) -> CheckOutcome {
    let seed = ctx.seed(faithful).await;
    match ctx.endpoint.resolve_bearer(&seed.raw).await {
        Ok(Resolution::Resolved(_)) => {}
        Ok(Resolution::Anonymous) => {
            return CheckOutcome::fail(
                id,
                expected,
                "the faithful vector already resolved to anonymous",
                "the declared mutation must be the only difference — a seed that never resolved proves nothing",
            );
        }
        Err(e) => {
            return CheckOutcome::fail(
                id,
                expected,
                "the pre-corruption endpoint call failed",
                format!("{e}"),
            );
        }
    }

    let corrupted = ctx.seed(corrupt).await;
    if corrupted.kv_key != seed.kv_key {
        return CheckOutcome::fail(
            id,
            expected,
            "the corrupted vector does not share the faithful vector's key",
            "the pair must overwrite one key, otherwise the two resolutions are unrelated",
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
        Resolution::Resolved(_) => {
            CheckOutcome::fail(id, expected, resolution.label(), on_resolved)
        }
    }
}
