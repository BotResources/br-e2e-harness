use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::Duration;

use br_test_harness::wait_until;
use conformance_scope::{Subject, SubjectConfig};

const LOSES_THE_PORT: &str = "echo x >> \"$(dirname \"$0\")/attempts\"\n\
     echo \"http listening on $HTTP_ADDR (/readyz, /livez)\"\n\
     echo \"http server: listen tcp $HTTP_ADDR: bind: address already in use\"\n\
     exit 1\n";

const KEEPS_THE_PORT: &str =
    "echo \"http listening on $HTTP_ADDR (/readyz, /livez)\"\nexec sleep 30\n";

fn stand_in(name: &str, body: &str) -> (PathBuf, PathBuf) {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("{name}-{}", uuid::Uuid::now_v7().simple()));
    std::fs::create_dir_all(&dir).expect("create the subject directory");
    let binary = dir.join("scope-service");
    std::fs::write(&binary, format!("#!/bin/sh\n{body}")).expect("write the stand-in subject");
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755))
        .expect("mark the stand-in executable");
    (dir, binary)
}

fn config() -> SubjectConfig {
    SubjectConfig::new("nats://127.0.0.1:1", "STREAM_CMD", "STREAM_EVT", "svc")
}

fn attempts(dir: &std::path::Path) -> usize {
    std::fs::read_to_string(dir.join("attempts"))
        .map(|raw| raw.lines().count())
        .unwrap_or(0)
}

#[tokio::test(flavor = "current_thread")]
async fn a_subject_that_lost_its_port_is_started_again_on_another_one() {
    let body = format!(
        "echo x >> \"$(dirname \"$0\")/attempts\"\n\
         if [ \"$(wc -l < \"$(dirname \"$0\")/attempts\")\" -lt 3 ]; then\n\
         echo \"http listening on $HTTP_ADDR (/readyz, /livez)\"\n\
         echo \"http server: listen tcp $HTTP_ADDR: bind: address already in use\"\n\
         exit 1\n\
         fi\n{KEEPS_THE_PORT}"
    );
    let (dir, binary) = stand_in("port-race-recovers", &body);

    let subject = Subject::start(&binary, &config()).await;
    let logs = subject.logs();
    let tries = attempts(&dir);
    subject.shutdown().await;
    std::fs::remove_dir_all(&dir).ok();

    assert_eq!(tries, 3, "{logs}");
    assert!(!logs.contains("address already in use"), "{logs}");
}

#[tokio::test(flavor = "current_thread")]
async fn a_subject_that_keeps_losing_its_port_is_given_up_after_five_attempts() {
    let (dir, binary) = stand_in("port-race-gives-up", LOSES_THE_PORT);

    let subject = Subject::start(&binary, &config()).await;
    wait_until(Duration::from_secs(10), || async { attempts(&dir) >= 5 }).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    let tries = attempts(&dir);
    subject.shutdown().await;
    std::fs::remove_dir_all(&dir).ok();

    assert_eq!(tries, 5);
}
