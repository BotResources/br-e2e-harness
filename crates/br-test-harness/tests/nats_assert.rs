#![cfg(feature = "nats")]

use br_test_harness::nats_assert::{await_integration_event, recreate_kv, recreate_stream};

#[test]
fn the_free_functions_are_publicly_reachable() {
    let _ = await_integration_event;
    let _ = recreate_stream;
    let _ = recreate_kv;
}

#[cfg(feature = "spawned-nats")]
mod real_infra {
    use std::time::Duration;

    use br_test_harness::nats_assert::{await_integration_event, recreate_kv, recreate_stream};
    use br_test_harness::{SpawnedNats, TestNats};

    const STREAM: &str = "CHARTER";
    const SUBJECT: &str = "charter.evt.thing.created.v1";

    #[tokio::test]
    #[ignore = "real-infra: needs `nats-server` on PATH"]
    async fn await_catches_an_event_published_after_a_clean_stream_reset() {
        let server = SpawnedNats::start().await;
        let nats = TestNats::setup_on(&server.url()).await;

        recreate_stream(nats.jetstream(), STREAM, &["charter.>"]).await;
        nats.publish_raw(SUBJECT, br#"{"id":"x"}"#.to_vec()).await;

        let event = await_integration_event(
            nats.jetstream(),
            STREAM,
            SUBJECT,
            Duration::from_secs(5),
        )
        .await
        .expect("the event published after the reset must be caught from the start of the stream");
        assert_eq!(event["id"], "x");

        nats.cleanup().await;
        server.shutdown().await;
    }

    #[tokio::test]
    #[ignore = "real-infra: needs `nats-server` on PATH"]
    async fn await_times_out_to_none_when_no_event_matches() {
        let server = SpawnedNats::start().await;
        let nats = TestNats::setup_on(&server.url()).await;

        recreate_stream(nats.jetstream(), STREAM, &["charter.>"]).await;

        let missed = await_integration_event(
            nats.jetstream(),
            STREAM,
            SUBJECT,
            Duration::from_millis(300),
        )
        .await;
        assert!(
            missed.is_none(),
            "an empty stream must time out to None, not hang"
        );

        nats.cleanup().await;
        server.shutdown().await;
    }

    #[tokio::test]
    #[ignore = "real-infra: needs `nats-server` on PATH"]
    async fn recreate_stream_drops_prior_messages_each_call() {
        let server = SpawnedNats::start().await;
        let nats = TestNats::setup_on(&server.url()).await;

        recreate_stream(nats.jetstream(), STREAM, &["charter.>"]).await;
        nats.publish_raw(SUBJECT, br#"{"id":"stale"}"#.to_vec())
            .await;

        recreate_stream(nats.jetstream(), STREAM, &["charter.>"]).await;

        let after = await_integration_event(
            nats.jetstream(),
            STREAM,
            SUBJECT,
            Duration::from_millis(300),
        )
        .await;
        assert!(
            after.is_none(),
            "delete-then-create must drop the pre-reset message, not retain it"
        );

        nats.cleanup().await;
        server.shutdown().await;
    }

    #[tokio::test]
    #[ignore = "real-infra: needs `nats-server` on PATH"]
    async fn recreate_kv_yields_a_usable_empty_store_each_call() {
        let server = SpawnedNats::start().await;
        let nats = TestNats::setup_on(&server.url()).await;

        let store = recreate_kv(nats.jetstream(), "charter").await;
        store
            .put("k", b"v".to_vec().into())
            .await
            .expect("the recreated bucket must accept a write");

        let store = recreate_kv(nats.jetstream(), "charter").await;
        let entry = store.get("k").await.expect("get on the recreated bucket");
        assert!(
            entry.is_none(),
            "delete-then-create must hand back an empty bucket, not the prior contents"
        );

        nats.cleanup().await;
        server.shutdown().await;
    }
}
