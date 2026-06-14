use sqlx::{Connection, Executor, PgConnection};

mod admin;
use admin::{
    drop_db_and_owner, ensure_app_role, ensure_owner_role, grant_app_schema, pg_host_port,
    role_exists, teardown_blocking,
};

pub struct E2eDatabase {
    admin_url: String,
    host_port: String,
    db_name: String,
    owner_role: String,
    owner_password: String,
    granted_roles: Vec<String>,
    app_role: Option<AppRole>,
    torn_down: bool,
}

struct AppRole {
    name: String,
    password: String,
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

        ensure_owner_role(&mut admin, &owner_role, &owner_password, bypassrls).await;

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
            app_role: None,
            torn_down: false,
        }
    }

    pub async fn with_app_role(mut self, name: &str, password: &str) -> Self {
        let mut admin = PgConnection::connect(&self.admin_url)
            .await
            .expect("failed to connect to PG admin URL to provision the app role");
        ensure_app_role(&mut admin, name, password).await;
        admin
            .execute(
                format!(
                    "GRANT CONNECT ON DATABASE \"{}\" TO \"{name}\"",
                    self.db_name
                )
                .as_str(),
            )
            .await
            .unwrap_or_else(|e| {
                panic!(
                    "failed to grant connect on '{}' to '{name}': {e}",
                    self.db_name
                )
            });
        admin.close().await.ok();

        let owner_url = self.owner_url();
        let mut owner = PgConnection::connect(&owner_url)
            .await
            .expect("failed to connect as owner to grant the app role public-schema rights");
        grant_app_schema(&mut owner, &self.owner_role, name).await;
        owner.close().await.ok();

        self.app_role = Some(AppRole {
            name: name.to_string(),
            password: password.to_string(),
        });
        self
    }

    pub fn owner_url(&self) -> String {
        format!(
            "postgresql://{}:{}@{}/{}",
            self.owner_role, self.owner_password, self.host_port, self.db_name
        )
    }

    pub fn app_url(&self) -> String {
        let app = self
            .app_role
            .as_ref()
            .expect("app_url requires with_app_role(name, password) to have been called");
        format!(
            "postgresql://{}:{}@{}/{}",
            app.name, app.password, self.host_port, self.db_name
        )
    }

    pub fn admin_url(&self) -> &str {
        &self.admin_url
    }

    pub fn db_name(&self) -> &str {
        &self.db_name
    }

    pub fn owner_role(&self) -> &str {
        &self.owner_role
    }

    pub fn app_role(&self) -> Option<&str> {
        self.app_role.as_ref().map(|r| r.name.as_str())
    }

    pub async fn cleanup(mut self) {
        self.torn_down = true;
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

impl Drop for E2eDatabase {
    fn drop(&mut self) {
        if self.torn_down {
            return;
        }
        self.torn_down = true;
        teardown_blocking(
            self.admin_url.clone(),
            self.db_name.clone(),
            self.owner_role.clone(),
            std::mem::take(&mut self.granted_roles),
        );
    }
}
