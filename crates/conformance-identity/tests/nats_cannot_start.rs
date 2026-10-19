use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

mod common;
mod scratch;

use conformance_identity::{ConformanceError, IdentityHarness};

const CHILD: &str = "CONFORMANCE_IDENTITY_NATS_CANNOT_START_CHILD";
const TEST: &str = "a_throwaway_nats_that_cannot_start_is_a_typed_error";
const STUB_NATS_SERVER: &[u8] =
    b"#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo \"nats-server: v2.14.1\"; exit 0; fi\nexit 1\n";

fn write_executable(path: &Path, body: &[u8]) {
    std::fs::write(path, body).expect("write a stub");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("make the stub executable");
}

fn run_in_isolated_copy() {
    let exe = std::env::current_exe().expect("test executable");
    let isolated = scratch::scratch("nats-cannot-start");
    let deps = isolated.join("deps");
    let stub_bin = isolated.join("stub-bin");
    std::fs::create_dir_all(&deps).expect("create the isolated target directory");
    std::fs::create_dir_all(&stub_bin).expect("create the stub PATH directory");
    std::fs::write(isolated.join("fabric-nats"), b"").expect("write a stub fabric-nats");
    write_executable(&stub_bin.join("nats-server"), STUB_NATS_SERVER);
    let copy = deps.join(exe.file_name().expect("test executable name"));
    std::fs::copy(&exe, &copy).expect("copy the test executable beside the stub fabric-nats");

    let output = Command::new(&copy)
        .args(["--exact", TEST, "--nocapture"])
        .env(CHILD, "1")
        .env("PATH", format!("{}:/usr/bin:/bin", stub_bin.display()))
        .output()
        .expect("run the isolated copy");
    common::assert_child_ran_one_passing_test(&output);
}

#[tokio::test(flavor = "current_thread")]
async fn a_throwaway_nats_that_cannot_start_is_a_typed_error() {
    if std::env::var_os(CHILD).is_none() {
        run_in_isolated_copy();
        return;
    }

    let binary = std::env::current_exe().expect("test executable");
    match IdentityHarness::start_with_binary(binary).await {
        Err(ConformanceError::Jetstream(detail)) => assert!(
            detail.contains("start the throwaway NATS"),
            "the error must say the broker did not start: {detail}"
        ),
        Err(other) => {
            panic!("a broker that cannot start must be a typed Jetstream error: {other:?}")
        }
        Ok(_) => panic!("a broker that exits at once must not yield a harness"),
    }
}
