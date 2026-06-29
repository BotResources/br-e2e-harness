use reqwest::StatusCode;

use crate::endpoint::Resolution;
use crate::error::Result;
use crate::outcome::{CheckId, CheckOutcome};
use crate::seal::SealedSeed;

use super::CheckContext;

pub async fn run_kv_error(ctx: &CheckContext<'_>) -> CheckOutcome {
    let id = CheckId::KvErrorFailsLoud;
    let expected = "with the PUBLISHED_LANGUAGE bucket destroyed, resolution fails loudly (5xx or the resolver becomes unreachable), never a silent 200";

    let seed = match ctx.seeder.seed(ctx.namespace, "kv_error").await {
        Ok(seed) => seed,
        Err(e) => return CheckOutcome::fail(id, expected, "seeding failed", format!("{e}")),
    };

    match ctx.endpoint.resolve_bearer(&seed.raw).await {
        Ok(Resolution::Resolved(_)) => {}
        Ok(Resolution::Anonymous) => {
            return CheckOutcome::fail(
                id,
                expected,
                "the subject resolved the seed as anonymous BEFORE bucket deletion",
                "subject did not resolve the seed before bucket deletion — cannot attribute a later failure to the infra loss",
            );
        }
        Err(e) => {
            return CheckOutcome::fail(
                id,
                expected,
                "the subject errored on the seed BEFORE bucket deletion",
                format!(
                    "subject did not resolve the seed before bucket deletion — cannot attribute a later failure to the infra loss: {e}"
                ),
            );
        }
    }

    ctx.harness.delete_published_language().await;

    let result = ctx.endpoint.status_for_bearer(&seed.raw).await;
    match classify(&result) {
        LoudVerdict::Pass { observed } => {
            CheckOutcome::pass(id, expected, format!("{observed} for {}", label(&seed)))
        }
        LoudVerdict::Fail { observed, detail } => {
            CheckOutcome::fail(id, expected, observed, detail)
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum LoudVerdict {
    Pass { observed: String },
    Fail { observed: String, detail: String },
}

fn classify(result: &Result<StatusCode>) -> LoudVerdict {
    match result {
        Ok(status) if status.is_server_error() => LoudVerdict::Pass {
            observed: format!("HTTP {status} — loss surfaced loudly"),
        },
        Err(e) => LoudVerdict::Pass {
            observed: format!(
                "resolver unreachable after infra loss — loud, not silently anonymous ({e})"
            ),
        },
        Ok(status) if status.is_success() => LoudVerdict::Fail {
            observed: format!("HTTP {status}"),
            detail: "a 2xx masks infra loss — the request would proceed; only a loud 5xx or an unreachable resolver is acceptable, never a silent anonymous/resolved 200".to_string(),
        },
        Ok(status) => LoudVerdict::Fail {
            observed: format!("HTTP {status}"),
            detail: "the endpoint resolves, it never gates — a non-5xx status (e.g. 4xx) is anomalous; only a loud 5xx or an unreachable resolver is acceptable".to_string(),
        },
    }
}

fn label(seed: &SealedSeed) -> String {
    format!("token_id={}", seed.token_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ConformanceError;

    fn is_pass(verdict: &LoudVerdict) -> bool {
        matches!(verdict, LoudVerdict::Pass { .. })
    }

    #[test]
    fn server_error_500_is_a_loud_pass() {
        assert!(is_pass(&classify(&Ok(StatusCode::INTERNAL_SERVER_ERROR))));
    }

    #[test]
    fn server_error_503_is_a_loud_pass() {
        assert!(is_pass(&classify(&Ok(StatusCode::SERVICE_UNAVAILABLE))));
    }

    #[test]
    fn transport_error_is_a_loud_pass() {
        let unreachable: Result<StatusCode> =
            Err(ConformanceError::Request("connection refused".to_string()));
        assert!(is_pass(&classify(&unreachable)));
    }

    #[test]
    fn ok_200_masking_loss_is_a_fail() {
        assert!(!is_pass(&classify(&Ok(StatusCode::OK))));
    }

    #[test]
    fn ok_401_is_a_fail() {
        assert!(!is_pass(&classify(&Ok(StatusCode::UNAUTHORIZED))));
    }
}
