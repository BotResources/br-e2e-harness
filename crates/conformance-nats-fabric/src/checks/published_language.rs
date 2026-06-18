use std::collections::BTreeMap;

use br_core_directory::PublishedUser;
use br_test_harness::FabricTestNats;
use br_util_nats_fabric::{
    FabricError, KvKey, PublishedLanguagePublisher, PublishedLanguageReader,
};

use crate::error::Result;
use crate::harness::{namespaced_key, namespaced_prefix};

pub async fn assert_retract_orphan_deletes(harness: &FabricTestNats) -> Result<()> {
    let fabric = harness.fabric_owned();
    let publisher: PublishedLanguagePublisher<PublishedUser> =
        PublishedLanguagePublisher::open(&fabric).await?;
    let reader: PublishedLanguageReader<PublishedUser> =
        PublishedLanguageReader::open(&fabric).await?;

    let key = namespaced_key(harness, "identity/users/ada");
    let ada = user("ada@example.com", Some("Ada"));
    publisher.put(&key, &ada).await?;
    assert!(reader.get(&key).await?.is_some(), "put must be readable");

    publisher.retract(&key).await?;
    assert!(
        reader.get(&key).await?.is_none(),
        "retract must orphan-delete the key"
    );
    Ok(())
}

pub async fn assert_reconcile_drift_converges(harness: &FabricTestNats) -> Result<()> {
    let fabric = harness.fabric_owned();
    let publisher: PublishedLanguagePublisher<PublishedUser> =
        PublishedLanguagePublisher::open(&fabric).await?;
    let reader: PublishedLanguageReader<PublishedUser> =
        PublishedLanguageReader::open(&fabric).await?;
    let prefix = namespaced_prefix(harness, "identity/users/");

    let stale = namespaced_key(harness, "identity/users/stale");
    let changed = namespaced_key(harness, "identity/users/changed");
    publisher
        .put(&stale, &user("stale@example.com", None))
        .await?;
    publisher
        .put(&changed, &user("old@example.com", Some("Old")))
        .await?;

    let kept = namespaced_key(harness, "identity/users/kept");
    let desired = desired_set(&[
        (&kept, user("kept@example.com", Some("Kept"))),
        (&changed, user("new@example.com", Some("New"))),
    ]);
    publisher.reconcile(&prefix, &desired).await?;

    assert!(
        reader.get(&stale).await?.is_none(),
        "reconcile must delete the orphaned key"
    );
    assert_eq!(
        reader.get(&changed).await?.map(|u| u.email),
        Some("new@example.com".to_string()),
        "reconcile must repair the drifted value"
    );
    assert_eq!(
        reader.get(&kept).await?.map(|u| u.email),
        Some("kept@example.com".to_string()),
        "reconcile must add the missing key"
    );
    Ok(())
}

pub async fn assert_decode_fails_closed_naming_the_key(harness: &FabricTestNats) -> Result<()> {
    let fabric = harness.fabric_owned();
    let reader: PublishedLanguageReader<PublishedUser> =
        PublishedLanguageReader::open(&fabric).await?;

    let key = namespaced_key(harness, "identity/users/poison");
    harness.pl_put_raw(&key, b"{ not json").await;

    let err = reader
        .get(&key)
        .await
        .expect_err("a malformed kv value must fail closed, not decode to a default");
    match err {
        FabricError::Decode { subject, .. } => {
            assert_eq!(
                subject,
                key.as_str(),
                "the decode error must name the offending kv key"
            );
        }
        other => panic!("expected Decode naming the key, got {other:?}"),
    }
    Ok(())
}

pub async fn assert_poison_from_anchor_names_the_key(
    harness: &FabricTestNats,
    anchor_key_suffix: &str,
    poison_value: &str,
) -> Result<()> {
    let fabric = harness.fabric_owned();
    let reader: PublishedLanguageReader<PublishedUser> =
        PublishedLanguageReader::open(&fabric).await?;

    let key = namespaced_key(harness, anchor_key_suffix);
    harness.pl_put_raw(&key, poison_value.as_bytes()).await;

    let err = reader
        .get(&key)
        .await
        .expect_err("anchor poison must fail closed");
    assert!(
        matches!(&err, FabricError::Decode { subject, .. } if subject == key.as_str()),
        "anchor poison decode must name the key, got {err:?}"
    );
    Ok(())
}

pub fn parse_published_user_through_lib(go_value: &serde_json::Value) -> Result<PublishedUser> {
    serde_json::from_value(go_value.clone()).map_err(|e| {
        crate::error::ConformanceError::Anchor(format!("anchor user not a PublishedUser: {e}"))
    })
}

fn user(email: &str, first: Option<&str>) -> PublishedUser {
    PublishedUser::new(
        email.to_string(),
        first.map(str::to_string),
        None,
        BTreeMap::new(),
    )
    .expect("a core-only published user is valid")
}

fn desired_set(entries: &[(&KvKey, PublishedUser)]) -> BTreeMap<KvKey, PublishedUser> {
    entries
        .iter()
        .map(|(k, v)| ((*k).clone(), v.clone()))
        .collect()
}
