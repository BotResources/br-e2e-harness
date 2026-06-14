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
        let pool = connect_pool(&db.owner_url())
            .await
            .map_err(|e| ConformanceError::Postgres(format!("connect pool: {e}")))?;
        Ok(Self { db, pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn cleanup(self) {
        self.pool.close().await;
        self.db.cleanup().await;
    }
}
