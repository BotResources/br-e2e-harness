#![cfg(feature = "nats-fabric")]

use std::time::Duration;

use br_core_integration::{Actor, Aggregate, Bc, CommandCoords, UserId, Verb};
use br_scope_declaration_contract::{accepted_event_coords, declare_command_coords};
use br_test_harness::{DurableConfig, FabricTestNats, FixedStream, wait_until};
use br_util_nats_fabric::{EventMetadata, IntegrationCommand, IntegrationEvent, command_subject};
use uuid::Uuid;

fn metadata() -> EventMetadata {
    EventMetadata::new(Actor::Human(UserId::from(Uuid::now_v7())), Uuid::now_v7())
}

fn command(kind: &str) -> IntegrationCommand<serde_json::Value> {
    IntegrationCommand::new(
        Uuid::now_v7(),
        kind,
        1,
        chrono::Utc::now(),
        metadata(),
        serde_json::json!({ "kind": kind }),
    )
}

fn event(fact: &str) -> IntegrationEvent<serde_json::Value> {
    IntegrationEvent::new(
        Uuid::now_v7(),
        fact,
        1,
        chrono::Utc::now(),
        metadata(),
        serde_json::json!({ "fact": fact }),
    )
}

fn sibling_command_coords() -> CommandCoords {
    CommandCoords {
        receiver: Bc::new("identity").expect("sibling receiver segment"),
        aggregate: Aggregate::new("service_scope").expect("sibling aggregate segment"),
        verb: Verb::new("retract").expect("sibling verb segment"),
        version: 1,
    }
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_finite_delivery_budget_stops_redelivering_once_exhausted() {
    let harness = FabricTestNats::start().await;
    let coords = declare_command_coords().expect("declare command coords");
    let durable = harness.durable("budgeted");
    harness
        .provision_command_durable_with(
            &coords,
            &durable,
            &DurableConfig::harness()
                .max_deliver(2)
                .ack_wait(Duration::from_millis(300)),
        )
        .await;

    harness
        .fabric()
        .publish_command(&coords, &command("declare"))
        .await
        .expect("publish the command the budgeted durable will keep redelivering");

    let mut tap = harness.tap_durable(FixedStream::Cmd, &durable).await;
    let deliveries = tap.deliveries_within(Duration::from_secs(3), 5).await;
    tap.drain().await;

    assert_eq!(
        deliveries
            .iter()
            .map(|delivery| delivery.delivered_count)
            .collect::<Vec<_>>(),
        vec![1, 2],
        "a never-acking handler must see exactly max_deliver deliveries and no more"
    );
    assert_eq!(
        harness.consumer_delivered(FixedStream::Cmd, &durable).await,
        2
    );
    assert_eq!(
        harness.consumer_pending(FixedStream::Cmd, &durable).await,
        0,
        "the exhausted command leaves the consumer with nothing pending"
    );
    assert_eq!(
        harness.command_stream_len().await,
        1,
        "an exhausted delivery budget drops the delivery, never the stored message"
    );

    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn the_counters_move_from_pending_to_delivered() {
    let harness = FabricTestNats::start().await;
    let coords = declare_command_coords().expect("declare command coords");
    let durable = harness.durable("counted");
    harness.provision_command_durable(&coords, &durable).await;

    for kind in ["declare", "declare"] {
        harness
            .fabric()
            .publish_command(&coords, &command(kind))
            .await
            .expect("publish a command onto the counted coordinate");
    }

    assert_eq!(harness.command_stream_len().await, 2);
    assert_eq!(
        harness.consumer_pending(FixedStream::Cmd, &durable).await,
        2
    );
    assert_eq!(
        harness.consumer_delivered(FixedStream::Cmd, &durable).await,
        0,
        "nothing is delivered until something pulls"
    );

    let mut tap = harness.tap_durable(FixedStream::Cmd, &durable).await;
    tap.next_within(Duration::from_secs(3))
        .await
        .expect("the tap must pull the first command");

    assert!(
        wait_until(Duration::from_secs(3), || async {
            harness.consumer_delivered(FixedStream::Cmd, &durable).await >= 1
        })
        .await,
        "delivered must move once the tap pulled a frame"
    );
    assert_eq!(
        harness.command_stream_len().await,
        2,
        "delivery without ack never removes a message from the stream"
    );

    tap.drain().await;
    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn purge_empties_a_whole_stream_or_a_single_subject() {
    let harness = FabricTestNats::start().await;
    let declare = declare_command_coords().expect("declare command coords");
    let sibling = sibling_command_coords();
    let accepted = accepted_event_coords().expect("accepted event coords");

    for kind in ["declare", "declare"] {
        harness
            .fabric()
            .publish_command(&declare, &command(kind))
            .await
            .expect("publish onto the declare coordinate");
    }
    harness
        .fabric()
        .publish_command(&sibling, &command("retract"))
        .await
        .expect("publish onto the sibling coordinate");
    harness
        .fabric()
        .publish_event(&accepted, &event("accepted"))
        .await
        .expect("publish onto the accepted coordinate");

    assert_eq!(harness.command_stream_len().await, 3);
    assert_eq!(harness.event_stream_len().await, 1);

    assert_eq!(harness.purge_command_subject(&declare).await, 2);
    assert_eq!(
        harness.command_stream_len().await,
        1,
        "a subject purge spares the sibling coordinate on the same stream"
    );
    assert_eq!(harness.event_stream_len().await, 1);

    assert_eq!(harness.purge_command_stream().await, 1);
    assert_eq!(harness.command_stream_len().await, 0);
    assert_eq!(
        harness.event_stream_len().await,
        1,
        "purging one fixed stream never touches the other"
    );

    assert_eq!(harness.purge_event_subject(&accepted).await, 1);
    assert_eq!(harness.event_stream_len().await, 0);
    assert_eq!(harness.purge_event_stream().await, 0);

    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn malformed_command_bytes_reach_a_lib_consumer_that_terms_them() {
    let harness = FabricTestNats::start().await;
    let coords = declare_command_coords().expect("declare command coords");
    let durable = harness.durable("poisoned");

    harness
        .publish_command_raw(&coords, b"{ this is not an integration command")
        .await;
    assert_eq!(harness.command_stream_len().await, 1);

    let mut consumer = harness
        .fabric()
        .ensure_command_consumer::<serde_json::Value>(&coords, &durable)
        .await
        .expect("the lib consumer binds the coordinate the raw bytes landed on");
    let delivered = consumer
        .recv()
        .await
        .expect("the lib consumer must receive the malformed frame")
        .expect("the malformed frame must not close the stream");

    let error = delivered
        .payload()
        .expect_err("malformed wire bytes must fail the envelope decode")
        .to_string();
    assert!(
        error.contains(&command_subject(&coords)),
        "the decode failure must name the coordinate it refused: {error}"
    );
    delivered
        .term()
        .await
        .expect("the fail-closed path terminates the poison frame");

    assert!(
        wait_until(Duration::from_secs(3), || async {
            harness.consumer_pending(FixedStream::Cmd, &durable).await == 0
        })
        .await,
        "a terminated frame is never redelivered"
    );
    assert_eq!(
        harness.command_stream_len().await,
        1,
        "Term stops redelivery; it never removes the stored message"
    );

    harness.shutdown().await;
}
