use br_util_directory::{DirectoryError, GROUP_NAMESPACE, Impact, ImpactStager, USER_NAMESPACE};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::error::{ConformanceError, Result};

pub const IMPACT_TABLE: &str = "conformance_impacts";

const CREATE: &str = "CREATE TABLE conformance_impacts (\
     seq            bigserial PRIMARY KEY, \
     namespace      text NOT NULL, \
     key            text NOT NULL, \
     roster_visible boolean\
 )";

const INSERT: &str =
    "INSERT INTO conformance_impacts (namespace, key, roster_visible) VALUES ($1, $2, $3)";

const READ: &str = "SELECT namespace, key, roster_visible FROM conformance_impacts ORDER BY seq";

#[derive(Debug, thiserror::Error)]
#[error("the conformance stager refused {namespace}/{key}")]
pub struct StagerFault {
    namespace: String,
    key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedImpact {
    pub namespace: String,
    pub key: String,
    pub roster_visible: Option<bool>,
}

impl StagedImpact {
    pub fn reference(&self) -> String {
        format!("{}/{}", self.namespace, self.key)
    }
}

pub struct RecordingStager {
    refuse: Option<String>,
}

impl RecordingStager {
    pub fn recording() -> Self {
        Self { refuse: None }
    }

    pub fn refusing(namespace: &str, key: Uuid) -> Self {
        Self {
            refuse: Some(format!("{namespace}/{key}")),
        }
    }
}

#[async_trait::async_trait]
impl ImpactStager for RecordingStager {
    async fn stage_in(
        &self,
        conn: &mut PgConnection,
        impacts: &[Impact],
    ) -> std::result::Result<(), DirectoryError> {
        for impact in impacts {
            let Impact::ForeignChanged { foreign } = impact else {
                continue;
            };
            let reference = format!("{}/{}", foreign.namespace(), foreign.key());
            if self.refuse.as_deref() == Some(reference.as_str()) {
                return Err(DirectoryError::Stager(Box::new(StagerFault {
                    namespace: foreign.namespace().to_string(),
                    key: foreign.key().to_string(),
                })));
            }
            let visible = roster_visible(conn, foreign.namespace(), foreign.key()).await?;
            sqlx::query(INSERT)
                .bind(foreign.namespace())
                .bind(foreign.key())
                .bind(visible)
                .execute(&mut *conn)
                .await?;
        }
        Ok(())
    }
}

async fn roster_visible(
    conn: &mut PgConnection,
    namespace: &str,
    key: &str,
) -> std::result::Result<Option<bool>, DirectoryError> {
    let probe = match namespace {
        USER_NAMESPACE => "SELECT EXISTS (SELECT 1 FROM known_users WHERE user_id = $1)",
        GROUP_NAMESPACE => "SELECT EXISTS (SELECT 1 FROM known_groups WHERE group_id = $1)",
        _ => return Ok(None),
    };
    let Ok(id) = Uuid::parse_str(key) else {
        return Ok(None);
    };
    let (exists,): (bool,) = sqlx::query_as(probe).bind(id).fetch_one(conn).await?;
    Ok(Some(exists))
}

pub async fn create_impact_table(pool: &PgPool) -> Result<()> {
    sqlx::query(CREATE)
        .execute(pool)
        .await
        .map_err(|e| ConformanceError::Postgres(format!("create {IMPACT_TABLE}: {e}")))?;
    Ok(())
}

pub async fn staged_impacts(pool: &PgPool) -> Result<Vec<StagedImpact>> {
    let rows: Vec<(String, String, Option<bool>)> = sqlx::query_as(READ)
        .fetch_all(pool)
        .await
        .map_err(|e| ConformanceError::Postgres(format!("read {IMPACT_TABLE}: {e}")))?;
    Ok(rows
        .into_iter()
        .map(|(namespace, key, roster_visible)| StagedImpact {
            namespace,
            key,
            roster_visible,
        })
        .collect())
}

pub async fn clear_impacts(pool: &PgPool) -> Result<()> {
    sqlx::query("TRUNCATE conformance_impacts")
        .execute(pool)
        .await
        .map_err(|e| ConformanceError::Postgres(format!("truncate {IMPACT_TABLE}: {e}")))?;
    Ok(())
}
