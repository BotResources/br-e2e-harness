use br_test_harness::FabricTestNats;
use conformance_directory::{ConformanceError, read_groups, read_meta, read_users};

fn assert_names_the_missing_bucket<T: std::fmt::Debug>(
    reader: &str,
    result: conformance_directory::Result<T>,
) {
    match result {
        Err(ConformanceError::Jetstream(detail)) => assert!(
            detail.contains("PUBLISHED_LANGUAGE"),
            "{reader}: the error must name the missing bucket: {detail}"
        ),
        other => panic!("{reader}: a missing bucket must be a typed error: {other:?}"),
    }
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn reading_a_missing_published_language_bucket_is_a_typed_error_not_a_panic() {
    let nats = FabricTestNats::start().await;

    assert_names_the_missing_bucket("read_users", read_users(&nats).await);
    assert_names_the_missing_bucket("read_groups", read_groups(&nats).await);
    assert_names_the_missing_bucket("read_meta", read_meta(&nats).await);

    nats.shutdown().await;
}
