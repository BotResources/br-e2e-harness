use std::process::Command;

mod common;
mod scratch;

use conformance_identity::{ConformanceError, IdentityHarness, provision};

const ISOLATED: &str = "CONFORMANCE_IDENTITY_ISOLATED_COPY";
const TEST: &str = "fabric_nats_cli_absent_is_a_typed_error";

#[test]
fn fabric_nats_cli_absent_is_a_typed_error() {
    if std::env::var_os(ISOLATED).is_some() {
        assert_typed_errors();
        return;
    }
    let exe = std::env::current_exe().expect("test executable");
    let isolated = scratch::scratch("no-fabric-nats");
    let deps = isolated.join("deps");
    std::fs::create_dir_all(&deps).expect("create the isolated target directory");
    let copy = deps.join(exe.file_name().expect("test executable name"));
    std::fs::copy(&exe, &copy).expect("copy the test executable beside no fabric-nats");

    let output = Command::new(&copy)
        .args(["--exact", TEST, "--nocapture"])
        .env(ISOLATED, "1")
        .output()
        .expect("run the isolated copy");
    common::assert_child_ran_one_passing_test(&output);
}

fn assert_typed_errors() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        match provision("nats://127.0.0.1:1", "scope_declaration.toml").await {
            Err(ConformanceError::Jetstream(detail)) => assert!(
                detail.contains("fabric-nats"),
                "provision must name the missing CLI: {detail}"
            ),
            other => panic!("provision without the fabric-nats CLI must be typed: {other:?}"),
        }
        match IdentityHarness::start_with_binary(std::env::current_exe().expect("exe")).await {
            Err(ConformanceError::Jetstream(detail)) => assert!(
                detail.contains("fabric-nats"),
                "start_with_binary must name the missing CLI: {detail}"
            ),
            Err(other) => panic!("start_with_binary must name the missing CLI: {other:?}"),
            Ok(_) => panic!("start_with_binary must not succeed without the fabric-nats CLI"),
        }
    });
}
