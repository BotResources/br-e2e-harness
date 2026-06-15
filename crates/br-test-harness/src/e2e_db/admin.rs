use sqlx::{Connection, Executor, PgConnection};

use super::quote::{quote_ident, quote_literal};

pub(super) async fn drop_db_and_owner(
    admin: &mut PgConnection,
    db_name: &str,
    owner_role: &str,
    granted_roles: &[String],
) {
    let db = quote_ident(db_name);
    let owner = quote_ident(owner_role);

    let terminate = format!(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
         WHERE datname = {} AND pid <> pg_backend_pid()",
        quote_literal(db_name)
    );
    if let Err(e) = admin.execute(terminate.as_str()).await {
        eprintln!("warning: failed to terminate backends on '{db_name}': {e}");
    }

    if let Err(e) = admin
        .execute(format!("DROP DATABASE IF EXISTS {db} WITH (FORCE)").as_str())
        .await
    {
        eprintln!("warning: failed to drop e2e database '{db_name}': {e}");
    }

    for role in granted_roles {
        let _ = admin
            .execute(format!("REVOKE {} FROM {owner}", quote_ident(role)).as_str())
            .await;
    }

    if let Err(e) = admin
        .execute(format!("DROP ROLE IF EXISTS {owner}").as_str())
        .await
    {
        eprintln!("warning: failed to drop e2e owner role '{owner_role}': {e}");
    }
}

pub(super) fn teardown_blocking(
    admin_url: String,
    db_name: String,
    owner_role: String,
    granted_roles: Vec<String>,
) {
    let join = std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("warning: e2e Drop net could not build a teardown runtime: {e}");
                return;
            }
        };
        rt.block_on(async {
            let connect = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                PgConnection::connect(&admin_url),
            );
            let mut admin = match connect.await {
                Ok(Ok(c)) => c,
                Ok(Err(e)) => {
                    eprintln!("warning: e2e Drop net could not connect to admin PG: {e}");
                    return;
                }
                Err(_) => {
                    eprintln!("warning: e2e Drop net timed out connecting to admin PG");
                    return;
                }
            };
            drop_db_and_owner(&mut admin, &db_name, &owner_role, &granted_roles).await;
            admin.close().await.ok();
        });
    });
    if let Err(e) = join.join() {
        eprintln!("warning: e2e Drop net teardown thread panicked: {e:?}");
    }
}

pub(super) async fn ensure_owner_role(
    admin: &mut PgConnection,
    name: &str,
    password: &str,
    bypassrls: bool,
) {
    let bypass_clause = if bypassrls {
        "BYPASSRLS"
    } else {
        "NOBYPASSRLS"
    };
    let attributes = format!(
        "LOGIN NOSUPERUSER CREATEROLE {bypass_clause} PASSWORD {}",
        quote_literal(password)
    );
    ensure_role_under_lock(admin, name, &attributes).await;
}

pub(super) async fn ensure_app_role(admin: &mut PgConnection, name: &str, password: &str) {
    let attributes = format!("LOGIN NOBYPASSRLS PASSWORD {}", quote_literal(password));
    ensure_role_under_lock(admin, name, &attributes).await;
}

async fn ensure_role_under_lock(admin: &mut PgConnection, name: &str, attributes: &str) {
    let ident = quote_ident(name);
    let body = format!(
        "DO $$ BEGIN \
           IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = {literal}) THEN \
             CREATE ROLE {ident} WITH {attributes}; \
           ELSE \
             ALTER ROLE {ident} WITH {attributes}; \
           END IF; \
         EXCEPTION WHEN duplicate_object THEN \
           ALTER ROLE {ident} WITH {attributes}; \
         END $$;",
        literal = quote_literal(name)
    );
    run_under_advisory_lock(admin, name, &body)
        .await
        .unwrap_or_else(|e| panic!("failed to ensure e2e role '{name}': {e}"));
}

pub(super) async fn grant_app_schema(owner: &mut PgConnection, owner_role: &str, app_role: &str) {
    let app = quote_ident(app_role);
    let owner_ident = quote_ident(owner_role);
    let body = [
        format!("GRANT USAGE, CREATE ON SCHEMA public TO {app}"),
        format!(
            "ALTER DEFAULT PRIVILEGES FOR ROLE {owner_ident} IN SCHEMA public \
             GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO {app}"
        ),
        format!(
            "ALTER DEFAULT PRIVILEGES FOR ROLE {owner_ident} IN SCHEMA public \
             GRANT USAGE, SELECT ON SEQUENCES TO {app}"
        ),
    ]
    .join("; ");
    run_under_advisory_lock(owner, app_role, &body)
        .await
        .unwrap_or_else(|e| panic!("failed to grant public schema to app role '{app_role}': {e}"));
}

pub(super) async fn grant_connect(admin: &mut PgConnection, db_name: &str, app_role: &str) {
    let body = format!(
        "GRANT CONNECT ON DATABASE {} TO {}",
        quote_ident(db_name),
        quote_ident(app_role)
    );
    run_under_advisory_lock(admin, app_role, &body)
        .await
        .unwrap_or_else(|e| panic!("failed to grant connect on '{db_name}' to '{app_role}': {e}"));
}

pub(super) async fn grant_managed_role(admin: &mut PgConnection, role: &str, owner_role: &str) {
    let body = format!(
        "GRANT {} TO {} WITH ADMIN OPTION",
        quote_ident(role),
        quote_ident(owner_role)
    );
    run_under_advisory_lock(admin, role, &body)
        .await
        .unwrap_or_else(|e| panic!("failed to grant role '{role}' to owner '{owner_role}': {e}"));
}

async fn run_under_advisory_lock(
    conn: &mut PgConnection,
    lock_key: &str,
    body: &str,
) -> Result<(), sqlx::Error> {
    let mut last_err = None;
    for attempt in 0..8 {
        let mut tx = conn.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
            .bind(lock_key)
            .execute(&mut *tx)
            .await?;
        match tx.execute(body).await {
            Ok(_) => {
                tx.commit().await?;
                return Ok(());
            }
            Err(e) => {
                tx.rollback().await.ok();
                if is_concurrent_catalog_conflict(&e) {
                    last_err = Some(e);
                    let backoff = std::time::Duration::from_millis(20 * (attempt + 1));
                    tokio::time::sleep(backoff).await;
                    continue;
                }
                return Err(e);
            }
        }
    }
    Err(last_err.expect("retry loop only continues after recording an error"))
}

fn is_concurrent_catalog_conflict(e: &sqlx::Error) -> bool {
    e.as_database_error()
        .and_then(|db| db.code())
        .is_some_and(|code| code == "XX000" || code == "40P01" || code == "55P03")
}

pub(super) async fn role_exists(admin: &mut PgConnection, role: &str) -> bool {
    let row: Option<(i32,)> = sqlx::query_as("SELECT 1 FROM pg_roles WHERE rolname = $1")
        .bind(role)
        .fetch_optional(&mut *admin)
        .await
        .unwrap_or_else(|e| panic!("failed to query pg_roles for '{role}': {e}"));
    row.is_some()
}

pub(super) fn pg_host_port(url: &str) -> String {
    let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);

    let authority = after_scheme.split('/').next().unwrap_or(after_scheme);
    authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{pg_host_port, teardown_blocking};

    #[test]
    fn teardown_against_an_unreachable_admin_returns_without_panicking() {
        teardown_blocking(
            "postgresql://e2e_drop_net@127.0.0.1:1/postgres".to_string(),
            "e2e_unreachable_db".to_string(),
            "e2e_unreachable_owner".to_string(),
            Vec::new(),
        );
    }

    #[test]
    fn extracts_host_port_with_credentials() {
        assert_eq!(
            pg_host_port("postgresql://user:pw@localhost:5432/postgres"),
            "localhost:5432"
        );
    }

    #[test]
    fn extracts_host_port_without_credentials() {
        assert_eq!(
            pg_host_port("postgresql://localhost:5432/postgres"),
            "localhost:5432"
        );
    }

    #[test]
    fn password_containing_at_sign_splits_on_the_last_at() {
        assert_eq!(
            pg_host_port("postgres://user:p@ss@localhost:5433/app"),
            "localhost:5433"
        );
    }

    #[test]
    fn host_without_port_is_returned_bare() {
        assert_eq!(pg_host_port("postgresql://user@host/db"), "host");
    }

    #[test]
    fn query_params_are_not_part_of_the_authority() {
        assert_eq!(
            pg_host_port("postgresql://u:p@localhost:5432/db?sslmode=require"),
            "localhost:5432"
        );
    }
}
