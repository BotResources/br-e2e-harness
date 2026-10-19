use std::time::Duration;

use reqwest::StatusCode;

use crate::endpoint::Resolution;
use crate::error::Result;
use crate::outcome::{CheckId, CheckOutcome};
use crate::seal::SealedSeed;
use crate::vectors::Vector;

use super::CheckContext;

const KV_LOSS_ANSWER_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn run_kv_error(ctx: &CheckContext<'_>) -> CheckOutcome {
    let id = CheckId::KvErrorFailsLoud;
    let expected = "with the PUBLISHED_LANGUAGE bucket destroyed, the subject answers with a 5xx within the time limit, never a silent 200, a refused connection or a hang";

    let seed = ctx.seed(Vector::KvError).await;

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

    let result = ctx
        .endpoint
        .status_for_bearer_within(&seed.raw, KV_LOSS_ANSWER_TIMEOUT)
        .await;
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
        Err(e) => LoudVerdict::Fail {
            observed: format!("no HTTP status after infra loss ({e})"),
            detail: "a refused connection, a crash or a hang proves no answer, not a 5xx: the subject must answer a lost backend with a 5xx within the time limit".to_string(),
        },
        Ok(status) if status.is_success() => LoudVerdict::Fail {
            observed: format!("HTTP {status}"),
            detail: "a 2xx masks infra loss — the request would proceed; only a 5xx is acceptable, never a silent anonymous/resolved 200".to_string(),
        },
        Ok(status) => LoudVerdict::Fail {
            observed: format!("HTTP {status}"),
            detail: "the endpoint resolves, it never gates — a non-5xx status (e.g. 4xx) is anomalous; only a 5xx is acceptable".to_string(),
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
    fn a_refused_connection_is_not_a_loud_failure() {
        let refused: Result<StatusCode> =
            Err(ConformanceError::Request("connection refused".to_string()));
        assert!(!is_pass(&classify(&refused)));
    }

    #[test]
    fn no_answer_in_time_is_not_a_loud_failure() {
        let hung: Result<StatusCode> = Err(ConformanceError::Request(
            "GET http://127.0.0.1:1/internal/passport: operation timed out".to_string(),
        ));
        match classify(&hung) {
            LoudVerdict::Fail { observed, .. } => {
                assert!(observed.contains("timed out"), "{observed}")
            }
            LoudVerdict::Pass { observed } => panic!("a hang must not pass: {observed}"),
        }
    }

    #[tokio::test]
    async fn a_listener_that_never_answers_fails_within_the_time_limit() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let _held = tokio::spawn(async move {
            let _connection = listener.accept().await;
            std::future::pending::<()>().await;
        });
        let endpoint =
            crate::endpoint::PassportEndpoint::new(&format!("http://127.0.0.1:{port}")).unwrap();
        let result = endpoint
            .status_for_bearer_within("brk_hang", Duration::from_millis(300))
            .await;
        assert!(result.is_err(), "{result:?}");
        assert!(!is_pass(&classify(&result)));
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
