#[path = "common/child.rs"]
mod child;
#[path = "common/scratch.rs"]
mod scratch;

use std::process::Command;

use conformance_directory::{ConformanceError, ConsumerDb};

const ISOLATED: &str = "CONFORMANCE_DIRECTORY_DOTENV_CHILD";
const TEST: &str =
    "the_postgres_probe_reads_a_dotenv_file_without_touching_the_process_environment";
const UNREACHABLE: &str = "postgresql://nobody@127.0.0.1:1/postgres";

fn run_in_directory_holding_a_dotenv_file() {
    let exe = std::env::current_exe().expect("test executable");
    let dir = scratch::scratch("dotenv");
    std::fs::write(
        dir.join(".env"),
        format!("E2E_PG_ADMIN_URL={UNREACHABLE}\nDATABASE_URL={UNREACHABLE}\n"),
    )
    .expect("write the .env file");

    let output = Command::new(&exe)
        .args(["--exact", TEST, "--nocapture"])
        .current_dir(&*dir)
        .env(ISOLATED, "1")
        .env_remove("E2E_PG_ADMIN_URL")
        .env_remove("DATABASE_URL")
        .output()
        .expect("run the child test");
    child::assert_child_ran_one_passing_test(&output);
}

#[tokio::test(flavor = "current_thread")]
async fn the_postgres_probe_reads_a_dotenv_file_without_touching_the_process_environment() {
    if std::env::var_os(ISOLATED).is_none() {
        run_in_directory_holding_a_dotenv_file();
        return;
    }

    match ConsumerDb::provision().await.map(|_| ()) {
        Err(ConformanceError::Postgres(detail)) => assert!(
            detail.contains("cannot connect"),
            "the .env URL must be the one probed: {detail}"
        ),
        other => panic!("expected an unreachable Postgres from the .env URL: {other:?}"),
    }
    assert!(std::env::var_os("E2E_PG_ADMIN_URL").is_none());
    assert!(std::env::var_os("DATABASE_URL").is_none());
}
