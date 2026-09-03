use serde_json::Value;
use uuid::Uuid;

use crate::error::{ConformanceError, Result};
use crate::pg::ConsumerDb;

pub(crate) type KnownUserRow = (String, Option<String>, Option<String>, Value);

const ROW: &str =
    "SELECT email, first_name, last_name, extensions FROM known_users WHERE user_id = $1";

pub(crate) async fn projected_user(db: &ConsumerDb, user_id: Uuid) -> Result<Option<KnownUserRow>> {
    sqlx::query_as(ROW)
        .bind(user_id)
        .fetch_optional(db.pool())
        .await
        .map_err(|e| ConformanceError::Postgres(format!("read known_users row: {e}")))
}

pub(crate) async fn projected_first_name(
    db: &ConsumerDb,
    user_id: Uuid,
) -> Result<Option<Option<String>>> {
    Ok(projected_user(db, user_id)
        .await?
        .map(|(_, first_name, _, _)| first_name))
}
