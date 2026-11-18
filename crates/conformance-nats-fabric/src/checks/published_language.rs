use std::collections::BTreeMap;

use br_core_directory::PublishedUser;
use br_test_harness::FabricTestNats;
use br_util_nats_fabric::{
    FabricError, KvKey, PublishedLanguagePublisher, PublishedLanguageReader,
};

use crate::error::Result;
use crate::harness::{namespaced_key, namespaced_prefix};
use crate::wire::FrozenWire;

pub async fn assert_retract_orphan_deletes(harness: &FabricTestNats) -> Result<()> {
    let fabric = harness.fabric_owned();
    let publisher: PublishedLanguagePublisher<PublishedUser> =
        PublishedLanguagePublisher::open(&fabric).await?;
    let reader: PublishedLanguageReader<PublishedUser> =
        PublishedLanguageReader::open(&fabric).await?;

    let key = namespaced_key(harness, "identity/users/ada");
    let ada = full_user("ada@example.com", Some("Ada"), Some("Lovelace"), "analyst");
    publisher.put(&key, &ada).await?;
    assert_reads_back(reader.get(&key).await?, &ada, "put");

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
    let unchanged = namespaced_key(harness, "identity/users/unchanged");
    let unchanged_user = full_user(
        "unchanged@example.com",
        Some("Un"),
        Some("Changed"),
        "support",
    );
    publisher
        .put(&stale, &user("stale@example.com", None))
        .await?;
    publisher
        .put(
            &changed,
            &full_user("old@example.com", Some("Old"), Some("Name"), "sales"),
        )
        .await?;
    publisher.put(&unchanged, &unchanged_user).await?;

    let kept = namespaced_key(harness, "identity/users/kept");
    let kept_user = full_user("kept@example.com", Some("Kept"), Some("Added"), "ops");
    let changed_user = full_user("new@example.com", Some("New"), Some("Person"), "legal");
    let desired = desired_set(&[
        (&kept, kept_user.clone()),
        (&changed, changed_user.clone()),
        (&unchanged, unchanged_user.clone()),
    ]);
    publisher.reconcile(&prefix, &desired).await?;

    assert!(
        reader.get(&stale).await?.is_none(),
        "reconcile must delete the orphaned key"
    );
    assert_reads_back(
        reader.get(&changed).await?,
        &changed_user,
        "reconcile (repair of the drifted value)",
    );
    assert_reads_back(
        reader.get(&kept).await?,
        &kept_user,
        "reconcile (addition of the missing key)",
    );
    assert_reads_back(
        reader.get(&unchanged).await?,
        &unchanged_user,
        "reconcile (untouched value)",
    );
    Ok(())
}

fn assert_reads_back(actual: Option<PublishedUser>, expected: &PublishedUser, what: &str) {
    match actual {
        None => panic!("{what}: the key is missing, expected {expected:?}"),
        Some(actual) => assert_eq!(
            &actual, expected,
            "{what}: the value read back has drifted from the desired one"
        ),
    }
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

pub fn assert_anchor_users_deserialize(wire: &FrozenWire) {
    assert!(
        !wire.published_users.is_empty(),
        "the anchor freezes no published user"
    );
    for entry in &wire.published_users {
        let user = parse_published_user_through_lib(&entry.value)
            .unwrap_or_else(|e| panic!("anchor user {} is not a PublishedUser: {e}", entry.key));
        assert!(
            !user.email.is_empty(),
            "the published user {} carries an email",
            entry.key
        );
    }
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

fn full_user(
    email: &str,
    first: Option<&str>,
    last: Option<&str>,
    department: &str,
) -> PublishedUser {
    PublishedUser::new(
        email.to_string(),
        first.map(str::to_string),
        last.map(str::to_string),
        BTreeMap::from([("department".to_string(), serde_json::json!(department))]),
    )
    .expect("a published user with an extension is valid")
}

fn desired_set(entries: &[(&KvKey, PublishedUser)]) -> BTreeMap<KvKey, PublishedUser> {
    entries
        .iter()
        .map(|(k, v)| ((*k).clone(), v.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_equal_in_every_field_reads_back() {
        let desired = full_user("a@example.com", Some("A"), Some("B"), "ops");
        assert_reads_back(Some(desired.clone()), &desired, "put");
    }

    #[test]
    #[should_panic(expected = "has drifted")]
    fn a_repaired_email_with_a_drifted_first_name_is_refused() {
        let desired = full_user("new@example.com", Some("New"), Some("B"), "ops");
        let actual = full_user("new@example.com", Some("Old"), Some("B"), "ops");
        assert_reads_back(Some(actual), &desired, "reconcile");
    }

    #[test]
    #[should_panic(expected = "has drifted")]
    fn a_repaired_email_with_a_drifted_last_name_is_refused() {
        let desired = full_user("new@example.com", Some("New"), Some("B"), "ops");
        let actual = full_user("new@example.com", Some("New"), Some("Old"), "ops");
        assert_reads_back(Some(actual), &desired, "reconcile");
    }

    #[test]
    #[should_panic(expected = "has drifted")]
    fn a_repaired_email_with_a_drifted_extension_is_refused() {
        let desired = full_user("new@example.com", Some("New"), Some("B"), "ops");
        let actual = full_user("new@example.com", Some("New"), Some("B"), "sales");
        assert_reads_back(Some(actual), &desired, "reconcile");
    }

    #[test]
    #[should_panic(expected = "the key is missing")]
    fn an_absent_value_is_refused() {
        let desired = full_user("a@example.com", None, None, "ops");
        assert_reads_back(None, &desired, "put");
    }
}
