mod common;

use conformance_directory::{
    ConformanceError, ConsumerDb, consumer_reads_users, extension_survives_projection,
    stager_stages_in_the_projection_transaction, users_only_narrows_projection,
};

const UNREACHABLE: &str = "postgresql://nobody@127.0.0.1:1/postgres";

#[tokio::test(flavor = "current_thread")]
async fn an_unreachable_postgres_is_a_typed_error_not_a_panic() {
    // SAFETY: this file holds a single test, so no other thread of this process reads or writes the environment while it runs.
    unsafe {
        std::env::set_var("E2E_PG_ADMIN_URL", UNREACHABLE);
        std::env::set_var("DATABASE_URL", UNREACHABLE);
    }

    match ConsumerDb::provision().await {
        Err(ConformanceError::Postgres(detail)) => {
            assert!(!detail.is_empty(), "the error must say why");
        }
        Err(other) => {
            panic!("an unreachable Postgres must be ConformanceError::Postgres: {other:?}")
        }
        Ok(_) => panic!("an unreachable Postgres must not provision a database"),
    }

    let snapshot = common::snapshot();
    assert!(consumer_reads_users(&snapshot).await.is_err());
    assert!(extension_survives_projection(&snapshot).await.is_err());
    assert!(users_only_narrows_projection(&snapshot).await.is_err());
    assert!(
        stager_stages_in_the_projection_transaction(&snapshot)
            .await
            .is_err()
    );
}
