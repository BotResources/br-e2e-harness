#![cfg(all(feature = "sse", feature = "server", feature = "passport"))]

use std::time::Duration;

use axum::Router;
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::post;
use br_test_harness::{PassportBuilder, SseSubscription, TestServer};

fn next_frames(count: usize) -> String {
    (0..count)
        .map(|i| format!("event: next\ndata: {{\"data\":{{\"tick\":{i}}}}}\n\n"))
        .collect()
}

fn error_frame() -> String {
    "event: next\ndata: {\"errors\":[{\"message\":\"boom\"}]}\n\n".to_string()
}

async fn serve(body: String) -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/event-stream")], body)
}

async fn open_against(body: String) -> SseSubscription {
    let frames = body.clone();
    let server = TestServer::spawn(Router::new().route(
        "/graphql",
        post(move || {
            let frames = frames.clone();
            async move { serve(frames).await }
        }),
    ))
    .await;
    SseSubscription::open(
        &server.base_url,
        &PassportBuilder::new().build(),
        "subscription { tick }",
    )
    .await
}

#[tokio::test]
async fn drain_counts_every_pushed_event_then_stops_on_stream_end() {
    let mut sub = open_against(next_frames(3)).await;

    let drained = sub.drain(10, Duration::from_secs(2)).await;

    assert_eq!(
        drained, 3,
        "drain must count every `next` push the source emitted"
    );
}

#[tokio::test]
async fn drain_stops_at_max_without_consuming_the_rest() {
    let mut sub = open_against(next_frames(5)).await;

    let first = sub.drain(2, Duration::from_secs(2)).await;
    assert_eq!(first, 2, "drain must stop once `max` is reached");

    let rest = sub.drain(10, Duration::from_secs(2)).await;
    assert_eq!(
        rest, 3,
        "the events past `max` must remain for the next read"
    );
}

#[tokio::test]
async fn drain_returns_zero_on_silence() {
    let mut sub = open_against(String::new()).await;

    let drained = sub.drain(10, Duration::from_millis(200)).await;

    assert_eq!(drained, 0, "a silent stream drains nothing");
}

#[tokio::test]
#[should_panic(expected = "subscription stream returned errors")]
async fn drain_panics_on_an_errors_frame_never_swallows_it() {
    let mut sub = open_against(format!("{}{}", next_frames(1), error_frame())).await;

    sub.drain(10, Duration::from_secs(2)).await;
}
