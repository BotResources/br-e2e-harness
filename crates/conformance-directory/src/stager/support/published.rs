use br_core_directory::{PublishedGroup, PublishedUser};
use uuid::Uuid;

use crate::error::{ConformanceError, Result};
use crate::outcome::CheckOutcome;
use crate::source::AnchorSource;
use crate::stager::support::{Phase, group_ref};

pub(crate) fn published_user(
    phase: &Phase<'_>,
    name: &str,
    source: &AnchorSource,
    user_id: Uuid,
) -> std::result::Result<PublishedUser, CheckOutcome> {
    source.users().get(&user_id).cloned().ok_or_else(|| {
        phase.fail(
            name,
            format!("user {user_id} is no longer published"),
            "this phase needs the user still in the source; the phase order is wrong",
        )
    })
}

pub(crate) fn published_group(
    phase: &Phase<'_>,
    name: &str,
    source: &AnchorSource,
    group_id: Uuid,
) -> std::result::Result<PublishedGroup, CheckOutcome> {
    source.groups().get(&group_id).cloned().ok_or_else(|| {
        phase.fail(
            name,
            format!("group {group_id} is no longer published"),
            "this phase needs the group still in the source; the phase order is wrong",
        )
    })
}

pub(crate) fn members_of(group: &PublishedGroup) -> Vec<Uuid> {
    let mut members = group.member_ids.clone();
    members.sort();
    members.dedup();
    members
}

pub(crate) fn groups_holding(source: &AnchorSource, user_id: Uuid) -> Vec<String> {
    source
        .groups()
        .iter()
        .filter(|(_, group)| group.has_member(user_id))
        .map(|(group_id, _)| group_ref(*group_id))
        .collect()
}

pub(crate) fn with_first_name(
    base: &PublishedUser,
    first_name: Option<&str>,
) -> Result<PublishedUser> {
    PublishedUser::new(
        base.email.clone(),
        first_name.map(str::to_string),
        base.last_name.clone(),
        base.extensions().clone(),
    )
    .map_err(|e| ConformanceError::Directory(format!("rebuild user: {e}")))
}

pub(crate) fn with_name(base: &PublishedGroup, name: &str) -> Result<PublishedGroup> {
    rebuild_group(name, base.member_ids.clone(), base)
}

pub(crate) fn without_member(base: &PublishedGroup, member: Uuid) -> Result<PublishedGroup> {
    let members = base
        .member_ids
        .iter()
        .copied()
        .filter(|id| *id != member)
        .collect();
    rebuild_group(&base.name, members, base)
}

fn rebuild_group(
    name: &str,
    member_ids: Vec<Uuid>,
    base: &PublishedGroup,
) -> Result<PublishedGroup> {
    PublishedGroup::new(name.to_string(), member_ids, base.extensions().clone())
        .map_err(|e| ConformanceError::Directory(format!("rebuild group: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::BTreeMap;

    fn group(members: &[Uuid]) -> PublishedGroup {
        PublishedGroup::new("crew".to_string(), members.to_vec(), BTreeMap::new())
            .expect("a published group")
    }

    #[test]
    fn a_membership_is_read_as_a_deduplicated_sorted_set() {
        let first = Uuid::from_u128(1);
        let second = Uuid::from_u128(2);
        assert_eq!(
            members_of(&group(&[second, first, second])),
            vec![first, second]
        );
        assert!(members_of(&group(&[])).is_empty());
    }

    #[test]
    fn a_rebuilt_group_keeps_every_field_the_phase_does_not_move() {
        let member = Uuid::from_u128(1);
        let other = Uuid::from_u128(2);
        let base = group(&[member, other]);

        let renamed = with_name(&base, "renamed").expect("rename");
        assert_eq!(renamed.name, "renamed");
        assert_eq!(renamed.member_ids, base.member_ids);

        let narrowed = without_member(&base, other).expect("drop a member");
        assert_eq!(narrowed.name, base.name);
        assert_eq!(narrowed.member_ids, vec![member]);
    }

    #[test]
    fn a_rebuilt_user_keeps_every_field_the_phase_does_not_move() {
        let base = PublishedUser::new(
            "a@b".to_string(),
            Some("Ada".to_string()),
            Some("Lovelace".to_string()),
            BTreeMap::new(),
        )
        .expect("a published user");
        let renamed = with_first_name(&base, Some("Renamed")).expect("rename");
        assert_eq!(renamed.first_name.as_deref(), Some("Renamed"));
        assert_eq!(renamed.email, base.email);
        assert_eq!(renamed.last_name, base.last_name);
        assert_eq!(
            with_first_name(&base, None).expect("clear").first_name,
            None
        );
    }
}
