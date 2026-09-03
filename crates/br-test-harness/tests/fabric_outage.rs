#![cfg(feature = "nats-fabric")]

#[path = "fabric_outage/support.rs"]
mod support;

use std::time::Duration;

use br_test_harness::FabricTestNats;
use br_util_nats_fabric::{EventConsumer, INTEGRATION_EVT, command_subject, event_subject};
use serde_json::Value;
use support::{
    assert_no_stream, command_envelope, created, deleted, deliver, envelope, marked_envelope,
    marker_of, renamed,
};

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
async fn an_outage_is_a_narrowing_so_a_coordinate_absent_from_keep_also_stops() {
    let nats = FabricTestNats::start().await;
    let (withheld, kept, unlisted) = (created(), renamed(), deleted());

    let outage = nats.withhold_event_subject(&withheld, &[&kept]).await;

    for coords in [&withheld, &unlisted] {
        let err = nats
            .fabric()
            .publish_event(coords, &envelope())
            .await
            .expect_err("only the coordinates listed in `keep` survive the narrowing");
        assert_no_stream(err, &event_subject(coords));
    }

    nats.fabric()
        .publish_event(&kept, &envelope())
        .await
        .expect("the one listed coordinate is still stored");

    outage.restore().await;

    nats.fabric()
        .publish_event(&unlisted, &envelope())
        .await
        .expect("the unlisted coordinate flows again once the binding is restored");

    nats.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_durable_keeps_its_position_and_the_stream_keeps_its_messages_across_an_outage() {
    let nats = FabricTestNats::start().await;
    let (withheld, kept) = (created(), renamed());
    let durable = nats.durable("outage_reader");
    let subject = event_subject(&withheld);

    let mut consumer = nats
        .fabric()
        .ensure_event_consumer::<Value>(&withheld, &durable)
        .await
        .expect("bind a durable on the withheld coordinate before the outage");

    nats.fabric()
        .publish_event(&withheld, &marked_envelope("before"))
        .await
        .expect("a message stored before the outage");

    let outage = nats.withhold_event_subject(&withheld, &[&kept]).await;
    let err = nats
        .fabric()
        .publish_event(&withheld, &marked_envelope("during"))
        .await
        .expect_err("the withheld coordinate is covered by no stream during the outage");
    assert_no_stream(err, &subject);
    assert!(
        !nats.raw_message_absent(INTEGRATION_EVT, &subject).await,
        "narrowing the binding must not delete the messages already stored on the subject"
    );
    outage.restore().await;

    nats.fabric()
        .publish_event(&withheld, &marked_envelope("after"))
        .await
        .expect("the coordinate publishes again after restore");

    assert_eq!(
        [
            recv(&mut consumer, &subject).await,
            recv(&mut consumer, &subject).await
        ],
        ["before", "after"],
        "the durable resumes at its own position: the pre-outage message is delivered first \
         and the post-restore one second, on the handle bound before the outage — so the \
         narrowing neither dropped a stored message nor reset the consumer"
    );
    assert!(
        tokio::time::timeout(Duration::from_secs(2), consumer.recv())
            .await
            .is_err(),
        "nothing is left pending: the withheld publish stored no message at all"
    );

    assert_eq!(
        nats.durable_filter_subjects(INTEGRATION_EVT, &durable)
            .await,
        vec![subject],
        "the outage never touched the durable's filter, only the stream binding"
    );

    nats.shutdown().await;
}

async fn recv(consumer: &mut EventConsumer<Value>, subject: &str) -> String {
    let delivered = tokio::time::timeout(Duration::from_secs(10), consumer.recv())
        .await
        .expect("the durable delivers within the deadline")
        .expect("the durable pull loop is healthy across the outage")
        .expect("a message is delivered, not an empty batch");
    assert_eq!(delivered.subject(), subject);
    assert_eq!(
        delivered.delivered_count(),
        Some(1),
        "a first delivery, never a redelivery: the outage caused no ack churn"
    );
    let marker = marker_of(delivered.payload().expect("the payload decodes")).to_string();
    delivered.ack().await.expect("ack the delivered message");
    marker
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
