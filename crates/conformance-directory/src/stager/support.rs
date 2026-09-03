use br_core_directory::{PublishedGroup, PublishedUser};
use br_util_directory::{DirectoryError, GROUP_NAMESPACE, USER_NAMESPACE};
use uuid::Uuid;

use crate::error::{ConformanceError, Result};
use crate::outcome::{CheckId, CheckOutcome};
use crate::pg::ConsumerDb;
use crate::stager::recording::StagedImpact;

pub(crate) fn user_ref(user_id: Uuid) -> String {
    format!("{USER_NAMESPACE}/{user_id}")
}

pub(crate) fn group_ref(group_id: Uuid) -> String {
    format!("{GROUP_NAMESPACE}/{group_id}")
}

pub(crate) struct Phase<'a> {
    pub(crate) id: CheckId,
    pub(crate) expected: &'a str,
}

impl Phase<'_> {
    pub(crate) fn fail(
        &self,
        name: &str,
        observed: impl Into<String>,
        detail: impl Into<String>,
    ) -> CheckOutcome {
        CheckOutcome::fail(
            self.id,
            self.expected,
            format!("[{name}] {}", observed.into()),
            detail.into(),
        )
    }

    pub(crate) fn reconcile_failed(&self, name: &str, error: DirectoryError) -> CheckOutcome {
        self.fail(
            name,
            format!("reconcile failed: {error}"),
            "this phase must project cleanly; only the rollback phase expects an error",
        )
    }

    pub(crate) fn expect_impacts(
        &self,
        name: &str,
        staged: &[StagedImpact],
        want: &[String],
    ) -> Option<CheckOutcome> {
        let mut got: Vec<String> = staged.iter().map(StagedImpact::reference).collect();
        got.sort();
        let mut want = want.to_vec();
        want.sort();
        if got == want {
            return None;
        }
        Some(self.fail(
            name,
            format!("staged {got:?}"),
            format!("the sink must stage exactly {want:?} for this roster write"),
        ))
    }

    pub(crate) fn expect_all_visible(
        &self,
        name: &str,
        staged: &[StagedImpact],
    ) -> Option<CheckOutcome> {
        let blind: Vec<String> = staged
            .iter()
            .filter(|impact| impact.roster_visible != Some(true))
            .map(StagedImpact::reference)
            .collect();
        if blind.is_empty() {
            return None;
        }
        Some(self.fail(
            name,
            format!("the stager could not read the roster row of {blind:?}"),
            "stage_in must run on the connection that carries the roster write, inside its \
             still-uncommitted transaction, so the row the impact names is already visible to it",
        ))
    }

    pub(crate) fn expect_first_name(
        &self,
        name: &str,
        projected: Option<Option<String>>,
        want: Option<Option<&str>>,
        detail: &str,
    ) -> Option<CheckOutcome> {
        let want = want.map(|inner| inner.map(str::to_string));
        if projected == want {
            return None;
        }
        Some(self.fail(
            name,
            format!("known_users row {projected:?}, expected {want:?}"),
            detail,
        ))
    }
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

pub(crate) fn without_member(base: &PublishedGroup, member: Uuid) -> Result<PublishedGroup> {
    PublishedGroup::new(
        base.name.clone(),
        base.member_ids
            .iter()
            .copied()
            .filter(|id| *id != member)
            .collect(),
        base.extensions().clone(),
    )
    .map_err(|e| ConformanceError::Directory(format!("rebuild group: {e}")))
}

pub(crate) async fn projected_first_name(
    db: &ConsumerDb,
    user_id: Uuid,
) -> Result<Option<Option<String>>> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT first_name FROM known_users WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(db.pool())
            .await
            .map_err(|e| ConformanceError::Postgres(format!("read first_name: {e}")))?;
    Ok(row.map(|(first_name,)| first_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phase() -> Phase<'static> {
        Phase {
            id: CheckId::ConsumerStagerTransaction,
            expected: "the stager contract",
        }
    }

    fn staged(reference: &str, roster_visible: Option<bool>) -> StagedImpact {
        let (namespace, key) = reference.split_once('/').expect("a namespaced reference");
        StagedImpact {
            namespace: namespace.to_string(),
            key: key.to_string(),
            roster_visible,
        }
    }

    #[test]
    fn an_impact_set_is_compared_as_a_set_not_as_a_staging_order() {
        let id = Uuid::from_u128(1);
        let other = Uuid::from_u128(2);
        let outcome = phase().expect_impacts(
            "p",
            &[
                staged(&group_ref(other), Some(true)),
                staged(&user_ref(id), Some(true)),
            ],
            &[user_ref(id), group_ref(other)],
        );
        assert!(outcome.is_none(), "{outcome:?}");
    }

    #[test]
    fn a_missing_or_extra_impact_fails_the_phase() {
        let id = Uuid::from_u128(1);
        assert!(phase().expect_impacts("p", &[], &[user_ref(id)]).is_some());
        assert!(
            phase()
                .expect_impacts("p", &[staged(&user_ref(id), Some(true))], &[])
                .is_some()
        );
    }

    #[test]
    fn an_impact_staged_without_seeing_its_roster_row_fails_the_phase() {
        let id = Uuid::from_u128(1);
        assert!(
            phase()
                .expect_all_visible("p", &[staged(&user_ref(id), Some(true))])
                .is_none()
        );
        for blind in [Some(false), None] {
            assert!(
                phase()
                    .expect_all_visible("p", &[staged(&user_ref(id), blind)])
                    .is_some(),
                "{blind:?} must fail"
            );
        }
    }

    #[test]
    fn an_absent_row_and_a_null_first_name_are_distinct_verdicts() {
        assert!(phase().expect_first_name("p", None, None, "d").is_none());
        assert!(
            phase()
                .expect_first_name("p", Some(None), None, "d")
                .is_some()
        );
        assert!(
            phase()
                .expect_first_name("p", Some(None), Some(None), "d")
                .is_none()
        );
    }
}
