use crate::endpoint::Resolution;
use crate::outcome::{CheckId, CheckOutcome};
use crate::seal::SealVariant;

use super::CheckContext;

pub async fn run_wrong_seal_key(ctx: &CheckContext<'_>) -> CheckOutcome {
    let id = CheckId::WrongSealKeyFailsClosed;
    let expected =
        "a bearer sealed under the WRONG key resolves to anonymous (200), never a wrong identity";

    let wrong_seeder = ctx.harness.wrong_key_seeder();
    let seed = match wrong_seeder
        .seed(ctx.harness, ctx.namespace, "wrong_key")
        .await
    {
        Ok(seed) => seed,
        Err(e) => {
            return CheckOutcome::fail(id, expected, "wrong-key seeding failed", format!("{e}"));
        }
    };

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

pub async fn run_tampered_envelope(ctx: &CheckContext<'_>) -> CheckOutcome {
    run_corrupted_envelope(
        ctx,
        CheckId::TamperedEnvelopeFailsClosed,
        "a bearer that resolved, then had its stored ciphertext byte-flipped, resolves to anonymous (200)",
        "tampered",
        SealVariant::TamperedCiphertext,
        "the subject opened a tampered ciphertext — the AEAD tag must fail closed to anonymous",
    )
    .await
}

pub async fn run_unreadable_envelope(ctx: &CheckContext<'_>) -> CheckOutcome {
    run_corrupted_envelope(
        ctx,
        CheckId::UnreadableEnvelopeFailsClosed,
        "a bearer that resolved, then had its stored envelope replaced by an unparseable one, resolves to anonymous (200)",
        "unreadable",
        SealVariant::Unreadable,
        "the subject accepted an envelope carrying an unknown field — the parse must fail closed to anonymous",
    )
    .await
}

async fn run_corrupted_envelope(
    ctx: &CheckContext<'_>,
    id: CheckId,
    expected: &'static str,
    label: &str,
    variant: SealVariant,
    on_resolved: &'static str,
) -> CheckOutcome {
    let seed = match ctx.seed(label).await {
        Ok(seed) => seed,
        Err(e) => return CheckOutcome::fail(id, expected, "seeding failed", format!("{e}")),
    };

    match ctx.endpoint.resolve_bearer(&seed.raw).await {
        Ok(Resolution::Resolved(_)) => {}
        Ok(Resolution::Anonymous) => {
            return CheckOutcome::fail(
                id,
                expected,
                "the faithful seed already resolved to anonymous",
                "the corruption must be the only difference — a seed that never resolved proves nothing",
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

    if let Err(e) = ctx.seeder.overwrite(ctx.harness, &seed, variant).await {
        return CheckOutcome::fail(id, expected, "overwriting the seed failed", format!("{e}"));
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
