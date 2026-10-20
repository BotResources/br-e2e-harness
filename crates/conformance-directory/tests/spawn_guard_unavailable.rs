#[path = "common/child.rs"]
mod child;
mod common;
#[path = "common/scratch.rs"]
mod scratch;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use conformance_directory::{
    ANCHOR_BINARY, ConformanceError, DirectoryHarness, consumer_reads_users, emit_snapshot,
    locate_and_emit, provision::provision,
};

const CHILD: &str = "CONFORMANCE_DIRECTORY_NO_SH_CHILD";
const SCRIPT: &[u8] = b"#!/bin/sh\nexit 0\n";

fn write_executable(path: &Path, body: &[u8]) {
    std::fs::write(path, body).expect("write a stub");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("make the stub executable");
}

fn run_without_sh(test: &str) {
    let exe = std::env::current_exe().expect("test executable");
    let isolated = scratch::scratch("no-sh");
    let deps = isolated.join("deps");
    let stub_bin = isolated.join("stub-bin");
    std::fs::create_dir_all(&deps).expect("create the isolated target directory");
    std::fs::create_dir_all(&stub_bin).expect("create the stub PATH directory");
    write_executable(&isolated.join("fabric-nats"), SCRIPT);
    write_executable(&stub_bin.join("nats-server"), SCRIPT);
    write_executable(&stub_bin.join(ANCHOR_BINARY), SCRIPT);
    let copy = deps.join(exe.file_name().expect("test executable name"));
    std::fs::copy(&exe, &copy).expect("copy the test executable beside the stub fabric-nats");

    let output = Command::new(&copy)
        .args(["--exact", test, "--nocapture"])
        .env(CHILD, "1")
        .env("PATH", &stub_bin)
        .output()
        .expect("run the isolated copy");
    child::assert_child_ran_one_passing_test(&output);
}

fn stub_anchor() -> PathBuf {
    let path = std::env::var_os("PATH").expect("the child PATH");
    PathBuf::from(path).join(ANCHOR_BINARY)
}

fn assert_names_the_guard(what: &str, detail: &str) {
    assert!(
        detail.contains("orphan guard"),
        "{what} must say the guard of the spawned process could not start: {detail}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_nats_server_probe_that_cannot_spawn_its_guard_is_a_typed_error() {
    if std::env::var_os(CHILD).is_none() {
        run_without_sh("a_nats_server_probe_that_cannot_spawn_its_guard_is_a_typed_error");
        return;
    }
    match DirectoryHarness::start().await.map(|_| ()) {
        Err(ConformanceError::Jetstream(detail)) => {
            assert!(detail.contains("nats-server"), "{detail}");
            assert_names_the_guard("the probe", &detail);
        }
        other => panic!("a probe without a spawnable sh must be a typed error: {other:?}"),
    }
    match consumer_reads_users(&common::snapshot()).await {
        Err(ConformanceError::Jetstream(detail)) => {
            assert!(detail.contains("nats-server"), "{detail}");
            assert_names_the_guard("the consumer probe", &detail);
        }
        other => {
            panic!("a consumer battery without a spawnable sh must be a typed error: {other:?}")
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a_provision_that_cannot_spawn_its_guard_is_a_typed_error() {
    if std::env::var_os(CHILD).is_none() {
        run_without_sh("a_provision_that_cannot_spawn_its_guard_is_a_typed_error");
        return;
    }
    match provision("nats://127.0.0.1:1", "published_language.toml").await {
        Err(ConformanceError::Jetstream(detail)) => {
            assert!(detail.contains("provision"), "{detail}");
            assert_names_the_guard("provision", &detail);
        }
        other => panic!("a provision without a spawnable sh must be a typed error: {other:?}"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn an_anchor_run_that_cannot_spawn_its_guard_is_a_typed_error() {
    if std::env::var_os(CHILD).is_none() {
        run_without_sh("an_anchor_run_that_cannot_spawn_its_guard_is_a_typed_error");
        return;
    }
    match emit_snapshot(&stub_anchor()).await {
        Err(ConformanceError::Run(detail)) => assert_names_the_guard("emit_snapshot", &detail),
        other => panic!("an anchor run without a spawnable sh must be a typed error: {other:?}"),
    }
    match locate_and_emit().await {
        Err(ConformanceError::Run(detail)) => assert_names_the_guard("locate_and_emit", &detail),
        other => panic!("locate_and_emit without a spawnable sh must be a typed error: {other:?}"),
    }
}
