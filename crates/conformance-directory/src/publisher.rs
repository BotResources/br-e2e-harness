use br_util_directory::DirectoryPublisher;

use crate::anchor::DirectorySnapshotWire;
use crate::error::{ConformanceError, Result};
use crate::harness::DirectoryHarness;
use crate::kv_read::{read_groups, read_meta, read_users};
use crate::outcome::{CheckId, CheckOutcome};
use crate::source::AnchorSource;

pub async fn publisher_floor(snapshot: &DirectorySnapshotWire) -> Result<CheckOutcome> {
    let id = CheckId::PublisherFloor;
    let expected = "reconcile publishes _meta + every user, a second reconcile is a no-op, the published wire \
                    matches the source, and dropping a user orphan-deletes its KV key";
    let harness = DirectoryHarness::start().await?;
    let outcome = publisher_floor_inner(id, expected, &harness, snapshot).await;
    harness.shutdown().await;
    outcome
}

async fn publisher_floor_inner(
    id: CheckId,
    expected: &str,
    harness: &DirectoryHarness,
    snapshot: &DirectorySnapshotWire,
) -> Result<CheckOutcome> {
    let mut source = AnchorSource::from_snapshot(snapshot)?;
    let publisher = DirectoryPublisher::open(harness.fabric())
        .await
        .map_err(|e| ConformanceError::Directory(format!("open publisher: {e}")))?;

    publisher
        .reconcile(&source)
        .await
        .map_err(|e| ConformanceError::Directory(format!("first reconcile: {e}")))?;

    match read_meta(harness.nats()).await? {
        Some(meta) if meta.publishes_users() => {}
        Some(_) => {
            return Ok(CheckOutcome::fail(
                id,
                expected,
                "_meta omits users",
                "the floor must publish users",
            ));
        }
        None => {
            return Ok(CheckOutcome::fail(
                id,
                expected,
                "_meta absent",
                "reconcile must write the manifest",
            ));
        }
    }

    let published_users = read_users(harness.nats()).await?;
    if &published_users != source.users() {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            format!("{} published user(s)", published_users.len()),
            "the published user wire did not round-trip identically to the source through the lib types",
        ));
    }

    let ops_second = before_after_keys(harness, &source).await?;
    if ops_second != 0 {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            format!("{ops_second} key(s) changed on the second reconcile"),
            "a second reconcile against an unchanged source must apply the empty diff (idempotent)",
        ));
    }

    let Some((orphan_id, _)) = source.users().iter().next().map(|(k, v)| (*k, v.clone())) else {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            "no user to orphan",
            "the source must carry at least one user",
        ));
    };
    source.drop_user(&orphan_id);
    publisher
        .reconcile(&source)
        .await
        .map_err(|e| ConformanceError::Directory(format!("orphan reconcile: {e}")))?;
    let after_drop = read_users(harness.nats()).await?;
    if after_drop.contains_key(&orphan_id) {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            format!("orphan {orphan_id} still present"),
            "a user removed from the source must be DELETE'd from KV (PII propagation)",
        ));
    }
    if after_drop.len() != published_users.len() - 1 {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            format!("{} user(s) after orphan-delete", after_drop.len()),
            "orphan-delete must remove exactly the dropped user",
        ));
    }

    Ok(CheckOutcome::pass(
        id,
        expected,
        format!(
            "{} user(s) published, idempotent, orphan-deleted",
            published_users.len()
        ),
    ))
}

async fn before_after_keys(harness: &DirectoryHarness, source: &AnchorSource) -> Result<usize> {
    let before = read_users(harness.nats()).await?;
    let publisher = DirectoryPublisher::open(harness.fabric())
        .await
        .map_err(|e| ConformanceError::Directory(format!("open publisher: {e}")))?;
    publisher
        .reconcile(source)
        .await
        .map_err(|e| ConformanceError::Directory(format!("idempotency reconcile: {e}")))?;
    let after = read_users(harness.nats()).await?;
    Ok(symmetric_diff(&before, &after))
}

fn symmetric_diff<T: PartialEq>(
    before: &std::collections::BTreeMap<uuid::Uuid, T>,
    after: &std::collections::BTreeMap<uuid::Uuid, T>,
) -> usize {
    let mut changed = 0;
    for (id, value) in after {
        match before.get(id) {
            Some(prior) if prior == value => {}
            _ => changed += 1,
        }
    }
    for id in before.keys() {
        if !after.contains_key(id) {
            changed += 1;
        }
    }
    changed
}

pub async fn publisher_groups_optional(snapshot: &DirectorySnapshotWire) -> Result<CheckOutcome> {
    let id = CheckId::PublisherGroupsOptional;
    let expected = "a users-only source publishes no groups and a _meta that omits groups (groups are optional, gated on _meta)";
    let harness = DirectoryHarness::start().await?;
    let outcome = publisher_groups_optional_inner(id, expected, &harness, snapshot).await;
    harness.shutdown().await;
    outcome
}

async fn publisher_groups_optional_inner(
    id: CheckId,
    expected: &str,
    harness: &DirectoryHarness,
    snapshot: &DirectorySnapshotWire,
) -> Result<CheckOutcome> {
    let source = AnchorSource::from_snapshot(snapshot)?.without_groups();
    let publisher = DirectoryPublisher::open(harness.fabric())
        .await
        .map_err(|e| ConformanceError::Directory(format!("open publisher: {e}")))?;
    publisher
        .reconcile(&source)
        .await
        .map_err(|e| ConformanceError::Directory(format!("users-only reconcile: {e}")))?;

    match read_meta(harness.nats()).await? {
        Some(meta) if meta.publishes_groups() => {
            return Ok(CheckOutcome::fail(
                id,
                expected,
                "_meta still declares groups",
                "a users-only source must publish a manifest that omits groups",
            ));
        }
        Some(meta) if !meta.publishes_users() => {
            return Ok(CheckOutcome::fail(
                id,
                expected,
                "_meta omits users",
                "users-only must still publish users",
            ));
        }
        Some(_) => {}
        None => {
            return Ok(CheckOutcome::fail(
                id,
                expected,
                "_meta absent",
                "reconcile must write the manifest",
            ));
        }
    }

    let groups = read_groups(harness.nats()).await?;
    if !groups.is_empty() {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            format!("{} group key(s) present", groups.len()),
            "a users-only source must write no group keys",
        ));
    }
    let users = read_users(harness.nats()).await?;
    if users.is_empty() {
        return Ok(CheckOutcome::fail(
            id,
            expected,
            "no users published",
            "users-only must still publish users",
        ));
    }
    Ok(CheckOutcome::pass(
        id,
        expected,
        format!("{} user(s), no groups, manifest gated", users.len()),
    ))
}
