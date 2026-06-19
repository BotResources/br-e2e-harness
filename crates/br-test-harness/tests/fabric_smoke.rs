#![cfg(feature = "nats-fabric")]
use std::time::Duration;

use br_core_integration::{Aggregate, Bc, EventCoords, PastFact};
use br_test_harness::FabricTestNats;
use br_util_nats_fabric::{EphemeralAuthStore, KvKey};
use futures_util::FutureExt as _;
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
    nats.publish_event_envelope(&coords, &bytes).await;

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
async fn ephemeral_auth_provisions_with_working_per_key_ttl_and_exact_inventory() {
    let nats = FabricTestNats::start().await.with_ephemeral_auth().await;

    assert!(nats.ephemeral_auth_present().await);
    nats.assert_only_kv_buckets(&["EPHEMERAL_AUTH"]).await;

    let store: EphemeralAuthStore<serde_json::Value> =
        EphemeralAuthStore::open(nats.fabric()).await.unwrap();
    let key = KvKey::new(format!("auth/code/{}", Uuid::now_v7().simple())).unwrap();
    store
        .create_with_ttl(
            &key,
            &serde_json::json!({ "code": "x" }),
            Duration::from_secs(1),
        )
        .await
        .expect("per-key ttl write succeeds against the provisioned bucket");
    assert!(
        store.get_with_revision(&key).await.unwrap().is_some(),
        "key is live immediately after the ttl write"
    );
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert!(
        store.get_with_revision(&key).await.unwrap().is_none(),
        "per-key ttl expired the key, proving limit_markers is configured"
    );

    nats.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "real-infra: needs nats-server"]
async fn assert_only_kv_buckets_fails_on_a_stray_bucket() {
    let nats = FabricTestNats::start()
        .await
        .with_ephemeral_auth()
        .await
        .with_published_language()
        .await;

    let outcome = std::panic::AssertUnwindSafe(nats.assert_only_kv_buckets(&["EPHEMERAL_AUTH"]))
        .catch_unwind()
        .await;
    assert!(
        outcome.is_err(),
        "a stray PUBLISHED_LANGUAGE bucket must make the EPHEMERAL_AUTH-only assertion panic"
    );

    nats.assert_only_kv_buckets(&["EPHEMERAL_AUTH", "PUBLISHED_LANGUAGE"])
        .await;

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
    assert!(
        attached
            .pl_get_raw(&KvKey::new("identity/users/keep").unwrap())
            .await
            .is_some(),
        "attach did not wipe"
    );
    attached.shutdown().await;
    assert!(
        owner.published_language_present().await,
        "attached shutdown is a no-op on the owner's server"
    );
    owner.shutdown().await;
}
