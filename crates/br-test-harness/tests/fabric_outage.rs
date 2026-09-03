#![cfg(feature = "nats-fabric")]

use std::time::Duration;

use br_core_integration::{
    Aggregate, Bc, CommandCoords, EventCoords, IntegrationCommand, IntegrationEvent, PastFact, Verb,
};
use br_test_harness::FabricTestNats;
use br_util_nats_fabric::{
    FabricError, INTEGRATION_EVT, PublishErrorKind, command_subject, event_subject,
};
use futures_util::FutureExt as _;
use serde_json::Value;
use uuid::Uuid;

fn created() -> EventCoords {
    EventCoords {
        producer: Bc::new("identity").unwrap(),
        aggregate: Aggregate::new("user").unwrap(),
        fact: PastFact::new("created").unwrap(),
        version: 1,
    }
}

fn renamed() -> EventCoords {
    EventCoords {
        producer: Bc::new("identity").unwrap(),
        aggregate: Aggregate::new("user").unwrap(),
        fact: PastFact::new("renamed").unwrap(),
        version: 1,
    }
}

fn deleted() -> EventCoords {
    EventCoords {
        producer: Bc::new("identity").unwrap(),
        aggregate: Aggregate::new("user").unwrap(),
        fact: PastFact::new("deleted").unwrap(),
        version: 1,
    }
}

fn deliver() -> CommandCoords {
    CommandCoords {
        receiver: Bc::new("notifier").unwrap(),
        aggregate: Aggregate::new("notification").unwrap(),
        verb: Verb::new("deliver").unwrap(),
        version: 1,
    }
}

fn envelope() -> IntegrationEvent<Value> {
    serde_json::from_value(serde_json::json!({
        "event_id": Uuid::now_v7(),
        "event_type": "outage.probe",
        "version": 1,
        "occurred_at": "2026-09-03T00:00:00Z",
        "metadata": {
            "actor_id": Uuid::now_v7(),
            "actor_kind": "service",
            "correlation_id": Uuid::now_v7()
        },
        "payload": { "probe": "delivery-outage" }
    }))
    .expect("the probe envelope deserializes into the lib's IntegrationEvent")
}

fn command_envelope() -> IntegrationCommand<Value> {
    serde_json::from_value(serde_json::json!({
        "command_id": Uuid::now_v7(),
        "command_type": "outage.probe",
        "version": 1,
        "issued_at": "2026-09-03T00:00:00Z",
        "metadata": {
            "actor_id": Uuid::now_v7(),
            "actor_kind": "service",
            "correlation_id": Uuid::now_v7()
        },
        "payload": { "probe": "delivery-outage" }
    }))
    .expect("the probe envelope deserializes into the lib's IntegrationCommand")
}

fn assert_no_stream(err: FabricError, subject: &str) {
    assert!(
        matches!(
            &err,
            FabricError::Publish {
                kind: PublishErrorKind::NoStream,
                ..
            }
        ),
        "a publish on the withheld '{subject}' must fail Publish(NoStream), got {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_withheld_event_coordinate_fails_typed_while_a_kept_sibling_keeps_flowing() {
    let nats = FabricTestNats::start().await;
    let (withheld, kept) = (created(), renamed());

    let outage = nats.withhold_event_subject(&withheld, &[&kept]).await;
    assert_eq!(outage.stream(), INTEGRATION_EVT);
    assert_eq!(outage.withheld_subjects(), [event_subject(&withheld)]);
    assert_eq!(outage.live_subjects(), [event_subject(&kept)]);

    let err = nats
        .fabric()
        .publish_event(&withheld, &envelope())
        .await
        .expect_err("the withheld coordinate is covered by no stream during the outage");
    assert_no_stream(err, &event_subject(&withheld));

    nats.fabric()
        .publish_event(&kept, &envelope())
        .await
        .expect("a coordinate listed in `keep` is still stored during the outage");

    outage.restore().await;

    nats.fabric()
        .publish_event(&withheld, &envelope())
        .await
        .expect("restore() puts the original binding back and the coordinate publishes again");

    nats.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_durable_bound_before_the_outage_still_receives_after_restore() {
    let nats = FabricTestNats::start().await;
    let (withheld, kept) = (created(), renamed());
    let durable = nats.durable("outage_reader");

    let mut consumer = nats
        .fabric()
        .ensure_event_consumer::<Value>(&withheld, &durable)
        .await
        .expect("bind a durable on the withheld coordinate before the outage");

    let outage = nats.withhold_event_subject(&withheld, &[&kept]).await;
    let err = nats
        .fabric()
        .publish_event(&withheld, &envelope())
        .await
        .expect_err("the withheld coordinate is covered by no stream during the outage");
    assert_no_stream(err, &event_subject(&withheld));
    outage.restore().await;

    nats.fabric()
        .publish_event(&withheld, &envelope())
        .await
        .expect("the coordinate publishes again after restore");

    let delivered = tokio::time::timeout(Duration::from_secs(10), consumer.recv())
        .await
        .expect("the durable receives within the deadline after restore")
        .expect("the durable pull loop is healthy after the outage")
        .expect("a message is delivered, not an empty batch");
    assert_eq!(delivered.subject(), event_subject(&withheld));
    delivered.ack().await.expect("ack the delivered message");

    assert_eq!(
        nats.durable_filter_subjects(INTEGRATION_EVT, &durable)
            .await,
        vec![event_subject(&withheld)],
        "the outage never touched the durable's filter, only the stream binding"
    );

    nats.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn withholding_the_whole_event_stream_stops_every_coordinate() {
    let nats = FabricTestNats::start().await;
    let (one, two) = (created(), renamed());

    let outage = nats.withhold_event_stream().await;
    assert_eq!(outage.withheld_subjects(), ["integration.evt.>"]);
    assert_eq!(outage.live_subjects(), ["integration.evt.__withheld__.>"]);

    for coords in [&one, &two] {
        let err = nats
            .fabric()
            .publish_event(coords, &envelope())
            .await
            .expect_err("no event coordinate survives a whole-stream outage");
        assert_no_stream(err, &event_subject(coords));
    }

    outage.restore().await;

    for coords in [&one, &two] {
        nats.fabric()
            .publish_event(coords, &envelope())
            .await
            .expect("every coordinate publishes again after restore");
    }

    nats.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_command_stream_outage_leaves_the_event_stream_untouched() {
    let nats = FabricTestNats::start().await;
    let coords = deliver();

    let outage = nats.withhold_command_stream().await;
    assert_eq!(outage.live_subjects(), ["integration.cmd.__withheld__.>"]);

    let err = nats
        .fabric()
        .publish_command(&coords, &command_envelope())
        .await
        .expect_err("the command stream carries no coordinate during the outage");
    assert_no_stream(err, &command_subject(&coords));

    nats.fabric()
        .publish_event(&created(), &envelope())
        .await
        .expect("a command-stream outage never narrows the event stream");

    outage.restore().await;
    nats.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn withholding_a_coordinate_the_stream_no_longer_carries_panics() {
    let nats = FabricTestNats::start().await;
    let (withheld, kept, absent) = (created(), renamed(), deleted());

    let outage = nats.withhold_event_subject(&withheld, &[&kept]).await;

    let misuse = std::panic::AssertUnwindSafe(nats.withhold_event_subject(&absent, &[&kept]))
        .catch_unwind()
        .await;
    let message = panic_message(misuse.err().expect(
        "withholding a coordinate the narrowed stream no longer carries must fail loud, \
         never silently succeed",
    ));
    assert!(
        message.contains("does not cover"),
        "the panic must name the uncovered coordinate, got: {message}"
    );

    outage.restore().await;
    nats.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn keeping_the_withheld_coordinate_panics_before_the_stream_is_touched() {
    let nats = FabricTestNats::start().await;
    let withheld = created();

    let misuse = std::panic::AssertUnwindSafe(nats.withhold_event_subject(&withheld, &[&withheld]))
        .catch_unwind()
        .await;
    let message = panic_message(
        misuse
            .err()
            .expect("asking to both drop and keep one coordinate must fail loud"),
    );
    assert!(
        message.contains("listed in `keep`"),
        "the panic must name the contradiction, got: {message}"
    );

    nats.fabric()
        .publish_event(&withheld, &envelope())
        .await
        .expect("a rejected misuse must leave the stream binding exactly as it was");

    nats.shutdown().await;
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return (*message).to_string();
    }
    payload
        .downcast_ref::<String>()
        .cloned()
        .unwrap_or_else(|| "<non-string panic payload>".to_string())
}
