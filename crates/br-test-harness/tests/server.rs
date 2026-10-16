#![cfg(feature = "server")]

use axum::{Router, routing::get};
use br_test_harness::TestServer;

fn boom() -> &'static str {
    panic!("boom")
}

#[tokio::test]
async fn spawn_returns_the_base_url_of_a_router_that_answers() {
    let server = TestServer::spawn(Router::new().route("/", get(|| async { "ok" }))).await;

    assert!(server.base_url.starts_with("http://127.0.0.1:"));
}

#[tokio::test]
#[should_panic(expected = "did not answer within 5s")]
async fn spawn_panics_when_the_server_never_answers() {
    let _ = TestServer::spawn(Router::new().route("/", get(|| async { boom() }))).await;
}

#[tokio::test]
#[should_panic(expected = "did not answer within 5s")]
async fn spawn_panics_when_the_handler_never_resolves() {
    let _ = TestServer::spawn(Router::new().route(
        "/",
        get(|| async {
            std::future::pending::<()>().await;
            "never"
        }),
    ))
    .await;
}
