use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use br_test_harness::wait_until;
use conformance_identity::{
    ConformanceError, IdentityHarness, SUBJECT_BINARY, Subject, SubjectConfig,
};

mod common;
mod scratch;

const CHILD: &str = "CONFORMANCE_IDENTITY_BARE_SUBJECT_NAME_CHILD";
const ACCEPTED: &str = "a_bare_name_found_on_path_and_not_in_the_cwd_is_accepted";
const REFUSED: &str = "a_bare_name_found_only_in_the_cwd_is_refused";
const SPAWNED: &str = "a_bare_name_is_spawned_from_its_path_entry_with_that_directory_leading";
const REFUSED_SPAWN: &str = "a_bare_name_found_only_in_the_cwd_is_never_spawned";
const MARKER: &str = "cwd-stand-in-ran";
const CWD_STAND_IN: &[u8] = b"#!/bin/sh\necho ran > cwd-stand-in-ran\nexec sleep 30\n";
const STAND_IN: &[u8] = b"#!/bin/sh\necho \"CHILD_PATH=$PATH\"\nexec sleep 30\n";
const STUB_NATS_SERVER: &[u8] =
    b"#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo \"nats-server: v2.14.1\"; exit 0; fi\nexit 1\n";

fn write_executable(path: &Path, body: &[u8]) {
    std::fs::write(path, body).expect("write a stand-in");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("make the stand-in executable");
}

fn run_isolated(test: &str, on_path: bool, in_cwd: bool) {
    let exe = std::env::current_exe().expect("test executable");
    let isolated = scratch::scratch("bare-subject-name");
    let deps = isolated.join("deps");
    let stub_bin = isolated.join("stub-bin");
    let path_dir = isolated.join("path-dir");
    let cwd = isolated.join("cwd");
    for dir in [&deps, &stub_bin, &path_dir, &cwd] {
        std::fs::create_dir_all(dir).expect("create an isolated directory");
    }
    std::fs::write(isolated.join("fabric-nats"), b"").expect("write a stub fabric-nats");
    write_executable(&stub_bin.join("nats-server"), STUB_NATS_SERVER);
    if on_path {
        write_executable(&path_dir.join(SUBJECT_BINARY), STAND_IN);
    }
    if in_cwd {
        write_executable(&cwd.join(SUBJECT_BINARY), CWD_STAND_IN);
    }
    let copy = deps.join(exe.file_name().expect("test executable name"));
    std::fs::copy(&exe, &copy).expect("copy the test executable beside the stub fabric-nats");

    let output = Command::new(&copy)
        .args(["--exact", test, "--nocapture"])
        .env(CHILD, path_dir.display().to_string())
        .env(
            "PATH",
            format!(
                "{}:/usr/bin:/bin:.:relative:{}",
                stub_bin.display(),
                path_dir.display()
            ),
        )
        .current_dir(&cwd)
        .output()
        .expect("run the isolated copy");
    common::assert_child_ran_one_passing_test(&output);
    assert!(
        !cwd.join(MARKER).exists(),
        "the stand-in of the current directory must never run"
    );
}

fn bare() -> std::path::PathBuf {
    SUBJECT_BINARY.into()
}

#[tokio::test(flavor = "current_thread")]
async fn a_bare_name_found_on_path_and_not_in_the_cwd_is_accepted() {
    if std::env::var_os(CHILD).is_none() {
        run_isolated(ACCEPTED, true, false);
        return;
    }
    match IdentityHarness::start_with_binary(bare()).await {
        Err(ConformanceError::Jetstream(detail)) => assert!(
            detail.contains("start the throwaway NATS"),
            "the subject check must pass and the stub broker be the first failure: {detail}"
        ),
        Err(other) => panic!("a bare name on PATH must pass the subject check: {other:?}"),
        Ok(_) => panic!("the stub broker cannot start"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a_bare_name_found_only_in_the_cwd_is_refused() {
    if std::env::var_os(CHILD).is_none() {
        run_isolated(REFUSED, false, true);
        return;
    }
    match IdentityHarness::start_with_binary(bare()).await {
        Err(ConformanceError::SubjectBinaryUnavailable { binary, detail }) => {
            assert_eq!(binary, SUBJECT_BINARY);
            assert!(detail.contains("PATH"), "{detail}");
        }
        Err(other) => panic!("a name only in the cwd must be a missing binary: {other:?}"),
        Ok(_) => panic!("the cwd must never be searched"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a_bare_name_is_spawned_from_its_path_entry_with_that_directory_leading() {
    let Some(path_dir) = std::env::var_os(CHILD) else {
        run_isolated(SPAWNED, true, false);
        return;
    };
    let subject = Subject::spawn(&bare(), &SubjectConfig::new("nats://127.0.0.1:1"));
    let expected = format!("CHILD_PATH={}:", Path::new(&path_dir).display());
    let seen = wait_until(Duration::from_secs(10), || async {
        subject.logs().contains(&expected)
    })
    .await;
    let logs = subject.logs();
    subject.shutdown().await;
    assert!(seen, "the child PATH must lead with {expected}: {logs}");
}

#[tokio::test(flavor = "current_thread")]
async fn a_bare_name_found_only_in_the_cwd_is_never_spawned() {
    if std::env::var_os(CHILD).is_none() {
        run_isolated(REFUSED_SPAWN, false, true);
        return;
    }
    let spawned = std::panic::catch_unwind(|| {
        Subject::spawn(&bare(), &SubjectConfig::new("nats://127.0.0.1:1"))
    });
    let payload = spawned
        .err()
        .expect("an unresolved bare name must not be spawned");
    let message = panic_text(payload);
    assert!(message.contains(SUBJECT_BINARY), "{message}");
    assert!(message.contains("PATH"), "{message}");
}

fn panic_text(payload: Box<dyn std::any::Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(payload) => payload
            .downcast_ref::<&str>()
            .map(|message| (*message).to_string())
            .unwrap_or_default(),
    }
}
