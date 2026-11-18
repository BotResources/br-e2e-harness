#[path = "common/child.rs"]
mod child;
mod common;
#[path = "common/scratch.rs"]
mod scratch;

use std::fmt::Debug;
use std::process::Command;

use conformance_directory::{
    ConformanceError, Result, consumer_reads_groups, consumer_reads_users,
    extension_survives_projection, filter_flip_orphan_deletes, publisher_floor,
    publisher_groups_optional, stager_stages_in_the_projection_transaction,
    users_only_narrows_projection,
};

const CHILD: &str = "CONFORMANCE_DIRECTORY_NO_NATS_SERVER_CHILD";
const TEST: &str = "every_battery_returns_a_typed_error_when_nats_server_is_not_on_path";

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
    if std::env::var_os(CHILD).is_none() {
        let empty = scratch::scratch("no-binaries");
        let output = Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, "1")
            .env("PATH", &*empty)
            .output()
            .expect("run the child test");
        child::assert_child_ran_one_passing_test(&output);
        return;
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
}
