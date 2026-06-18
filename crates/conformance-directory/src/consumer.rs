use br_core_directory::DirectoryMeta;
use br_util_directory::{
    DirectoryProjector, DirectorySnapshot, KnownUser, PersistedExtensions, migrate,
};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::anchor::DirectorySnapshotWire;
use crate::error::{ConformanceError, Result};
use crate::harness::DirectoryHarness;
use crate::outcome::{CheckId, CheckOutcome};
use crate::pg::ConsumerDb;
use crate::publish_fixture::publish_snapshot;
use crate::source::AnchorSource;

pub async fn consumer_reads_users(snapshot: &DirectorySnapshotWire) -> Result<CheckOutcome> {
    let id = CheckId::ConsumerReadsUsers;
    let expected = "reconcile-on-boot projects KV users into PG, resolve_user returns the carried fields, \
                    and retracting a user orphan-deletes its projection row";
    let harness = DirectoryHarness::start().await?;
    let db = ConsumerDb::provision().await?;
    let outcome = consumer_reads_users_inner(id, expected, &harness, &db, snapshot).await;
    db.cleanup().await;
    harness.shutdown().await;
    outcome
}

async fn consumer_reads_users_inner(
    id: CheckId,
    expected: &str,
    harness: &DirectoryHarness,
    db: &ConsumerDb,
    snapshot: &DirectorySnapshotWire,
) -> Result<CheckOutcome> {
    let source = AnchorSource::from_snapshot(snapshot)?;
    publish_snapshot(harness.fabric(), &source).await?;
    migrate(db.pool())
        .await
        .map_err(|e| ConformanceError::Directory(format!("migrate: {e}")))?;

    let projector = DirectoryProjector::new(harness.fabric().clone(), db.pool().clone());
    let manifest = projector
        .reconcile()
        .await
        .map_err(|e| ConformanceError::Directory(format!("reconcile: {e}")))?;

    let live = load_snapshot(db.pool(), &manifest).await?;
    let Some((expected_id, expected_user)) = source.users().iter().next() else {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            "no user in the source",
            "the snapshot must carry a user",
        ));
    };
    match live.resolve_user(*expected_id) {
        Some(known) if known.email == expected_user.email => {}
        Some(known) => {
            return Ok(CheckOutcome::fail(
                id,
                expected,
                format!("resolved email {:?}", known.email),
                format!(
                    "projected user email did not match the published {:?}",
                    expected_user.email
                ),
            ));
        }
        None => {
            return Ok(CheckOutcome::fail(
                id,
                expected,
                "resolve_user returned None",
                "the projector did not write the published user into known_users",
            ));
        }
    }

    let projector_after = DirectoryProjector::new(harness.fabric().clone(), db.pool().clone());
    let mut retracted = source.clone();
    retracted.drop_user(expected_id);
    publish_snapshot(harness.fabric(), &retracted).await?;
    let manifest = projector_after
        .reconcile()
        .await
        .map_err(|e| ConformanceError::Directory(format!("reconcile after retract: {e}")))?;
    let after = load_snapshot(db.pool(), &manifest).await?;
    if after.resolve_user(*expected_id).is_some() {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            format!("user {expected_id} still projected"),
            "retracting a KV user must orphan-delete its known_users row (PII propagation)",
        ));
    }

    Ok(CheckOutcome::pass(
        id,
        expected,
        format!("resolved {expected_id}, orphan-deleted on retract"),
    ))
}

pub async fn consumer_reads_groups(snapshot: &DirectorySnapshotWire) -> Result<CheckOutcome> {
    let id = CheckId::ConsumerReadsGroups;
    let expected = "with groups in _meta, is_member/group_name resolve; with a users-only _meta they auto-degrade to empty";
    let harness = DirectoryHarness::start().await?;
    let db = ConsumerDb::provision().await?;
    let outcome = consumer_reads_groups_inner(id, expected, &harness, &db, snapshot).await;
    db.cleanup().await;
    harness.shutdown().await;
    outcome
}

async fn consumer_reads_groups_inner(
    id: CheckId,
    expected: &str,
    harness: &DirectoryHarness,
    db: &ConsumerDb,
    snapshot: &DirectorySnapshotWire,
) -> Result<CheckOutcome> {
    let source = AnchorSource::from_snapshot(snapshot)?;
    let Some((group_id, group)) = source
        .groups()
        .iter()
        .find(|(_, g)| !g.member_ids.is_empty())
        .map(|(k, v)| (*k, v.clone()))
    else {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            "no group with members",
            "the snapshot must carry a non-empty group",
        ));
    };
    let member = group.member_ids[0];

    publish_snapshot(harness.fabric(), &source).await?;
    migrate(db.pool())
        .await
        .map_err(|e| ConformanceError::Directory(format!("migrate: {e}")))?;
    let projector = DirectoryProjector::new(harness.fabric().clone(), db.pool().clone());
    let manifest = projector
        .reconcile()
        .await
        .map_err(|e| ConformanceError::Directory(format!("reconcile: {e}")))?;

    let live = load_snapshot(db.pool(), &manifest).await?;
    if live.group_name(group_id).is_none() {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            "group_name returned None with groups published",
            "the projector did not write the group name into known_groups",
        ));
    }
    if !live.is_member(group_id, member) {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            "is_member false for a published member",
            "the projector did not write the membership into known_user_group",
        ));
    }

    let degraded_meta = DirectoryMeta {
        version: br_core_directory::DIRECTORY_META_VERSION,
        entities: vec![br_core_directory::PublishedEntity::Users],
    };
    let degraded = load_snapshot(db.pool(), &degraded_meta).await?;
    if degraded.group_name(group_id).is_some() {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            "group_name still resolves under a users-only manifest",
            "group readers must auto-degrade when _meta omits groups",
        ));
    }
    if degraded.is_member(group_id, member) {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            "is_member still true under a users-only manifest",
            "membership readers must auto-degrade when _meta omits groups",
        ));
    }

    Ok(CheckOutcome::pass(
        id,
        expected,
        "is_member/group_name resolve, then auto-degrade under users-only _meta",
    ))
}

type KnownUserRow = (Uuid, String, Option<String>, Option<String>, Value);

async fn load_snapshot(pool: &PgPool, manifest: &DirectoryMeta) -> Result<DirectorySnapshot> {
    let mut snapshot = DirectorySnapshot::new(manifest);

    let users: Vec<KnownUserRow> =
        sqlx::query_as("SELECT user_id, email, first_name, last_name, extensions FROM known_users")
            .fetch_all(pool)
            .await
            .map_err(|e| ConformanceError::Postgres(format!("read known_users: {e}")))?;
    for (user_id, email, first_name, last_name, extensions) in users {
        snapshot.upsert_user(KnownUser {
            user_id,
            email,
            first_name,
            last_name,
            extensions: PersistedExtensions::from_value(extensions),
        });
    }

    let groups: Vec<(Uuid, String)> = sqlx::query_as("SELECT group_id, name FROM known_groups")
        .fetch_all(pool)
        .await
        .map_err(|e| ConformanceError::Postgres(format!("read known_groups: {e}")))?;
    for (group_id, name) in groups {
        snapshot.upsert_group(group_id, name);
    }

    let members: Vec<(Uuid, Uuid)> =
        sqlx::query_as("SELECT group_id, user_id FROM known_user_group ORDER BY group_id")
            .fetch_all(pool)
            .await
            .map_err(|e| ConformanceError::Postgres(format!("read known_user_group: {e}")))?;
    let mut by_group: std::collections::BTreeMap<Uuid, Vec<Uuid>> =
        std::collections::BTreeMap::new();
    for (group_id, user_id) in members {
        by_group.entry(group_id).or_default().push(user_id);
    }
    for (group_id, member_ids) in by_group {
        snapshot.set_members(group_id, member_ids);
    }

    Ok(snapshot)
}
