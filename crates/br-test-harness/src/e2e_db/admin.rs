use sqlx::{Executor, PgConnection};

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
    use super::pg_host_port;

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
