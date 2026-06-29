use reqwest::StatusCode;

use crate::outcome::{CheckId, CheckOutcome};
use crate::seal::SealedSeed;

use super::CheckContext;

pub async fn run_kv_error(ctx: &CheckContext<'_>) -> CheckOutcome {
    let id = CheckId::KvErrorIs500;
    let expected = "with the PUBLISHED_LANGUAGE bucket destroyed, resolution returns 500 (never silently anonymous)";

    let seed = match ctx.seeder.seed(ctx.namespace, "kv_error").await {
        Ok(seed) => seed,
        Err(e) => return CheckOutcome::fail(id, expected, "seeding failed", format!("{e}")),
    };

    ctx.harness.delete_published_language().await;

    let status = match ctx.endpoint.status_for_bearer(&seed.raw).await {
        Ok(status) => status,
        Err(e) => {
            return CheckOutcome::fail(id, expected, "the endpoint call failed", format!("{e}"));
        }
    };

    if status == StatusCode::INTERNAL_SERVER_ERROR {
        CheckOutcome::pass(id, expected, format!("HTTP {status} for {}", label(&seed)))
    } else {
        CheckOutcome::fail(
            id,
            expected,
            format!("HTTP {status}"),
            "a backend KV failure must surface as 500; a 200/anonymous here would mask infra loss as a valid anonymous request",
        )
    }
}

fn label(seed: &SealedSeed) -> String {
    format!("token_id={}", seed.token_id)
}
