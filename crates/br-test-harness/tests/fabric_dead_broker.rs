#![cfg(all(feature = "nats-fabric", feature = "spawned-nats"))]

use std::future::Future;
use std::time::{Duration, Instant};

use br_core_integration::{Aggregate, Bc, CommandCoords, EventCoords, PastFact, Verb};
use br_test_harness::{FabricTestNats, FixedStream, SpawnedNats};
use uuid::Uuid;

const PROMPT: Duration = Duration::from_secs(2);

fn event_coords() -> EventCoords {
    EventCoords {
        producer: Bc::new("notifier").unwrap(),
        aggregate: Aggregate::new("notification").unwrap(),
        fact: PastFact::new("delivered").unwrap(),
        version: 1,
    }
}

fn command_coords() -> CommandCoords {
    CommandCoords {
        receiver: Bc::new("notifier").unwrap(),
        aggregate: Aggregate::new("notification").unwrap(),
        verb: Verb::new("deliver").unwrap(),
        version: 1,
    }
}

async fn attached_to_a_broker() -> (SpawnedNats, FabricTestNats) {
    let server = SpawnedNats::start().await;
    let nats = FabricTestNats::connect(&server.url()).await;
    (server, nats)
}

fn assert_names_the_dead_connection(panic: Box<dyn std::any::Any + Send>, elapsed: Duration) {
    let message = panic
        .downcast_ref::<String>()
        .cloned()
        .expect("the panic carries a formatted message");
    assert!(
        message.starts_with("fabric capture drain failed: the NATS connection is "),
        "got {message}"
    );
    assert!(
        elapsed < PROMPT,
        "the read must fail promptly, took {elapsed:?}"
    );
}

fn panics_promptly<T>(read: impl FnOnce() -> T) {
    let started = Instant::now();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(read))
        .map(|_| ())
        .expect_err("a read on a dead broker must panic, not report nothing happened");
    assert_names_the_dead_connection(panic, started.elapsed());
}

async fn panics_promptly_async<F: Future + Send + 'static>(read: F)
where
    F::Output: Send + 'static,
{
    let started = Instant::now();
    let joined = tokio::spawn(read).await;
    let error = joined.map(|_| ()).expect_err("the read must panic");
    assert!(error.is_panic(), "the read must panic, got {error}");
    assert_names_the_dead_connection(error.into_panic(), started.elapsed());
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn event_capture_reads_panic_when_the_broker_dies() {
    let (server, nats) = attached_to_a_broker().await;
    let capture = nats.capture_events(&[&event_coords()]).await;
    server.shutdown().await;
    tokio::time::sleep(Duration::from_millis(500)).await;

    panics_promptly(|| capture.count());
    panics_promptly(|| capture.first());
    panics_promptly(|| capture.for_correlation(Uuid::now_v7()));
    panics_promptly(|| capture.correlation_ids());
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn command_capture_reads_panic_when_the_broker_dies() {
    let (server, nats) = attached_to_a_broker().await;
    let capture = nats.capture_commands(&[&command_coords()]).await;
    server.shutdown().await;
    tokio::time::sleep(Duration::from_millis(500)).await;

    panics_promptly(|| capture.count());
    panics_promptly(|| capture.first());
    panics_promptly(|| capture.for_correlation(Uuid::now_v7()));
    panics_promptly(|| capture.correlation_ids());
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn the_event_awaiter_panics_at_its_deadline_when_the_broker_dies() {
    let (server, nats) = attached_to_a_broker().await;
    let mut awaiter = nats.await_event(&event_coords()).await;
    server.shutdown().await;
    tokio::time::sleep(Duration::from_millis(500)).await;

    panics_promptly_async(async move {
        awaiter
            .await_correlation(Uuid::now_v7(), Duration::from_millis(500))
            .await
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn the_command_awaiter_panics_at_its_deadline_when_the_broker_dies() {
    let (server, nats) = attached_to_a_broker().await;
    let mut awaiter = nats.await_command(&command_coords()).await;
    server.shutdown().await;
    tokio::time::sleep(Duration::from_millis(500)).await;

    panics_promptly_async(async move {
        awaiter
            .await_correlation(Uuid::now_v7(), Duration::from_millis(500))
            .await
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_durable_tap_is_not_quiet_when_the_broker_dies() {
    let (server, nats) = attached_to_a_broker().await;
    let durable = nats.durable("dead_broker");
    nats.provision_command_durable(&command_coords(), &durable)
        .await;
    let mut tap = nats.tap_durable(FixedStream::Cmd, &durable).await;
    server.shutdown().await;
    tokio::time::sleep(Duration::from_millis(500)).await;

    panics_promptly_async(async move {
        tap.expect_quiet("nothing may arrive", Duration::from_millis(500))
            .await
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_live_broker_still_reads_quiet() {
    let (server, nats) = attached_to_a_broker().await;
    let capture = nats.capture_events(&[&event_coords()]).await;
    let mut awaiter = nats.await_event(&event_coords()).await;
    let mut commands = nats.await_command(&command_coords()).await;
    let durable = nats.durable("live");
    nats.provision_command_durable(&command_coords(), &durable)
        .await;
    let mut tap = nats.tap_durable(FixedStream::Cmd, &durable).await;

    assert_eq!(capture.count(), 0);
    assert!(
        awaiter
            .await_correlation(Uuid::now_v7(), Duration::from_millis(300))
            .await
            .is_none()
    );
    assert!(
        commands
            .await_correlation(Uuid::now_v7(), Duration::from_millis(300))
            .await
            .is_none()
    );
    tap.expect_quiet("nothing was published", Duration::from_millis(300))
        .await;

    capture.stop().await;
    server.shutdown().await;
}
