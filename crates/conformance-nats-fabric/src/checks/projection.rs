use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use br_core_directory::PublishedUser;
use br_test_harness::FabricTestNats;
use br_util_nats_fabric::{
    KvKey, ProjectionSink, PublishedLanguageConsumer, PublishedLanguagePublisher,
};
use tokio::sync::Mutex;
use tokio::task::{JoinError, JoinHandle};

use crate::abort_on_drop::AbortOnDrop;

use crate::error::Result;
use crate::harness::{namespaced_key, namespaced_prefix};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SinkEvent {
    Project(KvKey),
    Retract(KvKey),
    BootstrapStarted,
}

#[derive(Clone, Default)]
pub struct MemorySink {
    rows: Arc<Mutex<BTreeMap<KvKey, PublishedUser>>>,
    events: Arc<Mutex<Vec<SinkEvent>>>,
}

impl MemorySink {
    pub async fn snapshot(&self) -> BTreeMap<KvKey, PublishedUser> {
        self.rows.lock().await.clone()
    }

    #[cfg(test)]
    pub(crate) fn holders(&self) -> usize {
        Arc::strong_count(&self.rows)
    }

    #[cfg(test)]
    pub(crate) async fn stall_recording(&self) -> tokio::sync::OwnedMutexGuard<Vec<SinkEvent>> {
        Arc::clone(&self.events).lock_owned().await
    }

    #[cfg(test)]
    pub(crate) async fn events(&self) -> Vec<SinkEvent> {
        self.events.lock().await.clone()
    }

    async fn record(&self, event: SinkEvent) {
        self.events.lock().await.push(event);
    }
}

#[async_trait]
impl ProjectionSink<PublishedUser> for MemorySink {
    type Error = std::convert::Infallible;

    async fn project(
        &self,
        key: &KvKey,
        value: &PublishedUser,
    ) -> std::result::Result<(), Self::Error> {
        self.rows.lock().await.insert(key.clone(), value.clone());
        self.record(SinkEvent::Project(key.clone())).await;
        Ok(())
    }

    async fn retract(&self, key: &KvKey) -> std::result::Result<(), Self::Error> {
        self.rows.lock().await.remove(key);
        self.record(SinkEvent::Retract(key.clone())).await;
        Ok(())
    }

    async fn known_keys(&self) -> std::result::Result<BTreeSet<KvKey>, Self::Error> {
        Ok(self.rows.lock().await.keys().cloned().collect())
    }
}

const WATCH_LIVE_DEADLINE: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(25);
const CANARY_RETRY: Duration = Duration::from_millis(500);

type Consumer = PublishedLanguageConsumer<PublishedUser, fn(&PublishedUser) -> bool, MemorySink>;
struct WatchTask {
    handle: JoinHandle<std::result::Result<(), String>>,
    _stop_with_this_check: AbortOnDrop,
}

impl WatchTask {
    fn new(handle: JoinHandle<std::result::Result<(), String>>) -> Self {
        let _stop_with_this_check = AbortOnDrop::new(handle.abort_handle());
        Self {
            handle,
            _stop_with_this_check,
        }
    }

    fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }

    async fn outcome(self) -> std::result::Result<std::result::Result<(), String>, JoinError> {
        self.handle.await
    }
}

fn accept_all(_user: &PublishedUser) -> bool {
    true
}

async fn open_consumer(harness: &FabricTestNats, sink: &MemorySink) -> Result<Arc<Consumer>> {
    let consumer = Consumer::open(
        &harness.fabric_owned(),
        vec![namespaced_prefix(harness, "identity/users/")],
        accept_all as fn(&PublishedUser) -> bool,
        sink.clone(),
    )
    .await?;
    Ok(Arc::new(consumer))
}

fn spawn_watch(consumer: &Arc<Consumer>) -> WatchTask {
    let watcher = consumer.clone();
    WatchTask::new(tokio::spawn(async move {
        watcher.watch().await.map_err(|e| format!("{e:?}"))
    }))
}

async fn assert_watch_has_not_ended(watch: WatchTask, stage: &str) -> WatchTask {
    if !watch.is_finished() {
        return watch;
    }
    let outcome = watch.outcome().await;
    panic!("the watch ended {stage}: {outcome:?}");
}

async fn prove_watch_is_live(
    harness: &FabricTestNats,
    publisher: &PublishedLanguagePublisher<PublishedUser>,
    sink: &MemorySink,
    mut watch: WatchTask,
) -> Result<WatchTask> {
    let canary = namespaced_key(harness, "identity/users/canary");
    let deadline = tokio::time::Instant::now() + WATCH_LIVE_DEADLINE;
    while tokio::time::Instant::now() < deadline {
        publisher.put(&canary, &user("canary@example.com")).await?;
        let (projected, returned) = wait_for_key(sink, &canary, watch, CANARY_RETRY).await;
        watch = returned;
        if projected {
            return Ok(watch);
        }
    }
    panic!("the watch projected no canary put within {WATCH_LIVE_DEADLINE:?}: it is not live");
}

pub async fn assert_bootstrap_then_watch_is_parallel_safe(harness: &FabricTestNats) -> Result<()> {
    assert_bootstrap_under_a_live_watch(harness, &MemorySink::default()).await
}

pub(crate) async fn assert_bootstrap_under_a_live_watch(
    harness: &FabricTestNats,
    sink: &MemorySink,
) -> Result<()> {
    let publisher: PublishedLanguagePublisher<PublishedUser> =
        PublishedLanguagePublisher::open(&harness.fabric_owned()).await?;
    let seed = namespaced_key(harness, "identity/users/seed");
    let orphan = namespaced_key(harness, "identity/users/orphan");
    publisher.put(&seed, &user("seed@example.com")).await?;
    sink.project(&orphan, &user("orphan@example.com"))
        .await
        .expect("seeding a sink orphan is infallible");

    let consumer = open_consumer(harness, sink).await?;
    let watch = spawn_watch(&consumer);
    let watch = prove_watch_is_live(harness, &publisher, sink, watch).await?;

    sink.record(SinkEvent::BootstrapStarted).await;
    consumer
        .bootstrap()
        .await
        .expect("a concurrent bootstrap projects the seed and retracts the orphan");
    assert_bootstrapped_while_watching(watch, sink, &seed, &orphan).await;
    Ok(())
}

async fn assert_bootstrapped_while_watching(
    watch: WatchTask,
    sink: &MemorySink,
    seed: &KvKey,
    orphan: &KvKey,
) {
    let watch = assert_watch_has_not_ended(watch, "before the check stopped it").await;

    let snapshot = sink.snapshot().await;
    assert!(
        snapshot.contains_key(seed),
        "bootstrap must project the published seed even with watch running"
    );
    assert!(
        !snapshot.contains_key(orphan),
        "bootstrap must retract the sink orphan even with watch running"
    );
    drop(watch);
}

pub async fn assert_prefix_watch_delivery_gap(harness: &FabricTestNats) -> Result<bool> {
    let publisher: PublishedLanguagePublisher<PublishedUser> =
        PublishedLanguagePublisher::open(&harness.fabric_owned()).await?;
    let sink = MemorySink::default();
    let consumer = open_consumer(harness, &sink).await?;
    let watch = spawn_watch(&consumer);
    let watch = prove_watch_is_live(harness, &publisher, &sink, watch).await?;

    let live = namespaced_key(harness, "identity/users/live");
    publisher.put(&live, &user("live@example.com")).await?;

    let (delivered, watch) = wait_for_key(&sink, &live, watch, Duration::from_secs(2)).await;
    drop(watch);
    Ok(delivered)
}

async fn wait_for_key(
    sink: &MemorySink,
    key: &KvKey,
    mut watch: WatchTask,
    timeout: Duration,
) -> (bool, WatchTask) {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if sink.snapshot().await.contains_key(key) {
            return (true, watch);
        }
        if tokio::time::Instant::now() >= deadline {
            return (false, watch);
        }
        watch = assert_watch_has_not_ended(watch, "before the live put was delivered").await;
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

fn user(email: &str) -> PublishedUser {
    PublishedUser::new(
        email.to_string(),
        None,
        None,
        std::collections::BTreeMap::new(),
    )
    .expect("a core-only published user is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn ended_watch() -> WatchTask {
        let watch = WatchTask::new(tokio::spawn(async { Err("watch failed".to_string()) }));
        while !watch.is_finished() {
            tokio::task::yield_now().await;
        }
        watch
    }

    #[tokio::test]
    #[should_panic(expected = "the watch ended before the live put was delivered")]
    async fn the_gap_probe_refuses_a_watch_that_ended() {
        let key = KvKey::new("identity/users/live".to_string()).expect("key");
        wait_for_key(
            &MemorySink::default(),
            &key,
            ended_watch().await,
            Duration::from_secs(2),
        )
        .await;
    }

    #[tokio::test]
    #[should_panic(expected = "the watch ended before the check stopped it")]
    async fn the_bootstrap_check_refuses_a_watch_that_ended() {
        let sink = MemorySink::default();
        let seed = KvKey::new("identity/users/seed".to_string()).expect("key");
        let orphan = KvKey::new("identity/users/orphan".to_string()).expect("key");
        sink.project(&seed, &user("seed@example.com"))
            .await
            .expect("infallible");
        assert_bootstrapped_while_watching(ended_watch().await, &sink, &seed, &orphan).await;
    }

    async fn harness() -> FabricTestNats {
        FabricTestNats::start()
            .await
            .with_published_language()
            .await
    }

    #[tokio::test]
    #[ignore = "requires a real nats-server"]
    async fn the_watch_has_projected_a_live_put_before_the_bootstrap_starts() {
        let harness = harness().await;
        let sink = MemorySink::default();
        assert_bootstrap_under_a_live_watch(&harness, &sink)
            .await
            .expect("the check runs");

        let events = sink.events().await;
        let started = events
            .iter()
            .position(|e| *e == SinkEvent::BootstrapStarted)
            .expect("the bootstrap start is recorded");
        let canary = namespaced_key(&harness, "identity/users/canary");
        assert!(
            events[..started].contains(&SinkEvent::Project(canary.clone())),
            "{canary:?} must be projected by the watch alone before the bootstrap starts: {events:?}"
        );
        let seed = namespaced_key(&harness, "identity/users/seed");
        assert!(
            events[started..].contains(&SinkEvent::Project(seed.clone())),
            "{seed:?} predates the watch, only the bootstrap can project it: {events:?}"
        );
        harness.shutdown().await;
    }

    #[tokio::test]
    #[ignore = "requires a real nats-server"]
    #[should_panic(expected = "the watch ended")]
    async fn a_watch_that_failed_is_refused_not_ignored() {
        let harness = harness().await;
        let publisher: PublishedLanguagePublisher<PublishedUser> =
            PublishedLanguagePublisher::open(&harness.fabric_owned())
                .await
                .expect("open the publisher");
        let sink = MemorySink::default();
        let consumer = open_consumer(&harness, &sink).await.expect("open");
        let watch = spawn_watch(&consumer);
        let mut watch = prove_watch_is_live(&harness, &publisher, &sink, watch)
            .await
            .expect("the watch goes live");
        harness
            .pl_put_raw(
                &namespaced_key(&harness, "identity/users/poison"),
                b"{ not json",
            )
            .await;
        let deadline = tokio::time::Instant::now() + WATCH_LIVE_DEADLINE;
        while !watch.is_finished() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "the poisoned watch must end"
            );
            tokio::time::sleep(POLL_INTERVAL).await;
        }
        watch = assert_watch_has_not_ended(watch, "before the check stopped it").await;
        drop(watch);
    }

    #[tokio::test]
    #[ignore = "requires a real nats-server"]
    async fn dropping_a_watch_task_stops_the_watch() {
        let harness = harness().await;
        let sink = MemorySink::default();
        let consumer = open_consumer(&harness, &sink).await.expect("open");
        let watch = spawn_watch(&consumer);
        drop(consumer);
        assert!(sink.holders() > 1, "the watch holds the sink while it runs");
        drop(watch);
        let deadline = tokio::time::Instant::now() + WATCH_LIVE_DEADLINE;
        while sink.holders() != 1 {
            assert!(
                tokio::time::Instant::now() < deadline,
                "a dropped watch task must release the consumer and its sink"
            );
            tokio::time::sleep(POLL_INTERVAL).await;
        }
        harness.shutdown().await;
    }

    #[tokio::test]
    #[ignore = "requires a real nats-server"]
    async fn a_running_watch_is_returned_untouched() {
        let harness = harness().await;
        let sink = MemorySink::default();
        let consumer = open_consumer(&harness, &sink).await.expect("open");
        let watch = spawn_watch(&consumer);
        let watch = assert_watch_has_not_ended(watch, "right after the spawn").await;
        drop(watch);
        harness.shutdown().await;
    }
}
