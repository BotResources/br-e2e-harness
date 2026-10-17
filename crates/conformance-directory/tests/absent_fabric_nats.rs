mod common;

use std::fmt::Debug;
use std::process::Command;

use conformance_directory::{
    ConformanceError, DirectoryHarness, Result, consumer_reads_users, provision::provision,
};

const ISOLATED: &str = "CONFORMANCE_DIRECTORY_ISOLATED_COPY";
const OFFLINE: &str = "provision_without_the_fabric_nats_cli_is_a_typed_error";
const REAL_INFRA: &str = "batteries_without_the_fabric_nats_cli_are_a_typed_error";

fn run_in_isolated_copy(test: &str, ignored: bool) {
    let exe = std::env::current_exe().expect("test executable");
    let isolated = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("no-fabric-nats-{}", uuid::Uuid::now_v7().simple()));
    let deps = isolated.join("deps");
    std::fs::create_dir_all(&deps).expect("create the isolated target directory");
    let copy = deps.join(exe.file_name().expect("test executable name"));
    std::fs::copy(&exe, &copy).expect("copy the test executable beside no fabric-nats");

    let mut command = Command::new(&copy);
    command.args(["--exact", test, "--nocapture"]);
    if ignored {
        command.arg("--ignored");
    }
    let status = command
        .env(ISOLATED, "1")
        .status()
        .expect("run the isolated copy");
    std::fs::remove_dir_all(&isolated).ok();
    assert!(status.success(), "the isolated copy must pass: {status}");
}

fn assert_names_the_missing_cli<T: Debug>(what: &str, result: Result<T>) {
    match result {
        Err(ConformanceError::Jetstream(detail)) => assert!(
            detail.contains("fabric-nats"),
            "{what} must name the missing CLI: {detail}"
        ),
        other => panic!("{what} without the fabric-nats CLI must be a typed error: {other:?}"),
    }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
}

#[test]
fn provision_without_the_fabric_nats_cli_is_a_typed_error() {
    if std::env::var_os(ISOLATED).is_none() {
        run_in_isolated_copy(OFFLINE, false);
        return;
    }
    runtime().block_on(async {
        assert_names_the_missing_cli(
            "provision",
            provision("nats://127.0.0.1:1", "published_language.toml").await,
        );
    });
}

#[test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
fn batteries_without_the_fabric_nats_cli_are_a_typed_error() {
    if std::env::var_os(ISOLATED).is_none() {
        run_in_isolated_copy(REAL_INFRA, true);
        return;
    }
    runtime().block_on(async {
        assert_names_the_missing_cli(
            "DirectoryHarness::start",
            DirectoryHarness::start().await.map(|_| ()),
        );
        assert_names_the_missing_cli(
            "consumer_reads_users",
            consumer_reads_users(&common::snapshot()).await,
        );
    });
}
