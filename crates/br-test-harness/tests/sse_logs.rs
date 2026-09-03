#![cfg(all(feature = "sse", feature = "server", feature = "passport"))]

mod sse_support;

use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::time::Duration;

use br_test_harness::{SpawnedProcess, wait_until};
use futures_util::FutureExt as _;
use sse_support::{PUSH, QUIET, open_closing_after, open_silent};

const BOOTED_MARKER: &str = "service-line-written-before-attach";
const LATE_MARKER: &str = "service-line-written-after-attach";

fn spawn_service(script: String) -> SpawnedProcess {
    SpawnedProcess::spawn("/bin/sh", &["-c", &script], &[])
}

async fn await_log_line(process: &SpawnedProcess, marker: &str) {
    let captured = wait_until(Duration::from_secs(5), || async {
        process.logs().contains(marker)
    })
    .await;
    assert!(
        captured,
        "the spawned service must have flushed its log line"
    );
}

async fn panic_message<F: Future<Output = ()>>(call: F) -> String {
    let payload = AssertUnwindSafe(call)
        .catch_unwind()
        .await
        .expect_err("the call under test must panic");
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_string()))
        .expect("a panic payload the test can read")
}

#[tokio::test]
async fn expect_event_dumps_the_attached_service_log_on_a_closed_stream() {
    let process = spawn_service(format!("echo {BOOTED_MARKER} >&2; exec sleep 30"));
    await_log_line(&process, BOOTED_MARKER).await;
    let mut sub = open_closing_after(String::new()).await.with_logs(&process);

    let message = panic_message(async move {
        sub.expect_event("a tick", PUSH).await;
    })
    .await;

    assert!(message.contains("got Closed"), "{message}");
    assert!(message.contains("service log tail"), "{message}");
    assert!(message.contains(BOOTED_MARKER), "{message}");
    process.shutdown().await;
}

#[tokio::test]
async fn expect_event_dumps_the_attached_service_log_on_a_timeout() {
    let process = spawn_service(format!("echo {BOOTED_MARKER} >&2; exec sleep 30"));
    await_log_line(&process, BOOTED_MARKER).await;
    let mut sub = open_silent().await.with_logs(&process);

    let message = panic_message(async move {
        sub.expect_event("a tick", QUIET).await;
    })
    .await;

    assert!(message.contains("got Timeout"), "{message}");
    assert!(message.contains(BOOTED_MARKER), "{message}");
    process.shutdown().await;
}

#[tokio::test]
async fn expect_event_on_dumps_the_attached_service_log_on_a_closed_stream() {
    let process = spawn_service(format!("echo {BOOTED_MARKER} >&2; exec sleep 30"));
    await_log_line(&process, BOOTED_MARKER).await;
    let mut sub = open_closing_after(String::new()).await.with_logs(&process);

    let message = panic_message(async move {
        sub.expect_event_on("tick", PUSH).await;
    })
    .await;

    assert!(message.contains("got Closed"), "{message}");
    assert!(message.contains(BOOTED_MARKER), "{message}");
    process.shutdown().await;
}

#[tokio::test]
async fn expect_silence_dumps_the_attached_service_log_on_a_closed_stream() {
    let process = spawn_service(format!("echo {BOOTED_MARKER} >&2; exec sleep 30"));
    await_log_line(&process, BOOTED_MARKER).await;
    let mut sub = open_closing_after(String::new()).await.with_logs(&process);

    let message = panic_message(async move {
        sub.expect_silence("no push", QUIET).await;
    })
    .await;

    assert!(
        message.contains("the server closed the stream"),
        "{message}"
    );
    assert!(message.contains(BOOTED_MARKER), "{message}");
    process.shutdown().await;
}

#[tokio::test]
async fn the_dump_reads_the_log_at_panic_time_not_at_attach_time() {
    let process = spawn_service(format!("sleep 0.5; echo {LATE_MARKER} >&2; exec sleep 30"));
    let mut sub = open_closing_after(String::new()).await.with_logs(&process);
    assert!(
        !process.logs().contains(LATE_MARKER),
        "the attach must happen before the service writes the line, or this proves nothing"
    );
    await_log_line(&process, LATE_MARKER).await;

    let message = panic_message(async move {
        sub.expect_silence("no push", QUIET).await;
    })
    .await;

    assert!(
        message.contains(LATE_MARKER),
        "a line the service printed after the attach must still reach the panic: {message}"
    );
    process.shutdown().await;
}

#[tokio::test]
async fn a_closed_stream_still_names_closed_with_no_log_attached() {
    let mut sub = open_closing_after(String::new()).await;

    let message = panic_message(async move {
        sub.expect_silence("no push", QUIET).await;
    })
    .await;

    assert!(
        message.contains("the server closed the stream"),
        "{message}"
    );
    assert!(
        !message.contains("service log tail"),
        "an unattached subscription must not claim a log dump: {message}"
    );
}
