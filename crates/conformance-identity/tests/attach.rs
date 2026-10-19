use std::time::Duration;

use br_test_harness::{FabricTestNats, SpawnedNats, wait_until};
use conformance_identity::{
    AttachTarget, ConformanceError, IdentityHarness, ReadyzProbe, Subject, SubjectConfig,
    attach_default, locate_subject, provision, run_attach,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const SHORT: Duration = Duration::from_secs(10);

async fn stub_readyz_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("address").port();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut request = [0u8; 1024];
                let _ = socket.read(&mut request).await;
                let _ = socket
                    .write_all(
                        b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .await;
            });
        }
    });
    format!("http://127.0.0.1:{port}/readyz")
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn attach_to_a_bare_broker_is_a_typed_error_and_creates_no_stream() {
    let nats = SpawnedNats::start().await;
    let url = nats.url();
    let target = AttachTarget {
        nats_url: url.clone(),
        readyz_url: stub_readyz_server().await,
    };

    let result = run_attach(&target, &attach_default(), SHORT).await;
    match result {
        Err(ConformanceError::Jetstream(detail)) => assert!(
            detail.contains("INTEGRATION_CMD") && detail.contains("INTEGRATION_EVT"),
            "the error must name the missing fixed streams: {detail}"
        ),
        other => panic!("a bare broker must be a typed error: {other:?}"),
    }

    let probe = FabricTestNats::attach_without_provisioning(&url).await;
    assert!(
        !probe.fixed_streams_present().await,
        "run_attach must not create the fixed streams on the broker it attaches to"
    );
    probe.shutdown().await;
    nats.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `identity-acceptor` on PATH"]
async fn attach_to_a_provisioned_broker_with_a_live_acceptor_is_conformant() {
    let harness = IdentityHarness::start().await.expect("harness");
    provision(&harness.nats_url(), "scope_declaration.toml")
        .await
        .expect("fabric-nats provision");
    let binary = locate_subject().await.expect("acceptor binary");
    let subject = Subject::spawn(&binary, &SubjectConfig::new(&harness.nats_url()));
    let readyz = ReadyzProbe::new(format!("{}/readyz", subject.base_url())).expect("readyz");
    assert!(
        wait_until(SHORT, || async { readyz.is_ready().await }).await,
        "acceptor must reach /readyz=200; logs:\n{}",
        subject.logs()
    );

    let target = AttachTarget {
        nats_url: harness.nats_url(),
        readyz_url: format!("{}/readyz", subject.base_url()),
    };
    let report = run_attach(&target, &attach_default(), SHORT)
        .await
        .expect("attach run");

    subject.shutdown().await;
    harness.shutdown().await;
    assert!(report.is_conformant(), "{report:?}");
    assert_eq!(report.passed(), 7);
}
