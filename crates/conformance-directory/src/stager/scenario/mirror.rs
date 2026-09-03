use br_util_directory::DirectoryProjector;

use crate::error::Result;
use crate::harness::DirectoryHarness;
use crate::outcome::CheckOutcome;
use crate::pg::ConsumerDb;
use crate::publish_fixture::publish_snapshot;
use crate::source::AnchorSource;
use crate::stager::Fixture;
use crate::stager::recording::{RecordingStager, clear_impacts, staged_impacts};
use crate::stager::scenario::reconcile_with;
use crate::stager::support::{
    Phase, group_ref, projected_first_name, published_user, user_ref, with_first_name,
};

pub(crate) async fn boot(
    phase: &Phase<'_>,
    harness: &DirectoryHarness,
    db: &ConsumerDb,
    source: &AnchorSource,
) -> Result<Option<CheckOutcome>> {
    if let Err(e) = reconcile_with(harness, db, RecordingStager::recording()).await {
        return Ok(Some(phase.reconcile_failed("boot", e)));
    }
    let staged = staged_impacts(db.pool()).await?;
    let want: Vec<String> = source
        .users()
        .keys()
        .map(|id| user_ref(*id))
        .chain(source.groups().keys().map(|id| group_ref(*id)))
        .collect();
    if let Some(failure) = phase.expect_impacts("boot", &staged, &want) {
        return Ok(Some(failure));
    }
    Ok(phase.expect_all_visible("boot", &staged))
}

pub(crate) async fn idempotence(
    phase: &Phase<'_>,
    harness: &DirectoryHarness,
    db: &ConsumerDb,
) -> Result<Option<CheckOutcome>> {
    clear_impacts(db.pool()).await?;
    if let Err(e) = reconcile_with(harness, db, RecordingStager::recording()).await {
        return Ok(Some(phase.reconcile_failed("idempotence", e)));
    }
    let staged = staged_impacts(db.pool()).await?;
    Ok(phase.expect_impacts("idempotence", &staged, &[]))
}

pub(crate) async fn without_stager(
    phase: &Phase<'_>,
    harness: &DirectoryHarness,
    db: &ConsumerDb,
    source: &mut AnchorSource,
    fixture: &Fixture,
) -> Result<Option<CheckOutcome>> {
    clear_impacts(db.pool()).await?;
    let base = match published_user(phase, "without stager", source, fixture.refused) {
        Ok(base) => base,
        Err(failure) => return Ok(Some(failure)),
    };
    source.upsert_user(fixture.refused, with_first_name(&base, Some("Unstaged"))?);
    publish_snapshot(harness.fabric(), source).await?;
    let projected = DirectoryProjector::new(harness.fabric().clone(), db.pool().clone())
        .reconcile()
        .await;
    if let Err(e) = projected {
        return Ok(Some(phase.reconcile_failed("without stager", e)));
    }
    if let Some(failure) = phase.expect_first_name(
        "without stager",
        projected_first_name(db, fixture.refused).await?,
        Some(Some("Unstaged")),
        "an unregistered stager must not change how the roster itself converges",
    ) {
        return Ok(Some(failure));
    }
    let staged = staged_impacts(db.pool()).await?;
    Ok(phase.expect_impacts("without stager", &staged, &[]))
}
