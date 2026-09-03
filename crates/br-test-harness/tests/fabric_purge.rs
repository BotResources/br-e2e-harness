#![cfg(feature = "nats-fabric")]

mod fabric_support;

use std::time::Duration;

use br_core_integration::{Aggregate, Bc, CommandCoords, EventCoords, PastFact, Verb};
use br_scope_declaration_contract::{accepted_event_coords, declare_command_coords};
use br_test_harness::{FabricTestNats, FixedStream, wait_until};
use br_util_nats_fabric::command_subject;
use fabric_support::{command, event};

pub fn sibling_command_coords() -> CommandCoords {
    CommandCoords {
        receiver: Bc::new("identity").expect("sibling receiver segment"),
        aggregate: Aggregate::new("service_scope").expect("sibling aggregate segment"),
        verb: Verb::new("retract").expect("sibling verb segment"),
        version: 1,
    }
}

pub fn sibling_event_coords() -> EventCoords {
    EventCoords {
        producer: Bc::new("identity").expect("sibling producer segment"),
        aggregate: Aggregate::new("service_scope").expect("sibling aggregate segment"),
        fact: PastFact::new("rejected").expect("sibling fact segment"),
        version: 1,
    }
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn purge_empties_a_whole_stream_or_a_single_subject() {
    let harness = FabricTestNats::start().await;
    let declare = declare_command_coords().expect("declare command coords");
    let sibling = sibling_command_coords();
    let accepted = accepted_event_coords().expect("accepted event coords");
    let rejected = sibling_event_coords();

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
    for fact in ["accepted", "accepted"] {
        harness
            .fabric()
            .publish_event(&accepted, &event(fact))
            .await
            .expect("publish onto the accepted coordinate");
    }
    harness
        .fabric()
        .publish_event(&rejected, &event("rejected"))
        .await
        .expect("publish onto the sibling event coordinate");

    assert_eq!(harness.command_stream_len().await, 3);
    assert_eq!(harness.event_stream_len().await, 3);

    assert_eq!(harness.purge_command_subject(&declare).await, 2);
    assert_eq!(
        harness.command_stream_len().await,
        1,
        "a subject purge spares the sibling coordinate on the same stream"
    );
    assert_eq!(harness.event_stream_len().await, 3);

    assert_eq!(harness.purge_command_stream().await, 1);
    assert_eq!(harness.command_stream_len().await, 0);
    assert_eq!(
        harness.event_stream_len().await,
        3,
        "purging one fixed stream never touches the other"
    );

    assert_eq!(harness.purge_event_subject(&accepted).await, 2);
    assert_eq!(
        harness.event_stream_len().await,
        1,
        "an event subject purge spares the sibling event coordinate"
    );

    harness
        .fabric()
        .publish_event(&accepted, &event("accepted"))
        .await
        .expect("republish onto the accepted coordinate");
    assert_eq!(harness.purge_event_stream().await, 2);
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
