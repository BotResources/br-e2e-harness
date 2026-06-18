#![cfg(feature = "nats-fabric")]
use std::time::Duration;

use br_core_integration::{Aggregate, Bc, EventCoords, PastFact};
use br_test_harness::FabricTestNats;
use br_util_nats_fabric::KvKey;
use uuid::Uuid;

fn ev() -> EventCoords {
    EventCoords {
        producer: Bc::new("identity").unwrap(),
        aggregate: Aggregate::new("user").unwrap(),
        fact: PastFact::new("created").unwrap(),
        version: 1,
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "real-infra: needs nats-server"]
async fn capture_await_pl_bearer_round_trip() {
    let nats = FabricTestNats::start()
        .await
        .with_published_language()
        .await
        .with_bearer_tokens()
        .await;

    let coords = ev();
    let capture = nats.capture_events(&[&coords]).await;
    let mut awaiter = nats.await_event(&coords).await;

    let cid = nats.correlation();
    let env = serde_json::json!({
        "metadata": { "actor_id": Uuid::nil(), "actor_kind": "human", "correlation_id": cid, "causation_id": cid },
        "payload": { "name": "ada" }
    });
    let bytes = serde_json::to_vec(&env).unwrap();
    let subject = br_util_nats_fabric::event_subject(&coords);
    nats.client().publish(subject, bytes.into()).await.unwrap();
    nats.client().flush().await.unwrap();

    let hit = awaiter.await_correlation(cid, Duration::from_secs(3)).await;
    assert!(hit.is_some(), "awaiter saw the correlated event");

    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(
        capture.for_correlation(cid).len(),
        1,
        "capture keyed by correlation"
    );
    assert_eq!(capture.count(), 1);
    capture.stop().await;

    nats.pl_put_raw(&KvKey::new("identity/users/smoke").unwrap(), b"{ poison")
        .await;

    let seeder = nats.bearer_seeder();
    let token = seeder.seed("smoke", "alice").await.unwrap();
    assert!(token.raw.starts_with("brk_"));
    seeder.revoke(&token).await.unwrap();

    nats.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "real-infra: needs nats-server"]
async fn connect_mode_attaches_without_wiping() {
    let owner = FabricTestNats::start()
        .await
        .with_published_language()
        .await;
    let url = owner.url();
    owner
        .pl_put_raw(&KvKey::new("identity/users/keep").unwrap(), b"{}")
        .await;

    let attached = FabricTestNats::connect(&url)
        .await
        .with_published_language()
        .await;
    let store = attached
        .jetstream()
        .get_key_value("PUBLISHED_LANGUAGE")
        .await
        .unwrap();
    assert!(
        store.get("identity/users/keep").await.unwrap().is_some(),
        "attach did not wipe"
    );
    attached.shutdown().await;
    assert!(
        owner
            .jetstream()
            .get_key_value("PUBLISHED_LANGUAGE")
            .await
            .is_ok(),
        "attached shutdown is a no-op on the owner's server"
    );
    owner.shutdown().await;
}
