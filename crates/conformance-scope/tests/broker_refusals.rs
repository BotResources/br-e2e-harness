#[path = "common/short_timeout.rs"]
mod short_timeout;
#[path = "common/uniform.rs"]
mod uniform;

use std::time::Duration;

use conformance_scope::{AcceptorBehavior, AttachTarget, ConformanceError, Scenario, run_attach};
use short_timeout::TIMEOUT;
use uniform::uniform;

#[tokio::test]
async fn attach_to_an_unreachable_broker_is_a_typed_error_not_a_panic() {
    let target = AttachTarget {
        nats_url: "nats://127.0.0.1:1".to_string(),
        readyz_url: "http://127.0.0.1:1/readyz".to_string(),
    };
    let started = std::time::Instant::now();
    let result = run_attach(
        &target,
        &uniform(),
        &AcceptorBehavior::Accept,
        &[Scenario::DeclareWellFormed],
        TIMEOUT,
    )
    .await;
    match result {
        Err(ConformanceError::Jetstream(detail)) => {
            assert!(detail.contains("nats://127.0.0.1:1"), "{detail}");
        }
        other => panic!("expected a typed Jetstream error: {other:?}"),
    }
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "{:?}",
        started.elapsed()
    );
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn attach_to_a_broker_without_the_handshake_stream_fails_loud_and_provisions_nothing() {
    let bare = br_test_harness::BareFabricNats::without_fixed_streams().await;
    let target = AttachTarget {
        nats_url: bare.url(),
        readyz_url: "http://127.0.0.1:1/readyz".to_string(),
    };
    let result = run_attach(
        &target,
        &uniform(),
        &AcceptorBehavior::Accept,
        &[Scenario::DeclareWellFormed],
        TIMEOUT,
    )
    .await;
    match result {
        Err(ConformanceError::Jetstream(detail)) => {
            assert!(detail.contains("must already exist"), "{detail}");
        }
        other => panic!("expected a typed Jetstream error: {other:?}"),
    }
    assert!(
        bare.command_stream_absent().await,
        "attach mode must not provision the command stream"
    );
    assert!(
        bare.event_stream_absent().await,
        "attach mode must not provision the event stream"
    );
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn attach_to_a_broker_with_only_the_command_stream_is_refused_and_creates_no_event_stream() {
    let bare = br_test_harness::BareFabricNats::with_only_command_stream().await;
    let target = AttachTarget {
        nats_url: bare.url(),
        readyz_url: "http://127.0.0.1:1/readyz".to_string(),
    };
    let result = run_attach(
        &target,
        &uniform(),
        &AcceptorBehavior::Accept,
        &[Scenario::DeclareWellFormed],
        TIMEOUT,
    )
    .await;
    match result {
        Err(ConformanceError::Jetstream(detail)) => {
            assert!(detail.contains("must already exist"), "{detail}");
        }
        other => panic!("expected a typed Jetstream error: {other:?}"),
    }
    assert!(
        bare.event_stream_absent().await,
        "attach mode must not provision the event stream"
    );
}
