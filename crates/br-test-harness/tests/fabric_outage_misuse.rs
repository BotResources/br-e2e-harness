#![cfg(feature = "nats-fabric")]

#[path = "fabric_outage/support.rs"]
mod support;

use br_test_harness::FabricTestNats;
use futures_util::FutureExt as _;
use support::{created, deleted, envelope, panic_message, renamed};

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

#[tokio::test(flavor = "multi_thread")]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_whole_stream_outage_nested_in_another_panics_instead_of_losing_the_binding() {
    let nats = FabricTestNats::start().await;

    let outage = nats.withhold_event_stream().await;

    let misuse = std::panic::AssertUnwindSafe(nats.withhold_event_stream())
        .catch_unwind()
        .await;
    let message = panic_message(misuse.err().expect(
        "a second whole-stream outage would record the placeholder as the binding to restore, \
         so it must fail loud",
    ));
    assert!(
        message.contains("already binds the withheld placeholder"),
        "the panic must name the already-withheld stream, got: {message}"
    );

    outage.restore().await;
    nats.fabric()
        .publish_event(&created(), &envelope())
        .await
        .expect("restore() put the real binding back, not the placeholder");

    nats.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_coordinate_listed_twice_in_keep_panics_before_the_broker_sees_it() {
    let nats = FabricTestNats::start().await;
    let (withheld, kept) = (created(), renamed());

    let misuse =
        std::panic::AssertUnwindSafe(nats.withhold_event_subject(&withheld, &[&kept, &kept]))
            .catch_unwind()
            .await;
    let message = panic_message(
        misuse
            .err()
            .expect("a duplicated `keep` coordinate must be caught as misuse, not sent to NATS"),
    );
    assert!(
        message.contains("appears twice in `keep`"),
        "the panic must name the duplicate, got: {message}"
    );

    nats.fabric()
        .publish_event(&withheld, &envelope())
        .await
        .expect("a rejected misuse must leave the stream binding exactly as it was");

    nats.shutdown().await;
}
