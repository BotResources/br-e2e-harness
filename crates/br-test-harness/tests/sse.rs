#![cfg(all(feature = "sse", feature = "server", feature = "passport"))]

use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::post;
use br_test_harness::{
    DrainStop, PassportBuilder, SpawnedProcess, SseOutcome, SseSubscription, TestServer, wait_until,
};
use bytes::Bytes;
use futures_util::FutureExt as _;

const SERVICE_MARKER: &str = "service-refused-to-serve";
const QUIET: Duration = Duration::from_millis(300);
const PUSH: Duration = Duration::from_secs(2);

fn next_frames(count: usize) -> String {
    (0..count)
        .map(|i| format!("event: next\ndata: {{\"data\":{{\"tick\":{i}}}}}\n\n"))
        .collect()
}

fn error_frame() -> String {
    "event: next\ndata: {\"errors\":[{\"message\":\"boom\"}]}\n\n".to_string()
}

async fn serve_finite(body: String) -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/event-stream")], body)
}

async fn serve_open_forever() -> impl IntoResponse {
    let never = futures_util::stream::pending::<Result<Bytes, std::io::Error>>();
    (
        [(header::CONTENT_TYPE, "text/event-stream")],
        Body::from_stream(never),
    )
}

async fn open_against(router: Router) -> SseSubscription {
    let server = TestServer::spawn(router).await;
    SseSubscription::open(
        &server.base_url,
        &PassportBuilder::new().build(),
        "subscription { tick }",
    )
    .await
}

async fn open_closing_after(body: String) -> SseSubscription {
    open_against(Router::new().route(
        "/graphql",
        post(move || {
            let frames = body.clone();
            async move { serve_finite(frames).await }
        }),
    ))
    .await
}

async fn open_silent() -> SseSubscription {
    open_against(Router::new().route("/graphql", post(serve_open_forever))).await
}

async fn spawn_logging_service() -> SpawnedProcess {
    let proc = SpawnedProcess::spawn(
        "/bin/sh",
        &["-c", &format!("echo {SERVICE_MARKER} >&2; exec sleep 30")],
        &[],
    );
    let captured = wait_until(Duration::from_secs(5), || async {
        proc.logs().contains(SERVICE_MARKER)
    })
    .await;
    assert!(
        captured,
        "the spawned service must have flushed its log line"
    );
    proc
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
async fn next_outcome_reports_closed_once_the_server_ends_the_body() {
    let mut sub = open_closing_after(next_frames(1)).await;

    assert!(matches!(sub.next_outcome(PUSH).await, SseOutcome::Event(_)));
    assert_eq!(
        sub.next_outcome(PUSH).await,
        SseOutcome::Closed,
        "a stream the server ended must read as Closed, never as a timeout"
    );
    assert_eq!(
        sub.next_outcome(PUSH).await,
        SseOutcome::Closed,
        "a closed stream stays Closed on every later read"
    );
}

#[tokio::test]
async fn next_outcome_reports_timeout_on_a_stream_held_open_without_frames() {
    let mut sub = open_silent().await;

    assert_eq!(
        sub.next_outcome(QUIET).await,
        SseOutcome::Timeout,
        "an open stream with nothing to say must read as Timeout, never as Closed"
    );
    assert_eq!(sub.next_outcome(QUIET).await, SseOutcome::Timeout);
}

#[tokio::test]
async fn next_outcome_carries_the_pushed_payload() {
    let mut sub = open_closing_after(next_frames(1)).await;

    match sub.next_outcome(PUSH).await {
        SseOutcome::Event(event) => assert_eq!(event["tick"], 0),
        other => panic!("a pushed frame must read as Event: {other:?}"),
    }
}

#[tokio::test]
async fn next_event_still_flattens_both_quiet_outcomes_to_none() {
    let mut closed = open_closing_after(String::new()).await;
    assert!(closed.next_event(PUSH).await.is_none());

    let mut silent = open_silent().await;
    assert!(silent.next_event(QUIET).await.is_none());
}

#[tokio::test]
async fn expect_silence_passes_on_a_stream_that_stays_open_and_quiet() {
    let mut sub = open_silent().await;

    sub.expect_silence("no push after an unrelated write", QUIET)
        .await;
}

#[tokio::test]
#[should_panic(expected = "the server closed the stream")]
async fn expect_silence_panics_when_the_stream_was_closed_instead_of_quiet() {
    let mut sub = open_closing_after(String::new()).await;

    sub.expect_silence("no push after an unrelated write", QUIET)
        .await;
}

#[tokio::test]
#[should_panic(expected = "no push after an unrelated write")]
async fn expect_silence_names_the_expectation_when_the_stream_was_closed() {
    let mut sub = open_closing_after(String::new()).await;

    sub.expect_silence("no push after an unrelated write", QUIET)
        .await;
}

#[tokio::test]
#[should_panic(expected = "got Timeout")]
async fn expect_event_names_timeout_when_the_stream_stayed_open() {
    let mut sub = open_silent().await;

    sub.expect_event("a tick", QUIET).await;
}

#[tokio::test]
#[should_panic(expected = "got Closed")]
async fn expect_event_names_closed_when_the_server_ended_the_stream() {
    let mut sub = open_closing_after(String::new()).await;

    sub.expect_event("a tick", PUSH).await;
}

#[tokio::test]
#[should_panic(expected = "got Closed")]
async fn expect_event_on_names_closed_when_the_server_ended_the_stream() {
    let mut sub = open_closing_after(String::new()).await;

    sub.expect_event_on("tick", PUSH).await;
}

#[tokio::test]
async fn expect_event_dumps_the_attached_service_log_on_a_closed_stream() {
    let proc = spawn_logging_service().await;
    let mut sub = open_closing_after(String::new()).await.with_logs(&proc);

    let message = panic_message(async move {
        sub.expect_event("a tick", PUSH).await;
    })
    .await;

    assert!(message.contains("got Closed"), "{message}");
    assert!(message.contains("service log tail"), "{message}");
    assert!(message.contains(SERVICE_MARKER), "{message}");
    proc.shutdown().await;
}

#[tokio::test]
async fn expect_silence_dumps_the_attached_service_log_on_a_closed_stream() {
    let proc = spawn_logging_service().await;
    let mut sub = open_closing_after(String::new()).await.with_logs(&proc);

    let message = panic_message(async move {
        sub.expect_silence("no push", QUIET).await;
    })
    .await;

    assert!(
        message.contains("the server closed the stream"),
        "{message}"
    );
    assert!(message.contains(SERVICE_MARKER), "{message}");
    proc.shutdown().await;
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

#[tokio::test]
async fn drain_outcome_counts_every_pushed_event_then_reports_the_stream_end() {
    let mut sub = open_closing_after(next_frames(3)).await;

    let (drained, stop) = sub.drain_outcome(10, PUSH).await;

    assert_eq!(
        drained, 3,
        "drain must count every `next` push the source emitted"
    );
    assert_eq!(
        stop,
        DrainStop::Closed,
        "a drain that ran out of stream must say so"
    );
}

#[tokio::test]
async fn drain_outcome_stops_at_max_without_consuming_the_rest() {
    let mut sub = open_closing_after(next_frames(5)).await;

    let (first, stop) = sub.drain_outcome(2, PUSH).await;
    assert_eq!(first, 2, "drain must stop once `max` is reached");
    assert_eq!(stop, DrainStop::Limit);

    let (rest, stop) = sub.drain_outcome(10, PUSH).await;
    assert_eq!(
        rest, 3,
        "the events past `max` must remain for the next read"
    );
    assert_eq!(stop, DrainStop::Closed);
}

#[tokio::test]
async fn drain_outcome_reports_a_timeout_on_a_stream_that_stays_open_and_quiet() {
    let mut sub = open_silent().await;

    let (drained, stop) = sub.drain_outcome(10, QUIET).await;

    assert_eq!(drained, 0, "a quiet stream drains nothing");
    assert_eq!(
        stop,
        DrainStop::Timeout,
        "an open stream that went quiet must not be reported as ended"
    );
}

#[tokio::test]
async fn drain_keeps_returning_the_count_alone() {
    let mut sub = open_closing_after(next_frames(3)).await;

    assert_eq!(sub.drain(10, PUSH).await, 3);
}

#[tokio::test]
#[should_panic(expected = "subscription stream returned errors")]
async fn drain_panics_on_an_errors_frame_never_swallows_it() {
    let mut sub = open_closing_after(format!("{}{}", next_frames(1), error_frame())).await;

    sub.drain(10, PUSH).await;
}
