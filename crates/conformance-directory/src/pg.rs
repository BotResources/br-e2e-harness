use br_test_harness::E2eDatabase;
use br_util_directory::connect_pool;
use sqlx::PgPool;

use crate::error::{ConformanceError, Result};

pub struct ConsumerDb {
    db: E2eDatabase,
    pool: PgPool,
}

impl ConsumerDb {
    pub async fn provision() -> Result<Self> {
        let db = E2eDatabase::create(false, &[]).await;
        let pool = connect_pool(&db.owner_migration_url())
            .await
            .map_err(|e| ConformanceError::Postgres(format!("connect pool: {e}")))?;
        Ok(Self { db, pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn apply_users_only_schema(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE known_users (\
                 user_id    uuid PRIMARY KEY, \
                 email      text NOT NULL, \
                 first_name text, \
                 last_name  text, \
                 extensions jsonb NOT NULL DEFAULT '{}'::jsonb\
             )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| ConformanceError::Postgres(format!("create users-only schema: {e}")))?;
        Ok(())
    }

    pub async fn group_tables_exist(&self) -> Result<bool> {
        let exists: (bool,) = sqlx::query_as(
            "SELECT EXISTS (\
                 SELECT 1 FROM information_schema.tables \
                 WHERE table_name IN ('known_groups', 'known_user_group')\
             )",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| ConformanceError::Postgres(format!("probe group tables: {e}")))?;
        Ok(exists.0)
    }

    pub async fn cleanup(self) {
        self.pool.close().await;
        self.db.cleanup().await;
    }
}
