use std::sync::Arc;

use br_util_directory::{DirectoryError, DirectoryProjector, USER_NAMESPACE};

use crate::error::Result;
use crate::harness::DirectoryHarness;
use crate::outcome::CheckOutcome;
use crate::pg::ConsumerDb;
use crate::publish_fixture::publish_snapshot;
use crate::source::AnchorSource;
use crate::stager::Fixture;
use crate::stager::recording::{RecordingStager, clear_impacts, staged_impacts};
use crate::stager::support::{
    Phase, group_ref, projected_first_name, user_ref, with_first_name, without_member,
};

async fn reconcile_with(
    harness: &DirectoryHarness,
    db: &ConsumerDb,
    stager: RecordingStager,
) -> std::result::Result<(), DirectoryError> {
    DirectoryProjector::new(harness.fabric().clone(), db.pool().clone())
        .with_impact_stager(Arc::new(stager))
        .reconcile()
        .await
        .map(|_| ())
}

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

pub(crate) async fn user_upsert(
    phase: &Phase<'_>,
    harness: &DirectoryHarness,
    db: &ConsumerDb,
    source: &mut AnchorSource,
    fixture: &Fixture,
) -> Result<Option<CheckOutcome>> {
    clear_impacts(db.pool()).await?;
    let base = source.users()[&fixture.sibling].clone();
    source.upsert_user(fixture.sibling, with_first_name(&base, Some("Renamed"))?);
    publish_snapshot(harness.fabric(), source).await?;
    if let Err(e) = reconcile_with(harness, db, RecordingStager::recording()).await {
        return Ok(Some(phase.reconcile_failed("user upsert", e)));
    }
    let staged = staged_impacts(db.pool()).await?;
    if let Some(failure) =
        phase.expect_impacts("user upsert", &staged, &[user_ref(fixture.sibling)])
    {
        return Ok(Some(failure));
    }
    if let Some(failure) = phase.expect_all_visible("user upsert", &staged) {
        return Ok(Some(failure));
    }
    Ok(phase.expect_first_name(
        "user upsert",
        projected_first_name(db, fixture.sibling).await?,
        Some(Some("Renamed")),
        "the roster write that staged the impact must itself be committed",
    ))
}

pub(crate) async fn rollback(
    phase: &Phase<'_>,
    harness: &DirectoryHarness,
    db: &ConsumerDb,
    source: &mut AnchorSource,
    fixture: &Fixture,
) -> Result<Option<CheckOutcome>> {
    clear_impacts(db.pool()).await?;
    let before = projected_first_name(db, fixture.refused).await?;
    let sibling = source.users()[&fixture.sibling].clone();
    let refused = source.users()[&fixture.refused].clone();
    source.upsert_user(
        fixture.sibling,
        with_first_name(&sibling, Some("Converged"))?,
    );
    source.upsert_user(fixture.refused, with_first_name(&refused, Some("Refused"))?);
    publish_snapshot(harness.fabric(), source).await?;

    let outcome = reconcile_with(
        harness,
        db,
        RecordingStager::refusing(USER_NAMESPACE, fixture.refused),
    )
    .await;
    match outcome {
        Err(DirectoryError::Stager(_)) => {}
        Err(other) => {
            return Ok(Some(phase.fail(
                "rollback",
                format!("reconcile failed with {other}"),
                "a stager error must surface unchanged as DirectoryError::Stager",
            )));
        }
        Ok(()) => {
            return Ok(Some(phase.fail(
                "rollback",
                "reconcile succeeded",
                "a stager that refuses an impact must fail the projection, never be swallowed",
            )));
        }
    }

    let after = projected_first_name(db, fixture.refused).await?;
    if after != before {
        return Ok(Some(phase.fail(
            "rollback",
            format!("the refused user moved from {before:?} to {after:?}"),
            "stage_in runs inside the roster transaction, so its failure must roll the roster \
             write back and leave the pre-existing row untouched",
        )));
    }
    let staged = staged_impacts(db.pool()).await?;
    if let Some(failure) = phase.expect_impacts("rollback", &staged, &[user_ref(fixture.sibling)]) {
        return Ok(Some(failure));
    }
    if let Some(failure) = phase.expect_first_name(
        "rollback",
        projected_first_name(db, fixture.sibling).await?,
        Some(Some("Converged")),
        "a sibling key committed before the refusal must stay converged",
    ) {
        return Ok(Some(failure));
    }

    clear_impacts(db.pool()).await?;
    if let Err(e) = reconcile_with(harness, db, RecordingStager::recording()).await {
        return Ok(Some(phase.reconcile_failed("rollback recovery", e)));
    }
    Ok(phase.expect_first_name(
        "rollback recovery",
        projected_first_name(db, fixture.refused).await?,
        Some(Some("Refused")),
        "the published value the refusal rolled back is unchanged in KV, so the next reconcile          with an accepting stager must converge it — proving the earlier phase observed a          rollback and not an unpublished change",
    ))
}

pub(crate) async fn member_dropped(
    phase: &Phase<'_>,
    harness: &DirectoryHarness,
    db: &ConsumerDb,
    source: &mut AnchorSource,
    fixture: &Fixture,
) -> Result<Option<CheckOutcome>> {
    clear_impacts(db.pool()).await?;
    let group = source.groups()[&fixture.group].clone();
    source.upsert_group(
        fixture.group,
        without_member(&group, fixture.dropped_member)?,
    );
    publish_snapshot(harness.fabric(), source).await?;
    if let Err(e) = reconcile_with(harness, db, RecordingStager::recording()).await {
        return Ok(Some(phase.reconcile_failed("member dropped", e)));
    }
    let staged = staged_impacts(db.pool()).await?;
    Ok(phase.expect_impacts(
        "member dropped",
        &staged,
        &[group_ref(fixture.group), user_ref(fixture.dropped_member)],
    ))
}

pub(crate) async fn user_deleted(
    phase: &Phase<'_>,
    harness: &DirectoryHarness,
    db: &ConsumerDb,
    source: &mut AnchorSource,
    fixture: &Fixture,
) -> Result<Option<CheckOutcome>> {
    clear_impacts(db.pool()).await?;
    source.drop_user(&fixture.deleted_user);
    publish_snapshot(harness.fabric(), source).await?;
    if let Err(e) = reconcile_with(harness, db, RecordingStager::recording()).await {
        return Ok(Some(phase.reconcile_failed("user deleted", e)));
    }
    let staged = staged_impacts(db.pool()).await?;
    if let Some(failure) = phase.expect_impacts(
        "user deleted",
        &staged,
        &[user_ref(fixture.deleted_user), group_ref(fixture.group)],
    ) {
        return Ok(Some(failure));
    }
    Ok(phase.expect_first_name(
        "user deleted",
        projected_first_name(db, fixture.deleted_user).await?,
        None,
        "the retraction that staged the impacts must itself be committed",
    ))
}

pub(crate) async fn without_stager(
    phase: &Phase<'_>,
    harness: &DirectoryHarness,
    db: &ConsumerDb,
    source: &mut AnchorSource,
    fixture: &Fixture,
) -> Result<Option<CheckOutcome>> {
    clear_impacts(db.pool()).await?;
    let base = source.users()[&fixture.refused].clone();
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
