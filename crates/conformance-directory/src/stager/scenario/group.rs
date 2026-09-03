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
    Phase, group_ref, members_of, published_group, user_ref, with_name, without_member,
};

pub(crate) async fn member_dropped(
    phase: &Phase<'_>,
    harness: &DirectoryHarness,
    db: &ConsumerDb,
    source: &mut AnchorSource,
    fixture: &Fixture,
) -> Result<Option<CheckOutcome>> {
    clear_impacts(db.pool()).await?;
    let group = match published_group(phase, "member dropped", source, fixture.group) {
        Ok(group) => group,
        Err(failure) => return Ok(Some(failure)),
    };
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

pub(crate) async fn group_renamed(
    phase: &Phase<'_>,
    harness: &DirectoryHarness,
    db: &ConsumerDb,
    source: &mut AnchorSource,
    fixture: &Fixture,
) -> Result<Option<CheckOutcome>> {
    clear_impacts(db.pool()).await?;
    let group = match published_group(phase, "group renamed", source, fixture.renamed_group) {
        Ok(group) => group,
        Err(failure) => return Ok(Some(failure)),
    };
    source.upsert_group(fixture.renamed_group, with_name(&group, "renamed")?);
    publish_snapshot(harness.fabric(), source).await?;
    if let Err(e) = reconcile_with(harness, db, RecordingStager::recording()).await {
        return Ok(Some(phase.reconcile_failed("group renamed", e)));
    }
    let staged = staged_impacts(db.pool()).await?;
    if let Some(failure) = phase.expect_impacts(
        "group renamed",
        &staged,
        &[group_ref(fixture.renamed_group)],
    ) {
        return Ok(Some(failure));
    }
    Ok(phase.expect_all_visible("group renamed", &staged))
}

pub(crate) async fn group_deleted(
    phase: &Phase<'_>,
    harness: &DirectoryHarness,
    db: &ConsumerDb,
    source: &mut AnchorSource,
    fixture: &Fixture,
) -> Result<Option<CheckOutcome>> {
    clear_impacts(db.pool()).await?;
    let members = match published_group(phase, "group deleted", source, fixture.group) {
        Ok(group) => members_of(&group),
        Err(failure) => return Ok(Some(failure)),
    };
    if members.is_empty() {
        return Ok(Some(phase.fail(
            "group deleted",
            format!("group {} holds no member", fixture.group),
            "the delete-with-members path stages one impact per unlinked member, so the group \
             must still hold at least one when it is retracted",
        )));
    }
    let mut want = vec![group_ref(fixture.group)];
    want.extend(members.iter().map(|id| user_ref(*id)));
    source.drop_group(&fixture.group);
    publish_snapshot(harness.fabric(), source).await?;
    if let Err(e) = reconcile_with(harness, db, RecordingStager::recording()).await {
        return Ok(Some(phase.reconcile_failed("group deleted", e)));
    }
    let staged = staged_impacts(db.pool()).await?;
    Ok(phase.expect_impacts("group deleted", &staged, &want))
}
