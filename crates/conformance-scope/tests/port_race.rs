use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use std::time::Duration;

use conformance_scope::{BootWatch, ConformanceError, Subject, SubjectConfig};

const KEEPS_THE_PORT: &str = "exec sleep 30\n";

fn losing_the_port(text: &str) -> String {
    format!("echo x >> \"$(dirname \"$0\")/attempts\"\necho \"{text}\" >&2\nexit 98\n")
}

fn losing_the_port_twice_then_keeping_it() -> String {
    format!(
        "echo x >> \"$(dirname \"$0\")/attempts\"\n\
         if [ \"$(wc -l < \"$(dirname \"$0\")/attempts\")\" -lt 3 ]; then\n\
         exit 98\n\
         fi\n{KEEPS_THE_PORT}"
    )
}

fn counting_then_keeping_the_port() -> String {
    format!("echo x >> \"$(dirname \"$0\")/attempts\"\n{KEEPS_THE_PORT}")
}

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

const EXIT_LOOKOUT: Duration = Duration::from_secs(60);
const RUNNING_LOOKOUT: Duration = Duration::from_millis(300);
const POLL_LIMIT: Duration = Duration::from_secs(30);

fn watch(lookout: Duration) -> BootWatch {
    BootWatch {
        lookout,
        ..BootWatch::default()
    }
}

async fn wait_for_attempts(dir: &std::path::Path, expected: usize) {
    let began = std::time::Instant::now();
    while attempts(dir) < expected {
        assert!(
            began.elapsed() < POLL_LIMIT,
            "the stand-in subject never ran {expected} time(s): {} run(s)",
            attempts(dir)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a_subject_that_lost_its_port_and_says_nothing_about_it_is_started_again() {
    let (dir, binary) = stand_in(
        "port-race-recovers",
        &losing_the_port_twice_then_keeping_it(),
    );

    let started = Subject::start_with(&binary, &config(), watch(Duration::from_secs(5))).await;
    let tries = attempts(&dir);
    let logs = match started {
        Ok(subject) => {
            let logs = subject.logs();
            subject.shutdown().await;
            logs
        }
        Err(error) => panic!("the subject must be started again on another port: {error}"),
    };
    std::fs::remove_dir_all(&dir).ok();

    assert_eq!(tries, 3, "logs:\n{logs}");
}

#[tokio::test(flavor = "current_thread")]
async fn a_silent_subject_that_keeps_running_is_started_once() {
    let (dir, binary) = stand_in("port-race-silent-runs", &counting_then_keeping_the_port());

    let started = Subject::start_with(&binary, &config(), watch(RUNNING_LOOKOUT)).await;
    wait_for_attempts(&dir, 1).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    let tries = attempts(&dir);
    started
        .expect("a running subject is not retried")
        .shutdown()
        .await;
    std::fs::remove_dir_all(&dir).ok();

    assert_eq!(tries, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn a_subject_that_keeps_losing_its_port_is_a_setup_error_after_five_attempts() {
    let (dir, binary) = stand_in(
        "port-race-gives-up",
        &losing_the_port("Error: Address already in use (os error 98)"),
    );

    let started = Subject::start_with(&binary, &config(), watch(EXIT_LOOKOUT)).await;
    let tries = attempts(&dir);
    std::fs::remove_dir_all(&dir).ok();

    match started {
        Err(error @ ConformanceError::SubjectExitedAtBoot { attempts, .. }) => {
            assert_eq!(attempts, 5);
            let message = error.to_string();
            assert!(message.contains("Address already in use"), "{message}");
            assert!(message.contains("98"), "{message}");
            assert!(message.contains("5 attempts"), "{message}");
        }
        Err(other) => panic!("expected the boot-exit error: {other:?}"),
        Ok(subject) => {
            subject.shutdown().await;
            panic!("a subject that never stays up must not be returned");
        }
    }
    assert_eq!(tries, 5);
}

#[tokio::test(flavor = "current_thread")]
async fn a_boot_watch_with_zero_attempts_is_refused_without_starting_the_subject() {
    let (dir, binary) = stand_in("port-race-zero", &counting_then_keeping_the_port());
    let no_attempts = BootWatch {
        attempts: 0,
        ..BootWatch::default()
    };

    let started = Subject::start_with(&binary, &config(), no_attempts).await;
    let tries = attempts(&dir);
    std::fs::remove_dir_all(&dir).ok();

    assert!(
        matches!(started, Err(ConformanceError::InvalidInput(_))),
        "zero attempts must be InvalidInput"
    );
    assert_eq!(tries, 0);
}
