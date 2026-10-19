use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use conformance_scope::{
    AcceptorBehavior, ConformanceError, ExpectedDeclaration, Scenario, SpawnTarget, provision,
    run_spawn,
};

const CHILD: &str = "CONFORMANCE_SCOPE_NO_FABRIC_NATS_CHILD";
const PROVISION_TEST: &str = "provision_without_fabric_nats_is_a_typed_error_not_a_panic";
const SPAWN_TEST: &str = "run_spawn_without_fabric_nats_is_a_typed_error_not_a_panic";

fn rerun_without_fabric_nats(test: &str) {
    let isolated = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("no-fabric-nats-{}", uuid::Uuid::now_v7().simple()));
    std::fs::create_dir_all(&isolated).expect("create an empty executable directory");
    let copy = isolated.join("test-executable");
    std::fs::copy(std::env::current_exe().expect("test executable"), &copy)
        .expect("copy the test executable next to nothing");
    let status = Command::new(&copy)
        .args(["--exact", test, "--include-ignored", "--nocapture"])
        .env(CHILD, "1")
        .status()
        .expect("run the child test");
    std::fs::remove_dir_all(&isolated).ok();
    assert!(status.success(), "the child must pass: {status}");
}

fn assert_names_fabric_nats(error: ConformanceError) {
    match error {
        ConformanceError::FabricProvisionUnavailable { .. } => {
            let message = error.to_string();
            assert!(message.contains("fabric-nats"), "{message}");
            assert!(message.contains("attach mode"), "{message}");
        }
        other => panic!("expected a typed provisioning error: {other:?}"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn provision_without_fabric_nats_is_a_typed_error_not_a_panic() {
    if std::env::var_os(CHILD).is_none() {
        rerun_without_fabric_nats(PROVISION_TEST);
        return;
    }
    match provision("nats://127.0.0.1:1", "scope_declaration.toml").await {
        Err(error) => assert_names_fabric_nats(error),
        Ok(()) => panic!("provisioning must not succeed without fabric-nats"),
    }
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn run_spawn_without_fabric_nats_is_a_typed_error_not_a_panic() {
    if std::env::var_os(CHILD).is_none() {
        rerun_without_fabric_nats(SPAWN_TEST);
        return;
    }
    let target = SpawnTarget {
        binary: std::env::current_exe().expect("test executable"),
    };
    let expected = ExpectedDeclaration::new("notifier", Vec::new());
    let report = run_spawn(
        &target,
        &expected,
        &AcceptorBehavior::Accept,
        &[Scenario::DeclareWellFormed],
        Duration::from_secs(5),
    )
    .await;
    match report {
        Err(error) => assert_names_fabric_nats(error),
        Ok(report) => panic!("spawn must not run without fabric-nats: {report:?}"),
    }
}
