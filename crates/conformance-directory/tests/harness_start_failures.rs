#[path = "common/child.rs"]
mod child;
#[path = "common/isolated.rs"]
mod isolated;
#[path = "common/scratch.rs"]
mod scratch;

use std::str::FromStr;

use conformance_directory::{ConformanceError, ConsumerDb, DirectoryHarness};
use isolated::{Isolation, NatsServer};
use sqlx::postgres::PgConnectOptions;
use sqlx::{Connection, Executor, PgConnection};

const FABRIC_NATS_OK: &str = "#!/bin/sh\nexit 0\n";
const FABRIC_NATS_FAILS: &str = "#!/bin/sh\necho 'provisioning refused' >&2\nexit 1\n";
const FABRIC_NATS_KILLS_THE_BROKER: &str =
    "#!/bin/sh\nkill -9 \"$(tail -n 1 \"$CONFORMANCE_DIRECTORY_NATS_PIDS\")\"\nsleep 1\nexit 0\n";

const NATS_CANNOT_START: &str = "a_throwaway_nats_that_cannot_start_is_a_typed_error";
const PROVISIONING_FAILS: &str =
    "a_failed_provisioning_is_a_typed_error_and_leaves_no_nats_server_running";
const BUCKET_ABSENT: &str =
    "a_bucket_absent_after_provisioning_is_a_typed_error_and_leaves_no_nats_server_running";
const PROBE_PANICS: &str =
    "a_broker_lost_before_the_bucket_probe_is_a_typed_error_and_leaves_no_nats_server_running";

fn is_child() -> bool {
    std::env::var_os(isolated::CHILD).is_some()
}

fn jetstream_detail<T>(result: Result<T, ConformanceError>, what: &str) -> String {
    match result {
        Err(ConformanceError::Jetstream(detail)) => detail,
        Err(other) => panic!("{what} must be a typed Jetstream error: {other:?}"),
        Ok(_) => panic!("{what} must not yield a harness"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a_throwaway_nats_that_cannot_start_is_a_typed_error() {
    if !is_child() {
        isolated::run(&Isolation {
            label: "nats-cannot-start",
            test: NATS_CANNOT_START,
            ignored: false,
            fabric_nats: FABRIC_NATS_OK,
            nats_server: NatsServer::ExitsAfterVersion,
            envs: &[],
        });
        return;
    }
    let detail = jetstream_detail(
        DirectoryHarness::start().await,
        "a broker that exits at once",
    );
    assert!(detail.contains("start the throwaway NATS"), "{detail}");
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_failed_provisioning_is_a_typed_error_and_leaves_no_nats_server_running() {
    if !is_child() {
        isolated::run(&Isolation {
            label: "provisioning-fails",
            test: PROVISIONING_FAILS,
            ignored: true,
            fabric_nats: FABRIC_NATS_FAILS,
            nats_server: NatsServer::RealRecordingItsPid,
            envs: &[],
        });
        return;
    }
    let detail = jetstream_detail(DirectoryHarness::start().await, "a refused provisioning");
    assert!(detail.contains("provisioning refused"), "{detail}");
    isolated::assert_every_started_nats_server_is_gone();
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_bucket_absent_after_provisioning_is_a_typed_error_and_leaves_no_nats_server_running() {
    if !is_child() {
        isolated::run(&Isolation {
            label: "bucket-absent",
            test: BUCKET_ABSENT,
            ignored: true,
            fabric_nats: FABRIC_NATS_OK,
            nats_server: NatsServer::RealRecordingItsPid,
            envs: &[],
        });
        return;
    }
    let detail = jetstream_detail(DirectoryHarness::start().await, "an unprovisioned bucket");
    assert!(detail.contains("absent after provisioning"), "{detail}");
    isolated::assert_every_started_nats_server_is_gone();
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_broker_lost_before_the_bucket_probe_is_a_typed_error_and_leaves_no_nats_server_running()
{
    if !is_child() {
        isolated::run(&Isolation {
            label: "probe-panics",
            test: PROBE_PANICS,
            ignored: true,
            fabric_nats: FABRIC_NATS_KILLS_THE_BROKER,
            nats_server: NatsServer::RealRecordingItsPid,
            envs: &[],
        });
        return;
    }
    let detail = jetstream_detail(DirectoryHarness::start().await, "a lost broker");
    assert!(
        detail.contains("probe the PUBLISHED_LANGUAGE bucket after provisioning"),
        "{detail}"
    );
    isolated::assert_every_started_nats_server_is_gone();
}

const DATABASE_DENIED: &str = "a_database_the_admin_role_may_not_create_is_a_typed_postgres_error";
const ADMIN_URL: &str = "E2E_PG_ADMIN_URL";
const UNPRIVILEGED_PASSWORD: &str = "unprivileged_pw";

fn admin_url() -> String {
    std::env::var(ADMIN_URL)
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("E2E_PG_ADMIN_URL or DATABASE_URL must be set to the admin URL of a real Postgres")
}

fn url_as(admin: &str, role: &str) -> String {
    let options = PgConnectOptions::from_str(admin).expect("the admin URL parses");
    format!(
        "postgresql://{role}:{UNPRIVILEGED_PASSWORD}@{}:{}/{}",
        options.get_host(),
        options.get_port(),
        options.get_database().unwrap_or("postgres"),
    )
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "real-infra: needs a Postgres reachable at E2E_PG_ADMIN_URL or DATABASE_URL"]
async fn a_database_the_admin_role_may_not_create_is_a_typed_postgres_error() {
    if is_child() {
        match ConsumerDb::provision().await {
            Err(ConformanceError::Postgres(detail)) => assert!(
                detail.contains("provision the throwaway database"),
                "{detail}"
            ),
            Err(other) => panic!("a refused CREATE ROLE must be a typed Postgres error: {other:?}"),
            Ok(_) => panic!("a role without CREATEROLE must not provision a database"),
        }
        return;
    }

    let admin = admin_url();
    let role = format!("unprivileged_{}", uuid::Uuid::now_v7().simple());
    let mut connection = PgConnection::connect(&admin)
        .await
        .expect("connect as admin");
    connection
        .execute(
            format!(
                "CREATE ROLE {role} LOGIN PASSWORD '{UNPRIVILEGED_PASSWORD}' \
                 NOSUPERUSER NOCREATEDB NOCREATEROLE"
            )
            .as_str(),
        )
        .await
        .expect("create the unprivileged role");

    let unprivileged = url_as(&admin, &role);
    let outcome = std::panic::catch_unwind(|| {
        isolated::run(&Isolation {
            label: "database-denied",
            test: DATABASE_DENIED,
            ignored: true,
            fabric_nats: "#!/bin/sh\nexit 0\n",
            nats_server: NatsServer::ExitsAfterVersion,
            envs: &[(ADMIN_URL, &unprivileged), ("DATABASE_URL", &unprivileged)],
        })
    });

    connection
        .execute(format!("DROP ROLE {role}").as_str())
        .await
        .expect("drop the unprivileged role");
    connection.close().await.ok();
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
}
