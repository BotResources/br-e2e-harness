#![cfg(all(feature = "graphql", feature = "passport"))]

use br_test_harness::GraphqlClient;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn serve_cut_off_body() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let base = format!("http://{}", listener.local_addr().expect("local addr"));
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut request = [0u8; 4096];
                let _ = socket.read(&mut request).await;
                let head = "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 500\r\n\r\n{\"data\":";
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });
    base
}

#[tokio::test]
#[should_panic(expected = "response body read failed")]
async fn a_cut_off_body_panics_in_unauthenticated_queries() {
    let client = GraphqlClient::new(&serve_cut_off_body().await);

    client.query_unauthenticated("{ a }", json!({})).await;
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
