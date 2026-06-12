use std::time::Duration;

use br_test_harness::SpawnedNats;
use br_test_harness::nats::connect;
use futures_util::StreamExt as _;

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn spawns_a_server_a_client_can_round_trip_against_its_reported_port() {
    let server = SpawnedNats::start().await;
    let url = server.url();

    assert!(
        url.ends_with(&format!(":{}", server.port())),
        "the reported url must carry the bound port: {url}"
    );

    let client = connect(&url)
        .await
        .expect("a client must connect to the spawned server's reported port");

    let mut sub = client
        .subscribe("harness.selftest")
        .await
        .expect("subscribe must succeed");
    client
        .publish("harness.selftest", "ping".into())
        .await
        .expect("publish must succeed");
    client.flush().await.expect("flush must succeed");

    let msg = tokio::time::timeout(Duration::from_secs(5), sub.next())
        .await
        .expect("the published message must arrive before the deadline")
        .expect("the subscription must yield the message");
    assert_eq!(msg.payload.as_ref(), b"ping");

    drop(client);
    server.shutdown().await;

    let addr = format!("127.0.0.1:{}", server_port_from(&url));
    let after = tokio::time::timeout(Duration::from_secs(5), wait_until_refused(&addr)).await;
    assert!(
        after.is_ok(),
        "the spawned server must stop accepting once shut down — a leaked process keeps {addr} open"
    );
}

fn server_port_from(url: &str) -> u16 {
    url.rsplit(':')
        .next()
        .and_then(|p| p.parse().ok())
        .expect("url must end with a port")
}

async fn wait_until_refused(addr: &str) {
    loop {
        if tokio::net::TcpStream::connect(addr).await.is_err() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
