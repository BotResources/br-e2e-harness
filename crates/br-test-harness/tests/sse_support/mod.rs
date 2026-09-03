use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::post;
use br_test_harness::{PassportBuilder, SseSubscription, TestServer};
use bytes::Bytes;

pub const QUIET: Duration = Duration::from_millis(300);
pub const PUSH: Duration = Duration::from_secs(2);

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

pub async fn open_closing_after(body: String) -> SseSubscription {
    open_against(Router::new().route(
        "/graphql",
        post(move || {
            let frames = body.clone();
            async move { serve_finite(frames).await }
        }),
    ))
    .await
}

pub async fn open_silent() -> SseSubscription {
    open_against(Router::new().route("/graphql", post(serve_open_forever))).await
}
