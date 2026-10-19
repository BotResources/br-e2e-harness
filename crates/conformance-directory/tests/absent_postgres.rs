#[path = "common/child.rs"]
mod child;
mod common;

use std::fmt::Debug;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use conformance_directory::{
    ConformanceError, ConsumerDb, Result, consumer_reads_groups, consumer_reads_users,
    extension_survives_projection, filter_flip_orphan_deletes,
    stager_stages_in_the_projection_transaction, users_only_narrows_projection,
};

const ISOLATED: &str = "CONFORMANCE_DIRECTORY_ISOLATED_COPY";
const TEST: &str = "an_unreachable_postgres_is_a_typed_error_not_a_panic";
const UNREACHABLE: &str = "postgresql://nobody@127.0.0.1:1/postgres";

fn write_stub(path: &std::path::Path) {
    std::fs::write(path, b"#!/bin/sh\nexit 0\n").expect("write a stub");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod a stub");
}

fn system_path_after(stubs: &std::path::Path) -> std::ffi::OsString {
    std::env::join_paths([stubs, "/usr/bin".as_ref(), "/bin".as_ref()]).expect("join the PATH")
}

fn run_in_isolated_copy() {
    let exe = std::env::current_exe().expect("test executable");
    let isolated = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("no-postgres-{}", uuid::Uuid::now_v7().simple()));
    let deps = isolated.join("deps");
    let stubs = isolated.join("stubs");
    std::fs::create_dir_all(&deps).expect("create the isolated target directory");
    std::fs::create_dir_all(&stubs).expect("create the stub directory");
    write_stub(&stubs.join("nats-server"));
    write_stub(&isolated.join("fabric-nats"));
    let copy = deps.join(exe.file_name().expect("test executable name"));
    std::fs::copy(&exe, &copy).expect("copy the test executable beside the stub fabric-nats");

    let output = Command::new(&copy)
        .args(["--exact", TEST, "--nocapture"])
        .env(ISOLATED, "1")
        .env("PATH", system_path_after(&stubs))
        .env("E2E_PG_ADMIN_URL", UNREACHABLE)
        .env("DATABASE_URL", UNREACHABLE)
        .output()
        .expect("run the isolated copy");
    std::fs::remove_dir_all(&isolated).ok();
    child::assert_child_ran_one_passing_test(&output);
}

fn assert_postgres_error<T: Debug>(battery: &str, result: Result<T>) {
    match result {
        Err(ConformanceError::Postgres(detail)) => {
            assert!(!detail.is_empty(), "{battery}: the error must say why");
        }
        other => panic!(
            "{battery}: an unreachable Postgres must be ConformanceError::Postgres: {other:?}"
        ),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn an_unreachable_postgres_is_a_typed_error_not_a_panic() {
    if std::env::var_os(ISOLATED).is_none() {
        run_in_isolated_copy();
        return;
    }

    assert_postgres_error("provision", ConsumerDb::provision().await.map(|_| ()));

    let snapshot = common::snapshot();
    assert_postgres_error("c1", consumer_reads_users(&snapshot).await);
    assert_postgres_error("c2", consumer_reads_groups(&snapshot).await);
    assert_postgres_error("c3", extension_survives_projection(&snapshot).await);
    assert_postgres_error("c4", filter_flip_orphan_deletes(&snapshot).await);
    assert_postgres_error("c5", users_only_narrows_projection(&snapshot).await);
    assert_postgres_error(
        "c6",
        stager_stages_in_the_projection_transaction(&snapshot).await,
    );
}
