mod common;

use std::fmt::Debug;

use conformance_directory::{
    ConformanceError, Result, consumer_reads_groups, consumer_reads_users,
    extension_survives_projection, filter_flip_orphan_deletes, publisher_floor,
    publisher_groups_optional, stager_stages_in_the_projection_transaction,
    users_only_narrows_projection,
};

fn assert_names_the_missing_server<T: Debug>(battery: &str, result: Result<T>) {
    match result {
        Err(ConformanceError::Jetstream(detail)) => assert!(
            detail.contains("nats-server"),
            "{battery}: the error must name the missing prerequisite: {detail}"
        ),
        other => panic!("{battery}: a missing nats-server must be a typed error: {other:?}"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn every_battery_returns_a_typed_error_when_nats_server_is_not_on_path() {
    let empty = std::env::temp_dir().join(format!("no-binaries-{}", uuid::Uuid::now_v7().simple()));
    std::fs::create_dir_all(&empty).expect("create an empty PATH directory");
    // SAFETY: this file holds a single test, so no other thread of this process reads or writes the environment while it runs.
    unsafe {
        std::env::set_var("PATH", &empty);
    }

    let snapshot = common::snapshot();
    assert_names_the_missing_server("p1", publisher_floor(&snapshot).await);
    assert_names_the_missing_server("p2", publisher_groups_optional(&snapshot).await);
    assert_names_the_missing_server("c1", consumer_reads_users(&snapshot).await);
    assert_names_the_missing_server("c2", consumer_reads_groups(&snapshot).await);
    assert_names_the_missing_server("c3", extension_survives_projection(&snapshot).await);
    assert_names_the_missing_server("c4", filter_flip_orphan_deletes(&snapshot).await);
    assert_names_the_missing_server("c5", users_only_narrows_projection(&snapshot).await);
    assert_names_the_missing_server(
        "c6",
        stager_stages_in_the_projection_transaction(&snapshot).await,
    );

    std::fs::remove_dir_all(&empty).ok();
}
