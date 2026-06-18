use std::time::{Duration, Instant};

use br_core_directory::PublishedUser;
use br_util_directory::{ConsumptionScope, DirectoryConsumerConfig, DirectoryProjector};
use uuid::Uuid;

use crate::anchor::DirectorySnapshotWire;
use crate::error::{ConformanceError, Result};
use crate::harness::DirectoryHarness;
use crate::outcome::{CheckId, CheckOutcome};
use crate::pg::ConsumerDb;
use crate::publish_fixture::{publish_added_user, publish_snapshot};
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
    let watch_task = tokio::spawn(async move { watcher.watch().await });

    let live_id = Uuid::now_v7();
    let live_user = PublishedUser::new(
        format!("live-{}@example.com", live_id.simple()),
        Some("Live".to_string()),
        Some("Narrowing".to_string()),
        Default::default(),
    )
    .map_err(|e| ConformanceError::Directory(format!("build live user: {e}")))?;
    publish_added_user(harness.fabric(), &source, live_id, live_user).await?;

    let live_projected = poll_user_row(db, live_id, Duration::from_secs(5)).await?;

    if let Some(joined) = abort_watch(watch_task).await {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            format!("watch errored: {joined}"),
            "a UsersOnly watch must not touch the absent group tables",
        ));
    }

    if !live_projected {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            format!("live user {live_id} never projected"),
            "a UsersOnly watch must apply a live user PUT to known_users",
        ));
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
        format!(
            "UsersOnly projected {user_id} on reconcile and {live_id} on a live PUT, no group DML \
             against the missing tables"
        ),
    ))
}

async fn poll_user_row(db: &ConsumerDb, user_id: Uuid, deadline: Duration) -> Result<bool> {
    let start = Instant::now();
    loop {
        if user_row_exists(db, user_id).await? {
            return Ok(true);
        }
        if start.elapsed() >= deadline {
            return Ok(false);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn abort_watch(
    task: tokio::task::JoinHandle<std::result::Result<(), br_util_directory::DirectoryError>>,
) -> Option<String> {
    task.abort();
    match task.await {
        Ok(Ok(())) => None,
        Ok(Err(e)) => Some(e.to_string()),
        Err(joined) if joined.is_cancelled() => None,
        Err(joined) => Some(joined.to_string()),
    }
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
