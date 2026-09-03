use br_util_directory::{DirectoryError, USER_NAMESPACE};

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
    Phase, groups_holding, projected_first_name, projected_user, published_user, user_ref,
    with_first_name,
};

pub(crate) async fn user_upsert(
    phase: &Phase<'_>,
    harness: &DirectoryHarness,
    db: &ConsumerDb,
    source: &mut AnchorSource,
    fixture: &Fixture,
) -> Result<Option<CheckOutcome>> {
    clear_impacts(db.pool()).await?;
    let base = match published_user(phase, "user upsert", source, fixture.sibling) {
        Ok(base) => base,
        Err(failure) => return Ok(Some(failure)),
    };
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
    let before = projected_user(db, fixture.refused).await?;
    let sibling = match published_user(phase, "rollback", source, fixture.sibling) {
        Ok(user) => user,
        Err(failure) => return Ok(Some(failure)),
    };
    let refused = match published_user(phase, "rollback", source, fixture.refused) {
        Ok(user) => user,
        Err(failure) => return Ok(Some(failure)),
    };
    source.upsert_user(
        fixture.sibling,
        with_first_name(&sibling, Some("Converged"))?,
    );
    source.upsert_user(fixture.refused, with_first_name(&refused, Some("Refused"))?);
    publish_snapshot(harness.fabric(), source).await?;

    match reconcile_with(
        harness,
        db,
        RecordingStager::refusing(USER_NAMESPACE, fixture.refused),
    )
    .await
    {
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

    let after = projected_user(db, fixture.refused).await?;
    if after != before {
        return Ok(Some(phase.fail(
            "rollback",
            format!("the refused known_users row moved from {before:?} to {after:?}"),
            "stage_in runs inside the roster transaction, so its failure must roll the roster \
             write back and leave every column of the pre-existing row untouched",
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
        "the value the refusal rolled back is unchanged in KV, so the next reconcile with an \
         accepting stager must converge it — proving the earlier phase observed a rollback and \
         not a change that never reached the bucket",
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
    let mut want = groups_holding(source, fixture.deleted_user);
    want.push(user_ref(fixture.deleted_user));
    source.drop_user(&fixture.deleted_user);
    publish_snapshot(harness.fabric(), source).await?;
    if let Err(e) = reconcile_with(harness, db, RecordingStager::recording()).await {
        return Ok(Some(phase.reconcile_failed("user deleted", e)));
    }
    let staged = staged_impacts(db.pool()).await?;
    if let Some(failure) = phase.expect_impacts("user deleted", &staged, &want) {
        return Ok(Some(failure));
    }
    Ok(phase.expect_first_name(
        "user deleted",
        projected_first_name(db, fixture.deleted_user).await?,
        None,
        "the retraction that staged the impacts must itself be committed",
    ))
}
