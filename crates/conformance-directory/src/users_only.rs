use std::time::Duration;

use br_util_directory::{ConsumptionScope, DirectoryConsumerConfig, DirectoryProjector};

use crate::anchor::DirectorySnapshotWire;
use crate::error::{ConformanceError, Result};
use crate::harness::DirectoryHarness;
use crate::outcome::{CheckId, CheckOutcome};
use crate::pg::ConsumerDb;
use crate::publish_fixture::publish_snapshot;
use crate::source::AnchorSource;

pub async fn users_only_narrows_projection(
    snapshot: &DirectorySnapshotWire,
) -> Result<CheckOutcome> {
    let id = CheckId::ConsumerUsersOnlyNarrows;
    let expected = "a UsersOnly consumer against a schema lacking group tables reconciles + watches \
                    cleanly and projects users, emitting no group DML (any group write would error \
                    on the missing tables)";
    let harness = DirectoryHarness::start().await?;
    let db = ConsumerDb::provision().await?;
    let outcome = users_only_inner(id, expected, &harness, &db, snapshot).await;
    db.cleanup().await;
    harness.shutdown().await;
    outcome
}

async fn users_only_inner(
    id: CheckId,
    expected: &str,
    harness: &DirectoryHarness,
    db: &ConsumerDb,
    snapshot: &DirectorySnapshotWire,
) -> Result<CheckOutcome> {
    let source = AnchorSource::from_snapshot(snapshot)?;
    if source.groups().is_empty() {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            "no groups in the source",
            "the snapshot must carry groups so a UsersOnly scope has something to narrow away",
        ));
    }
    let Some((user_id, _)) = source.first_user() else {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            "no user in the source",
            "the snapshot must carry a user",
        ));
    };

    publish_snapshot(harness.fabric(), &source).await?;
    db.apply_users_only_schema().await?;
    if db.group_tables_exist().await? {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            "group tables present",
            "the users-only schema must lack known_groups / known_user_group for this proof",
        ));
    }

    let config = DirectoryConsumerConfig::default().scope(ConsumptionScope::UsersOnly);
    let projector = DirectoryProjector::with_config(
        harness.fabric().clone(),
        db.pool().clone(),
        config.clone(),
    );
    projector
        .reconcile()
        .await
        .map_err(|e| ConformanceError::Directory(format!("users-only reconcile: {e}")))?;

    if !user_row_exists(db, user_id).await? {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            format!("user {user_id} absent after reconcile"),
            "a UsersOnly consumer must still project users",
        ));
    }

    let watcher =
        DirectoryProjector::with_config(harness.fabric().clone(), db.pool().clone(), config);
    match tokio::time::timeout(Duration::from_millis(750), watcher.watch()).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            return Ok(CheckOutcome::fail(
                id,
                expected,
                format!("watch errored: {e}"),
                "a UsersOnly watch must not touch the absent group tables",
            ));
        }
        Err(_) => {}
    }

    if db.group_tables_exist().await? {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            "group tables materialized",
            "a UsersOnly consumer must never create or write group tables",
        ));
    }

    Ok(CheckOutcome::pass(
        id,
        expected,
        format!("UsersOnly projected {user_id}, no group DML against the missing tables"),
    ))
}

async fn user_row_exists(db: &ConsumerDb, user_id: uuid::Uuid) -> Result<bool> {
    let row: (bool,) =
        sqlx::query_as("SELECT EXISTS (SELECT 1 FROM known_users WHERE user_id = $1)")
            .bind(user_id)
            .fetch_one(db.pool())
            .await
            .map_err(|e| ConformanceError::Postgres(format!("probe user row: {e}")))?;
    Ok(row.0)
}
