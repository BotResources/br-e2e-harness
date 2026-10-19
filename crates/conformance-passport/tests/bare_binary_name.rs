use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

use conformance_passport::{ConformanceError, PassportHarness};

const CHILD: &str = "CONFORMANCE_PASSPORT_BARE_NAME_CHILD";
const TEST: &str = "a_bare_binary_name_is_resolved_on_path_like_a_spawned_command";

#[tokio::test(flavor = "current_thread")]
async fn a_bare_binary_name_is_resolved_on_path_like_a_spawned_command() {
    if std::env::var_os(CHILD).is_none() {
        let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("bare-name-{}", uuid::Uuid::now_v7().simple()));
        std::fs::create_dir_all(&dir).expect("create the PATH directory");
        let binary = dir.join("identity-passport");
        std::fs::write(&binary, b"#!/bin/sh\nexec sleep 30\n").expect("write the stand-in");
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755))
            .expect("mark the stand-in executable");
        let mut entries = vec![dir.clone()];
        entries.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        let status = Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, dir.as_os_str())
            .env("PATH", std::env::join_paths(entries).expect("join PATH"))
            .status()
            .expect("run the child test");
        std::fs::remove_dir_all(&dir).ok();
        assert!(status.success(), "the child must pass: {status}");
        return;
    }

    let expected = PathBuf::from(std::env::var_os(CHILD).unwrap()).join("identity-passport");
    let harness = PassportHarness::start_with_binary(PathBuf::from("identity-passport"))
        .await
        .expect("a bare name on PATH must start the harness");
    assert_eq!(harness.binary(), &expected);
    harness.shutdown().await;

    match PassportHarness::start_with_binary(PathBuf::from("no-such-subject-binary")).await {
        Err(ConformanceError::SubjectBinaryUnavailable { detail, .. }) => {
            assert!(detail.contains("not found on PATH"), "{detail}");
        }
        Err(other) => panic!("expected not found on PATH: {other:?}"),
        Ok(_) => panic!("an absent bare name must not start"),
    }
}
