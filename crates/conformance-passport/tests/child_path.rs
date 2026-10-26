use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use br_test_harness::wait_until;
use conformance_passport::{Subject, SubjectConfig};

#[tokio::test(flavor = "current_thread")]
async fn the_subject_directory_leads_the_path_of_the_spawned_child() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("child-path-{}", uuid::Uuid::now_v7().simple()));
    std::fs::create_dir_all(&dir).expect("create the subject directory");
    let binary = dir.join("identity-passport");
    std::fs::write(
        &binary,
        b"#!/bin/sh\necho \"CHILD_PATH=$PATH\"\nexec sleep 30\n",
    )
    .expect("write the stand-in subject");
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755))
        .expect("mark the stand-in executable");

    let subject = Subject::spawn(&binary, &SubjectConfig::new("nats://127.0.0.1:1"));
    let expected = format!("CHILD_PATH={}:", dir.display());
    let seen = wait_until(Duration::from_secs(10), || async {
        subject.logs().contains(&expected)
    })
    .await;
    let logs = subject.logs();
    subject.shutdown().await;
    std::fs::remove_dir_all(&dir).ok();
    assert!(seen, "the child PATH must lead with {expected}: {logs}");
}
