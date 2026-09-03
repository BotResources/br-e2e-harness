mod recording;
mod scenario;
mod support;

use uuid::Uuid;

use crate::anchor::DirectorySnapshotWire;
use crate::error::{ConformanceError, Result};
use crate::harness::DirectoryHarness;
use crate::outcome::{CheckId, CheckOutcome};
use crate::pg::ConsumerDb;
use crate::publish_fixture::publish_snapshot;
use crate::source::AnchorSource;

use br_util_directory::migrate;
use support::Phase;

pub use recording::{
    IMPACT_TABLE, RecordingStager, StagedImpact, StagerFault, clear_impacts, create_impact_table,
    staged_impacts,
};

pub async fn stager_stages_in_the_projection_transaction(
    snapshot: &DirectorySnapshotWire,
) -> Result<CheckOutcome> {
    let id = CheckId::ConsumerStagerTransaction;
    let expected = "a registered ImpactStager runs on the sink's own connection inside the roster \
                    transaction: the impacts of a committed write are durable with it, a stager \
                    failure rolls its roster write back while a sibling key still converges, an \
                    unregistered stager stages nothing, and the six roster writes the Go anchor \
                    can drive — user upsert, group upsert adding a member, group upsert dropping \
                    a member, group rename, user delete, group delete — each stage exactly the \
                    entities they make unnameable afterwards";
    let harness = DirectoryHarness::start().await?;
    let db = ConsumerDb::provision().await?;
    let outcome = run(id, expected, &harness, &db, snapshot).await;
    db.cleanup().await;
    harness.shutdown().await;
    outcome
}

pub(crate) struct Fixture {
    pub(crate) sibling: Uuid,
    pub(crate) refused: Uuid,
    pub(crate) group: Uuid,
    pub(crate) renamed_group: Uuid,
    pub(crate) dropped_member: Uuid,
    pub(crate) deleted_user: Uuid,
}

fn plan(source: &AnchorSource) -> std::result::Result<Fixture, String> {
    let mut users = source.users().keys().copied();
    let sibling = users.next().ok_or("the snapshot must carry two users")?;
    let refused = users.last().ok_or("the snapshot must carry two users")?;
    let (group, published) = source
        .groups()
        .iter()
        .find(|(_, group)| group.member_ids.len() >= 2)
        .ok_or("the snapshot must carry a group with at least two members")?;
    let renamed_group = *source
        .groups()
        .keys()
        .find(|id| *id != group)
        .ok_or("the snapshot must carry a second group for the name-only upsert")?;
    let mut members = published.member_ids.clone();
    members.sort();
    let dropped_member = *members.last().expect("a member");
    let deleted_user = *members.first().expect("a member");
    if refused == deleted_user {
        return Err(format!(
            "the refused user {refused} is also the deleted user: the later phases would drive a \
             row the delete phase removed"
        ));
    }
    Ok(Fixture {
        sibling,
        refused,
        group: *group,
        renamed_group,
        dropped_member,
        deleted_user,
    })
}

async fn run(
    id: CheckId,
    expected: &str,
    harness: &DirectoryHarness,
    db: &ConsumerDb,
    snapshot: &DirectorySnapshotWire,
) -> Result<CheckOutcome> {
    let phase = Phase { id, expected };
    let mut source = AnchorSource::from_snapshot(snapshot)?;
    let fixture = match plan(&source) {
        Ok(fixture) => fixture,
        Err(detail) => return Ok(phase.fail("plan", "unusable anchor snapshot", detail)),
    };

    publish_snapshot(harness.fabric(), &source).await?;
    migrate(db.pool())
        .await
        .map_err(|e| ConformanceError::Directory(format!("migrate: {e}")))?;
    create_impact_table(db.pool()).await?;

    if let Some(failure) = scenario::boot(&phase, harness, db, &source).await? {
        return Ok(failure);
    }
    if let Some(failure) = scenario::idempotence(&phase, harness, db).await? {
        return Ok(failure);
    }
    if let Some(failure) = scenario::user_upsert(&phase, harness, db, &mut source, &fixture).await?
    {
        return Ok(failure);
    }
    if let Some(failure) = scenario::rollback(&phase, harness, db, &mut source, &fixture).await? {
        return Ok(failure);
    }
    if let Some(failure) =
        scenario::member_dropped(&phase, harness, db, &mut source, &fixture).await?
    {
        return Ok(failure);
    }
    if let Some(failure) =
        scenario::group_renamed(&phase, harness, db, &mut source, &fixture).await?
    {
        return Ok(failure);
    }
    if let Some(failure) =
        scenario::user_deleted(&phase, harness, db, &mut source, &fixture).await?
    {
        return Ok(failure);
    }
    if let Some(failure) =
        scenario::group_deleted(&phase, harness, db, &mut source, &fixture).await?
    {
        return Ok(failure);
    }
    if let Some(failure) =
        scenario::without_stager(&phase, harness, db, &mut source, &fixture).await?
    {
        return Ok(failure);
    }

    Ok(CheckOutcome::pass(
        id,
        expected,
        format!(
            "impacts staged inside the roster transaction; the refused {} rolled its own upsert \
             back while {} converged; user upsert, group upsert adding and dropping a member, \
             group rename, user delete and group delete each staged exactly what they unname; \
             with no stager registered nothing was staged",
            fixture.refused, fixture.sibling
        ),
    ))
}
