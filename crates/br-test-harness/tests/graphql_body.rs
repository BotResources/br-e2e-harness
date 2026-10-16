#![cfg(all(feature = "graphql", feature = "passport"))]

use br_test_harness::{GraphqlClient, PassportBuilder};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn serve_raw(response: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let base = format!("http://{}", listener.local_addr().expect("local addr"));
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut request = Vec::new();
                let mut chunk = [0u8; 1024];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    match socket.read(&mut chunk).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => request.extend_from_slice(&chunk[..n]),
                    }
                }
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });
    base
}

async fn serve_cut_off_body() -> String {
    serve_raw(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\nconnection: close\r\ncontent-length: 500\r\n\r\n{\"data\":",
    )
    .await
}

#[tokio::test]
#[should_panic(expected = "response body read failed")]
async fn a_cut_off_body_panics_in_authenticated_queries() {
    let client = GraphqlClient::new(&serve_cut_off_body().await);

    client
        .query(&PassportBuilder::new().build(), "{ a }", json!({}))
        .await;
}

#[tokio::test]
#[should_panic(expected = "response body read failed")]
async fn a_cut_off_body_panics_in_unauthenticated_queries() {
    let client = GraphqlClient::new(&serve_cut_off_body().await);

    client.query_unauthenticated("{ a }", json!({})).await;
}

#[tokio::test]
#[should_panic(expected = "response body read failed")]
async fn a_cut_off_body_panics_in_queries_with_a_raw_passport_header() {
    let client = GraphqlClient::new(&serve_cut_off_body().await);

    client
        .query_with_passport_header("forged", "{ a }", json!({}))
        .await;
}

#[tokio::test]
#[should_panic(expected = "response body read failed")]
async fn a_cut_off_body_panics_in_raw_posts() {
    let client = GraphqlClient::new(&serve_cut_off_body().await);

    client.post_raw("/graphql", &[], json!({})).await;
}

#[tokio::test]
#[should_panic(expected = "response body read failed")]
async fn a_cut_off_body_panics_in_raw_gets() {
    let client = GraphqlClient::new(&serve_cut_off_body().await);

    client.get_raw("/anything").await;
}

#[tokio::test]
#[should_panic(expected = "graphql response not JSON")]
async fn a_complete_non_json_body_is_reported_as_not_json_by_authenticated_queries() {
    let client = GraphqlClient::new(
        &serve_raw("HTTP/1.1 502 Bad Gateway\r\nconnection: close\r\ncontent-length: 3\r\n\r\nbad")
            .await,
    );

    client
        .query(&PassportBuilder::new().build(), "{ a }", json!({}))
        .await;
}

#[tokio::test]
async fn a_complete_body_is_returned_as_sent() {
    let client = GraphqlClient::new(
        &serve_raw("HTTP/1.1 200 OK\r\nconnection: close\r\ncontent-length: 16\r\n\r\n{\"data\":{\"a\":1}}").await,
    );

    let (status, body) = client.query_unauthenticated("{ a }", json!({})).await;
    let (_, raw) = client.get_raw("/anything").await;

    assert_eq!(status, 200);
    assert_eq!(body["data"]["a"], 1);
    assert_eq!(raw, "{\"data\":{\"a\":1}}");
}
