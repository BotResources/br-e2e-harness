#![cfg(feature = "nats-fabric")]

mod fabric_support;

use std::time::Duration;

use br_scope_declaration_contract::{accepted_event_coords, declare_command_coords};
use br_test_harness::{
    DurableConfig, FabricTestNats, FixedStream, TapOutcome, TapStop, wait_until,
};
use br_util_nats_fabric::event_subject;
use fabric_support::{command, event};

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
    let (deliveries, stop) = tap.deliveries_within(Duration::from_secs(3), 5).await;
    tap.close();

    assert_eq!(
        stop,
        TapStop::Timeout,
        "the tap must still hold a live consumer when the redeliveries stop, or the absence of a third delivery proves nothing"
    );
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
        harness
            .consumer_redelivered(FixedStream::Cmd, &durable)
            .await,
        1,
        "num_redelivered counts the deliveries past the first, so an exhausted budget of 2 leaves 1"
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
    tap.expect_delivery("the first command", Duration::from_secs(3))
        .await;

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

    tap.close();
    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn an_event_durable_taps_the_coordinate_it_was_provisioned_with() {
    let harness = FabricTestNats::start().await;
    let coords = accepted_event_coords().expect("accepted event coords");
    let durable = harness.durable("tapped_events");
    harness
        .provision_event_durable_with(
            &coords,
            &durable,
            &DurableConfig::default().ack_wait(Duration::from_secs(1)),
        )
        .await;

    harness
        .fabric()
        .publish_event(&coords, &event("accepted"))
        .await
        .expect("publish onto the tapped event coordinate");
    assert_eq!(harness.event_stream_len().await, 1);
    assert_eq!(
        harness.consumer_pending(FixedStream::Evt, &durable).await,
        1
    );

    let mut tap = harness.tap_durable(FixedStream::Evt, &durable).await;
    let delivery = tap
        .expect_delivery("the published event", Duration::from_secs(3))
        .await;
    tap.close();

    assert_eq!(delivery.subject, event_subject(&coords));
    assert_eq!(delivery.delivered_count, 1);
    assert_eq!(
        harness.consumer_delivered(FixedStream::Evt, &durable).await,
        1
    );
    assert_eq!(
        harness
            .consumer_redelivered(FixedStream::Evt, &durable)
            .await,
        0,
        "a first delivery is not a redelivery"
    );

    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_tap_that_lost_its_consumer_reads_as_closed_never_as_quiet() {
    let harness = FabricTestNats::start().await;
    let coords = declare_command_coords().expect("declare command coords");
    let durable = harness.durable("vanishing");
    harness.provision_command_durable(&coords, &durable).await;

    let mut tap = harness.tap_durable(FixedStream::Cmd, &durable).await;
    assert_eq!(
        tap.next_within(Duration::from_millis(500)).await,
        TapOutcome::Timeout,
        "a live tap on an empty coordinate is quiet"
    );

    harness.delete_durable(FixedStream::Cmd, &durable).await;

    assert_eq!(
        tap.next_within(Duration::from_secs(5)).await,
        TapOutcome::Closed,
        "a tap whose durable was deleted has stopped observing and must not read as quiet"
    );
    let (deliveries, stop) = tap.deliveries_within(Duration::from_secs(1), 5).await;
    assert!(deliveries.is_empty());
    assert_eq!(
        stop,
        TapStop::Closed,
        "a collection loop on a dead tap reports Closed, never a successful exhaustion"
    );

    tap.close();
    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn expect_quiet_passes_on_a_live_tap_and_panics_once_the_consumer_is_gone() {
    let harness = FabricTestNats::start().await;
    let coords = declare_command_coords().expect("declare command coords");
    let durable = harness.durable("quiet_then_gone");
    harness.provision_command_durable(&coords, &durable).await;

    let mut tap = harness.tap_durable(FixedStream::Cmd, &durable).await;
    tap.expect_quiet("nothing published yet", Duration::from_millis(500))
        .await;

    harness.delete_durable(FixedStream::Cmd, &durable).await;
    let message = panic_message(async move {
        tap.expect_quiet("nothing published yet", Duration::from_secs(5))
            .await;
    })
    .await;

    assert!(
        message.contains(&durable) && message.contains("not quiet"),
        "a tap that lost its consumer must fail loud naming the durable, got: {message}"
    );

    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn expect_delivery_names_the_expectation_and_the_durable_on_timeout() {
    let harness = FabricTestNats::start().await;
    let coords = declare_command_coords().expect("declare command coords");
    let durable = harness.durable("awaited");
    harness.provision_command_durable(&coords, &durable).await;

    let tap = harness.tap_durable(FixedStream::Cmd, &durable).await;
    let message = panic_message(async move {
        let mut tap = tap;
        tap.expect_delivery("the command nobody published", Duration::from_millis(500))
            .await;
    })
    .await;

    assert!(
        message.contains("the command nobody published")
            && message.contains("got Timeout")
            && message.contains(&durable),
        "a missed delivery must name the expectation and the durable, got: {message}"
    );

    harness.shutdown().await;
}

async fn panic_message<F>(future: F) -> String
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let payload = tokio::spawn(future)
        .await
        .expect_err("the future under test was expected to panic")
        .into_panic();
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_string()))
        .expect("the panic payload must be a string")
}
