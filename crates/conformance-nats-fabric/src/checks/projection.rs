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

use crate::error::Result;
use crate::harness::{namespaced_key, namespaced_prefix};

#[derive(Clone, Default)]
pub struct MemorySink {
    rows: Arc<Mutex<BTreeMap<KvKey, PublishedUser>>>,
}

impl MemorySink {
    pub async fn snapshot(&self) -> BTreeMap<KvKey, PublishedUser> {
        self.rows.lock().await.clone()
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
        Ok(())
    }

    async fn retract(&self, key: &KvKey) -> std::result::Result<(), Self::Error> {
        self.rows.lock().await.remove(key);
        Ok(())
    }

    async fn known_keys(&self) -> std::result::Result<BTreeSet<KvKey>, Self::Error> {
        Ok(self.rows.lock().await.keys().cloned().collect())
    }
}

pub async fn assert_bootstrap_then_watch_is_parallel_safe(harness: &FabricTestNats) -> Result<()> {
    let fabric = harness.fabric_owned();
    let publisher: PublishedLanguagePublisher<PublishedUser> =
        PublishedLanguagePublisher::open(&fabric).await?;
    let prefix = namespaced_prefix(harness, "identity/users/");

    let seed = namespaced_key(harness, "identity/users/seed");
    let orphan = namespaced_key(harness, "identity/users/orphan");
    publisher.put(&seed, &user("seed@example.com")).await?;

    let sink = MemorySink::default();
    sink.project(&orphan, &user("orphan@example.com"))
        .await
        .expect("seeding a sink orphan is infallible");

    let consumer: PublishedLanguageConsumer<PublishedUser, _, _> =
        PublishedLanguageConsumer::open(&fabric, vec![prefix.clone()], |_user| true, sink.clone())
            .await?;
    let consumer = Arc::new(consumer);

    let watcher = consumer.clone();
    let watch = tokio::spawn(async move {
        if let Err(e) = watcher.watch().await {
            eprintln!("watch ended with: {e:?}");
        }
    });

    consumer
        .bootstrap()
        .await
        .expect("a concurrent bootstrap projects the seed and retracts the orphan");

    let snapshot = sink.snapshot().await;
    assert!(
        snapshot.contains_key(&seed),
        "bootstrap must project the published seed even with watch running"
    );
    assert!(
        !snapshot.contains_key(&orphan),
        "bootstrap must retract the sink orphan even with watch running"
    );

    watch.abort();
    Ok(())
}

pub async fn assert_prefix_watch_delivery_gap(harness: &FabricTestNats) -> Result<bool> {
    let fabric = harness.fabric_owned();
    let publisher: PublishedLanguagePublisher<PublishedUser> =
        PublishedLanguagePublisher::open(&fabric).await?;
    let prefix = namespaced_prefix(harness, "identity/users/");

    let sink = MemorySink::default();
    let consumer: PublishedLanguageConsumer<PublishedUser, _, _> =
        PublishedLanguageConsumer::open(&fabric, vec![prefix.clone()], |_user| true, sink.clone())
            .await?;
    let consumer = Arc::new(consumer);

    let watcher = consumer.clone();
    let watch = tokio::spawn(async move {
        let _ = watcher.watch().await;
    });
    tokio::time::sleep(Duration::from_millis(300)).await;

    let live = namespaced_key(harness, "identity/users/live");
    publisher.put(&live, &user("live@example.com")).await?;

    let delivered = wait_for_key(&sink, &live, Duration::from_secs(2)).await;
    watch.abort();
    Ok(delivered)
}

async fn wait_for_key(sink: &MemorySink, key: &KvKey, timeout: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if sink.snapshot().await.contains_key(key) {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
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
