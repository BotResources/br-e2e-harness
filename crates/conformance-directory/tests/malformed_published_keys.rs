use br_core_directory::{GROUPS_KEY_PREFIX, USERS_KEY_PREFIX};
use br_util_nats_fabric::KvKey;
use conformance_directory::{ConformanceError, DirectoryHarness, read_groups, read_users};

fn assert_names_the_stray_key<T: std::fmt::Debug>(
    reader: &str,
    expected_key: &str,
    result: conformance_directory::Result<T>,
) {
    match result {
        Err(ConformanceError::NonConformantWire { key, .. }) => assert_eq!(key, expected_key),
        other => panic!("{reader}: a stray key must be a typed error, not dropped: {other:?}"),
    }
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH and the `fabric-nats` CLI built in the same target directory"]
async fn a_stray_key_under_a_directory_prefix_is_reported_not_dropped() {
    let harness = DirectoryHarness::start().await.expect("start the harness");
    let users_key = format!("{USERS_KEY_PREFIX}not-a-uuid");
    let groups_key = format!("{GROUPS_KEY_PREFIX}not-a-uuid");

    assert!(read_users(harness.nats()).await.expect("read").is_empty());
    assert!(read_groups(harness.nats()).await.expect("read").is_empty());

    harness
        .nats()
        .pl_put_raw(&KvKey::new(users_key.as_str()).unwrap(), b"{}")
        .await;
    harness
        .nats()
        .pl_put_raw(&KvKey::new(groups_key.as_str()).unwrap(), b"{}")
        .await;

    assert_names_the_stray_key("read_users", &users_key, read_users(harness.nats()).await);
    assert_names_the_stray_key(
        "read_groups",
        &groups_key,
        read_groups(harness.nats()).await,
    );

    harness.shutdown().await;
}
