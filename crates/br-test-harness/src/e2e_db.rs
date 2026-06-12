use sqlx::{Connection, Executor, PgConnection};

mod admin;
use admin::{drop_db_and_owner, pg_host_port, role_exists};

pub struct E2eDatabase {
    admin_url: String,
    host_port: String,
    db_name: String,
    owner_role: String,
    owner_password: String,
    granted_roles: Vec<String>,
}

impl E2eDatabase {
    pub async fn create(bypassrls: bool, managed_roles: &[&str]) -> Self {
        let suffix = uuid::Uuid::now_v7().simple().to_string();
        let db_name = format!("e2e_{suffix}");
        let owner_role = format!("e2e_owner_{suffix}");
        Self::provision(owner_role, db_name, bypassrls, managed_roles, false).await
    }

    pub async fn create_named(
        owner_name: &str,
        db_name: &str,
        bypassrls: bool,
        managed_roles: &[&str],
    ) -> Self {
        Self::provision(
            owner_name.to_string(),
            db_name.to_string(),
            bypassrls,
            managed_roles,
            true,
        )
        .await
    }

    async fn provision(
        owner_role: String,
        db_name: String,
        bypassrls: bool,
        managed_roles: &[&str],
        drop_first: bool,
    ) -> Self {
        dotenvy::dotenv().ok();

        let admin_url = std::env::var("E2E_PG_ADMIN_URL")
            .or_else(|_| std::env::var("DATABASE_URL"))
            .unwrap_or_else(|_| {
                panic!(
                    "E2E_PG_ADMIN_URL or DATABASE_URL must be set to run e2e DB tests \
                     (e.g. DATABASE_URL=postgresql://user@localhost:5432/postgres)"
                )
            });

        let host_port = pg_host_port(&admin_url);
        let owner_password = format!("pw_{}", uuid::Uuid::now_v7().simple());

        let mut admin = PgConnection::connect(&admin_url)
            .await
            .expect("failed to connect to PG admin URL — is PostgreSQL running?");

        if drop_first {
            drop_db_and_owner(&mut admin, &db_name, &owner_role, &[]).await;
        }

        let bypass_clause = if bypassrls { " BYPASSRLS" } else { "" };
        admin
            .execute(
                format!(
                    "CREATE ROLE \"{owner_role}\" LOGIN PASSWORD '{owner_password}' \
                     NOSUPERUSER CREATEROLE{bypass_clause}"
                )
                .as_str(),
            )
            .await
            .expect("failed to create e2e owner role");

        admin
            .execute(format!("CREATE DATABASE \"{db_name}\" OWNER \"{owner_role}\"").as_str())
            .await
            .expect("failed to create e2e database");

        let mut granted_roles = Vec::new();
        for role in managed_roles {
            if role_exists(&mut admin, role).await {
                admin
                    .execute(
                        format!("GRANT \"{role}\" TO \"{owner_role}\" WITH ADMIN OPTION").as_str(),
                    )
                    .await
                    .unwrap_or_else(|e| panic!("failed to grant role '{role}' to owner: {e}"));
                granted_roles.push((*role).to_string());
            }
        }

        admin.close().await.ok();

        Self {
            admin_url,
            host_port,
            db_name,
            owner_role,
            owner_password,
            granted_roles,
        }
    }

    pub fn owner_url(&self) -> String {
        format!(
            "postgresql://{}:{}@{}/{}",
            self.owner_role, self.owner_password, self.host_port, self.db_name
        )
    }

    pub fn db_name(&self) -> &str {
        &self.db_name
    }

    pub fn owner_role(&self) -> &str {
        &self.owner_role
    }

    pub async fn cleanup(self) {
        let mut admin = match PgConnection::connect(&self.admin_url).await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("warning: e2e cleanup could not connect to admin PG: {e}");
                return;
            }
        };
        drop_db_and_owner(
            &mut admin,
            &self.db_name,
            &self.owner_role,
            &self.granted_roles,
        )
        .await;
        admin.close().await.ok();
    }
}
