use sqlx::{Connection, Executor, PgConnection};

pub(super) async fn drop_db_and_owner(
    admin: &mut PgConnection,
    db_name: &str,
    owner_role: &str,
    granted_roles: &[String],
) {
    let terminate = format!(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
         WHERE datname = '{db_name}' AND pid <> pg_backend_pid()"
    );
    if let Err(e) = admin.execute(terminate.as_str()).await {
        eprintln!("warning: failed to terminate backends on '{db_name}': {e}");
    }

    if let Err(e) = admin
        .execute(format!("DROP DATABASE IF EXISTS \"{db_name}\" WITH (FORCE)").as_str())
        .await
    {
        eprintln!("warning: failed to drop e2e database '{db_name}': {e}");
    }

    for role in granted_roles {
        let _ = admin
            .execute(format!("REVOKE \"{role}\" FROM \"{owner_role}\"").as_str())
            .await;
    }

    if let Err(e) = admin
        .execute(format!("DROP ROLE IF EXISTS \"{owner_role}\"").as_str())
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
    let password = password.replace('\'', "''");
    let bypass_clause = if bypassrls {
        "BYPASSRLS"
    } else {
        "NOBYPASSRLS"
    };
    let attributes = format!("LOGIN NOSUPERUSER CREATEROLE {bypass_clause} PASSWORD '{password}'");
    let sql = format!(
        "DO $$ BEGIN \
           IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = '{name}') THEN \
             CREATE ROLE \"{name}\" WITH {attributes}; \
           ELSE \
             ALTER ROLE \"{name}\" WITH {attributes}; \
           END IF; \
         EXCEPTION WHEN duplicate_object THEN \
           ALTER ROLE \"{name}\" WITH {attributes}; \
         END $$;"
    );
    admin
        .execute(sql.as_str())
        .await
        .unwrap_or_else(|e| panic!("failed to ensure e2e owner role '{name}': {e}"));
}

pub(super) async fn ensure_app_role(admin: &mut PgConnection, name: &str, password: &str) {
    let password = password.replace('\'', "''");
    let sql = format!(
        "DO $$ BEGIN \
           IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = '{name}') THEN \
             CREATE ROLE \"{name}\" WITH LOGIN NOBYPASSRLS PASSWORD '{password}'; \
           ELSE \
             ALTER ROLE \"{name}\" WITH LOGIN NOBYPASSRLS PASSWORD '{password}'; \
           END IF; \
         EXCEPTION WHEN duplicate_object THEN \
           ALTER ROLE \"{name}\" WITH LOGIN NOBYPASSRLS PASSWORD '{password}'; \
         END $$;"
    );
    admin
        .execute(sql.as_str())
        .await
        .unwrap_or_else(|e| panic!("failed to ensure e2e app role '{name}': {e}"));
}

pub(super) async fn grant_app_schema(owner: &mut PgConnection, owner_role: &str, app_role: &str) {
    for stmt in [
        format!("GRANT USAGE, CREATE ON SCHEMA public TO \"{app_role}\""),
        format!(
            "ALTER DEFAULT PRIVILEGES FOR ROLE \"{owner_role}\" IN SCHEMA public \
             GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO \"{app_role}\""
        ),
        format!(
            "ALTER DEFAULT PRIVILEGES FOR ROLE \"{owner_role}\" IN SCHEMA public \
             GRANT USAGE, SELECT ON SEQUENCES TO \"{app_role}\""
        ),
    ] {
        owner.execute(stmt.as_str()).await.unwrap_or_else(|e| {
            panic!("failed to grant public schema to app role '{app_role}': {e}")
        });
    }
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
