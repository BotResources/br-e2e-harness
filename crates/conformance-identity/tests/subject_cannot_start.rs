use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::Duration;

use conformance_identity::{ConformanceError, Scenario, SpawnTarget, run_spawn};

const SHORT: Duration = Duration::from_secs(2);

fn stand_in(label: &str, mode: u32, body: &[u8]) -> PathBuf {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("{label}-{}", uuid::Uuid::now_v7().simple()));
    std::fs::create_dir_all(&dir).expect("create the stand-in directory");
    let binary = dir.join("identity-acceptor");
    std::fs::write(&binary, body).expect("write the stand-in subject");
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(mode))
        .expect("set the stand-in mode");
    binary
}

fn remove(binary: &std::path::Path) {
    std::fs::remove_dir_all(binary.parent().expect("stand-in directory")).ok();
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "real-infra: needs `nats-server` on PATH and the fabric-nats CLI built"]
async fn a_subject_that_cannot_be_executed_is_a_typed_input_error() {
    let binary = stand_in("not-executable", 0o644, b"#!/bin/sh\nexit 0\n");
    let result = run_spawn(
        &SpawnTarget {
            binary: binary.clone(),
        },
        &[Scenario::CleanDeclarationAccepted],
        SHORT,
    )
    .await;
    remove(&binary);
    match result {
        Err(ConformanceError::InvalidInput(detail)) => assert!(
            detail.contains("identity-acceptor"),
            "the error must name the binary: {detail}"
        ),
        other => panic!("a subject that cannot start must be a typed input error: {other:?}"),
    }
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "real-infra: needs `nats-server` on PATH and the fabric-nats CLI built"]
async fn a_subject_that_exits_at_once_is_a_timeout_carrying_its_logs() {
    let binary = stand_in(
        "exits-at-once",
        0o755,
        b"#!/bin/sh\necho boom-from-the-subject\n",
    );
    let result = run_spawn(
        &SpawnTarget {
            binary: binary.clone(),
        },
        &[Scenario::CleanDeclarationAccepted],
        SHORT,
    )
    .await;
    remove(&binary);
    match result {
        Err(ConformanceError::Timeout(detail)) => assert!(
            detail.contains("boom-from-the-subject"),
            "the timeout must carry the subject logs: {detail}"
        ),
        other => panic!("a subject that never gets ready must be a typed timeout: {other:?}"),
    }
}
