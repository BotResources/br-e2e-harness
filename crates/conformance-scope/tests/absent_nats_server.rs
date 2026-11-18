use std::process::Command;

use conformance_scope::{ConformanceError, ScopeHarness};

const CHILD: &str = "CONFORMANCE_SCOPE_NO_NATS_SERVER_CHILD";
const TEST: &str = "a_missing_nats_server_is_a_typed_error_not_a_panic";

#[tokio::test(flavor = "current_thread")]
async fn a_missing_nats_server_is_a_typed_error_not_a_panic() {
    if std::env::var_os(CHILD).is_none() {
        let empty = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("no-nats-{}", uuid::Uuid::now_v7().simple()));
        std::fs::create_dir_all(&empty).expect("create an empty PATH directory");
        let status = Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, "1")
            .env("PATH", &empty)
            .status()
            .expect("run the child test");
        std::fs::remove_dir_all(&empty).ok();
        assert!(status.success(), "the child must pass: {status}");
        return;
    }

    let binary = std::env::current_exe().expect("test executable");
    match ScopeHarness::start_with_binary(binary).await {
        Err(ConformanceError::Jetstream(detail)) => {
            assert!(detail.contains("throwaway NATS"), "{detail}");
        }
        Err(other) => panic!("expected a typed Jetstream error: {other:?}"),
        Ok(_) => panic!("start must not succeed without nats-server"),
    }
}
