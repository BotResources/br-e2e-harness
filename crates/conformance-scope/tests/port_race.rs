use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use conformance_scope::{BootWatch, ConformanceError, MAX_TIMEOUT, Subject, SubjectConfig};

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

fn losing_the_port_to_a_process_that_answers_livez() -> String {
    format!(
        "dir=\"$(dirname \"$0\")\"\n\
         echo \"$HTTP_ADDR\" >> \"$dir/addrs\"\n\
         if [ \"$(wc -l < \"$dir/addrs\")\" -le 1 ]; then\n\
         until [ -f \"$dir/answered\" ]; do sleep 0.02; done\n\
         exit 98\n\
         fi\n{KEEPS_THE_PORT}"
    )
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

fn announced_addresses(dir: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(dir.join("addrs"))
        .map(|raw| raw.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

async fn answer_livez(listener: TcpListener, marker: Option<PathBuf>) {
    loop {
        let Ok((mut connection, _)) = listener.accept().await else {
            return;
        };
        let mut request = [0u8; 1024];
        if connection.read(&mut request).await.is_err() {
            continue;
        }
        let answer = b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        let flushed =
            connection.write_all(answer).await.is_ok() && connection.flush().await.is_ok();
        if flushed && let Some(marker) = &marker {
            std::fs::write(marker, "x").ok();
        }
    }
}

async fn answer_every_announced_address(dir: PathBuf) {
    let mut bound = 0;
    let mut tasks = Vec::new();
    loop {
        let announced = announced_addresses(&dir);
        while bound < announced.len() {
            if let Ok(listener) = TcpListener::bind(&announced[bound]).await {
                let marker = (bound == 0).then(|| dir.join("answered"));
                tasks.push(tokio::spawn(answer_livez(listener, marker)));
            }
            bound += 1;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
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

#[tokio::test(flavor = "current_thread")]
async fn a_boot_watch_above_the_cap_is_refused_without_starting_the_subject() {
    let beyond = MAX_TIMEOUT + Duration::from_nanos(1);
    let watches = [
        (
            "lookout-max",
            BootWatch {
                lookout: Duration::MAX,
                ..BootWatch::default()
            },
        ),
        (
            "lookout-cap",
            BootWatch {
                lookout: beyond,
                ..BootWatch::default()
            },
        ),
        (
            "settle-max",
            BootWatch {
                settle: Duration::MAX,
                ..BootWatch::default()
            },
        ),
        (
            "settle-cap",
            BootWatch {
                settle: beyond,
                ..BootWatch::default()
            },
        ),
    ];
    for (label, watch) in watches {
        let (dir, binary) = stand_in(
            &format!("port-race-above-cap-{label}"),
            &counting_then_keeping_the_port(),
        );

        let started = Subject::start_with(&binary, &config(), watch).await;
        let tries = attempts(&dir);
        std::fs::remove_dir_all(&dir).ok();

        match started {
            Err(ConformanceError::InvalidInput(detail)) => {
                assert!(detail.contains("24 hours"), "{label}: {detail}");
            }
            other => panic!("{label}: expected InvalidInput: {:?}", other.err()),
        }
        assert_eq!(tries, 0, "{label}: the subject must not be started");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a_subject_that_lost_its_port_to_a_process_answering_livez_is_started_again() {
    let (dir, binary) = stand_in(
        "port-race-answered-by-the-holder",
        &losing_the_port_to_a_process_that_answers_livez(),
    );
    let holder = tokio::spawn(answer_every_announced_address(dir.clone()));
    let patient = BootWatch {
        lookout: Duration::from_secs(10),
        settle: Duration::from_secs(3),
        ..BootWatch::default()
    };

    let started = Subject::start_with(&binary, &config(), patient).await;
    let announced = announced_addresses(&dir);
    holder.abort();
    let logs = match started {
        Ok(subject) => {
            let logs = subject.logs();
            subject.shutdown().await;
            logs
        }
        Err(error) => panic!("the subject must be started again on another port: {error}"),
    };
    std::fs::remove_dir_all(&dir).ok();

    assert_eq!(
        announced.len(),
        2,
        "addresses: {announced:?}\nlogs:\n{logs}"
    );
}
