use std::fmt::Debug;
use std::process::Command;
use std::time::Duration;

use conformance_identity::{
    ConformanceError, IdentityHarness, Result, Scenario, SpawnTarget, run_spawn,
};

const CHILD: &str = "CONFORMANCE_IDENTITY_NO_NATS_SERVER_CHILD";
const TEST: &str = "spawn_returns_a_typed_error_when_nats_server_is_not_on_path";

fn assert_names_the_missing_server<T: Debug>(what: &str, result: Result<T>) {
    match result {
        Err(ConformanceError::Jetstream(detail)) => assert!(
            detail.contains("nats-server"),
            "{what}: the error must name the missing prerequisite: {detail}"
        ),
        other => panic!("{what}: a missing nats-server must be a typed error: {other:?}"),
    }
}

fn run_in_isolated_copy() {
    let exe = std::env::current_exe().expect("test executable");
    let isolated = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("no-nats-server-{}", uuid::Uuid::now_v7().simple()));
    let deps = isolated.join("deps");
    let empty = isolated.join("empty-path");
    std::fs::create_dir_all(&deps).expect("create the isolated target directory");
    std::fs::create_dir_all(&empty).expect("create an empty PATH directory");
    std::fs::write(isolated.join("fabric-nats"), b"").expect("write a stub fabric-nats");
    let copy = deps.join(exe.file_name().expect("test executable name"));
    std::fs::copy(&exe, &copy).expect("copy the test executable beside the stub fabric-nats");

    let status = Command::new(&copy)
        .args(["--exact", TEST, "--nocapture"])
        .env(CHILD, "1")
        .env("PATH", &empty)
        .status()
        .expect("run the isolated copy");
    std::fs::remove_dir_all(&isolated).ok();
    assert!(status.success(), "the isolated copy must pass: {status}");
}

#[tokio::test(flavor = "current_thread")]
async fn spawn_returns_a_typed_error_when_nats_server_is_not_on_path() {
    if std::env::var_os(CHILD).is_none() {
        run_in_isolated_copy();
        return;
    }

    let binary = std::env::current_exe().expect("test executable");
    assert_names_the_missing_server(
        "start_with_binary",
        IdentityHarness::start_with_binary(binary.clone())
            .await
            .map(|_| ()),
    );
    assert_names_the_missing_server(
        "run_spawn",
        run_spawn(
            &SpawnTarget { binary },
            &[Scenario::CleanDeclarationAccepted],
            Duration::from_secs(2),
        )
        .await,
    );
}
