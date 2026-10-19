use std::path::{Path, PathBuf};
use std::time::Duration;

use br_test_harness::run_once;
use sqlx::{Connection, PgConnection};

use crate::containment::contained;
use crate::error::{ConformanceError, Result};

const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) async fn nats_server() -> Result<()> {
    match contained(run_once("nats-server", &["--version"], &[], PROBE_TIMEOUT)).await {
        Ok(Ok(output)) if output.status.success() => Ok(()),
        Ok(Ok(output)) => Err(ConformanceError::Jetstream(format!(
            "nats-server is on PATH but `--version` exited with {}",
            output.status
        ))),
        Ok(Err(e)) => Err(ConformanceError::Jetstream(format!(
            "nats-server is unavailable on PATH (install it to run the real-infra batteries): {e}"
        ))),
        Err(panic) => Err(ConformanceError::Jetstream(format!(
            "the nats-server probe could not run (the real-infra batteries need nats-server on PATH and a spawnable `sh`): {panic}"
        ))),
    }
}

pub(crate) fn fabric_nats() -> Result<PathBuf> {
    let exe = std::env::current_exe().map_err(|e| {
        ConformanceError::Jetstream(format!(
            "cannot locate the running executable to find the fabric-nats CLI: {e}"
        ))
    })?;
    fabric_nats_beside(&exe)
}

fn fabric_nats_beside(exe: &Path) -> Result<PathBuf> {
    let mut dir = exe.to_path_buf();
    dir.pop();
    if dir.ends_with("deps") {
        dir.pop();
    }
    let binary = dir.join("fabric-nats");
    if binary.exists() {
        Ok(binary)
    } else {
        Err(ConformanceError::Jetstream(format!(
            "the fabric-nats CLI is not built at {}: build br-test-harness with its nats-fabric \
             feature in the same target directory",
            binary.display()
        )))
    }
}

pub(crate) async fn postgres() -> Result<()> {
    let url = pg_admin_url()?;
    match tokio::time::timeout(PROBE_TIMEOUT, PgConnection::connect(&url)).await {
        Ok(Ok(connection)) => {
            connection.close().await.ok();
            Ok(())
        }
        Ok(Err(e)) => Err(ConformanceError::Postgres(format!(
            "cannot connect to the Postgres admin URL (is PostgreSQL running?): {e}"
        ))),
        Err(_) => Err(ConformanceError::Postgres(format!(
            "the Postgres admin URL did not answer within {PROBE_TIMEOUT:?}"
        ))),
    }
}

fn pg_admin_url() -> Result<String> {
    let dotenv_file = std::cell::OnceCell::new();
    admin_url_from(|name| {
        std::env::var(name).ok().or_else(|| {
            dotenv_file
                .get_or_init(read_dotenv)
                .iter()
                .find_map(|(key, value)| (key == name).then(|| value.clone()))
        })
    })
}

fn read_dotenv() -> Vec<(String, String)> {
    match dotenvy::dotenv_iter() {
        Ok(entries) => entries.map_while(std::result::Result::ok).collect(),
        Err(_) => Vec::new(),
    }
}

fn admin_url_from(lookup: impl Fn(&str) -> Option<String>) -> Result<String> {
    lookup("E2E_PG_ADMIN_URL")
        .or_else(|| lookup("DATABASE_URL"))
        .ok_or_else(|| {
            ConformanceError::Postgres(
                "E2E_PG_ADMIN_URL or DATABASE_URL must be set to the admin URL of a Postgres \
                 that can CREATE ROLE and CREATE DATABASE"
                    .to_string(),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_scratch::scratch;

    #[test]
    fn a_missing_fabric_nats_binary_is_a_typed_error() {
        let target = scratch("no-fabric-nats");
        let exe = target.join("deps").join("conformance-test");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        match fabric_nats_beside(&exe) {
            Err(ConformanceError::Jetstream(detail)) => assert!(detail.contains("fabric-nats")),
            other => panic!("expected a typed fabric-nats error: {other:?}"),
        }
        std::fs::remove_dir_all(target).ok();
    }

    #[test]
    fn fabric_nats_is_found_beside_the_deps_directory() {
        let target = scratch("with-fabric-nats");
        let exe = target.join("deps").join("conformance-test");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(target.join("fabric-nats"), b"").unwrap();
        assert_eq!(
            fabric_nats_beside(&exe).unwrap(),
            target.join("fabric-nats")
        );
        std::fs::remove_dir_all(target).ok();
    }

    #[test]
    fn an_unset_postgres_admin_url_is_a_typed_error() {
        match admin_url_from(|_| None) {
            Err(ConformanceError::Postgres(detail)) => {
                assert!(detail.contains("E2E_PG_ADMIN_URL"));
            }
            other => panic!("expected a typed Postgres error: {other:?}"),
        }
    }

    #[test]
    fn the_admin_url_prefers_e2e_pg_admin_url_over_database_url() {
        let url = admin_url_from(|name| match name {
            "E2E_PG_ADMIN_URL" => Some("postgresql://admin".to_string()),
            "DATABASE_URL" => Some("postgresql://other".to_string()),
            _ => None,
        })
        .unwrap();
        assert_eq!(url, "postgresql://admin");
        let fallback = admin_url_from(|name| {
            (name == "DATABASE_URL").then(|| "postgresql://other".to_string())
        });
        assert_eq!(fallback.unwrap(), "postgresql://other");
    }
}
