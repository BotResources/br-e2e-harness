use br_test_harness::{FabricTestNats, SpawnedNats};
use conformance_directory::{ConformanceError, read_groups, read_meta, read_users};

fn assert_names_the_bucket<T: std::fmt::Debug>(
    reader: &str,
    result: conformance_directory::Result<T>,
) {
    match result {
        Err(ConformanceError::Jetstream(detail)) => assert!(
            detail.contains("PUBLISHED_LANGUAGE"),
            "{reader}: the error must name the bucket: {detail}"
        ),
        other => panic!("{reader}: an unreachable broker must be a typed error: {other:?}"),
    }
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn reading_the_bucket_of_a_dead_broker_is_a_typed_error_not_a_panic() {
    let server = SpawnedNats::start().await;
    let nats = FabricTestNats::attach_without_provisioning(&server.url()).await;
    server.shutdown().await;

    assert_names_the_bucket("read_users", read_users(&nats).await);
    assert_names_the_bucket("read_groups", read_groups(&nats).await);
    assert_names_the_bucket("read_meta", read_meta(&nats).await);
}
