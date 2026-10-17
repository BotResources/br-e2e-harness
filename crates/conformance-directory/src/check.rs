use std::fmt::Display;

use crate::error::{ConformanceError, Result};
use crate::outcome::{CheckId, CheckOutcome};

pub(crate) enum Exit {
    Verdict(CheckOutcome),
    Fixture(ConformanceError),
}

impl From<ConformanceError> for Exit {
    fn from(error: ConformanceError) -> Self {
        Exit::Fixture(error)
    }
}

pub(crate) type Run = std::result::Result<CheckOutcome, Exit>;

pub(crate) fn finish(run: Run) -> Result<CheckOutcome> {
    match run {
        Ok(outcome) | Err(Exit::Verdict(outcome)) => Ok(outcome),
        Err(Exit::Fixture(error)) => Err(error),
    }
}

pub(crate) struct Check<'a> {
    id: CheckId,
    expected: &'a str,
}

impl<'a> Check<'a> {
    pub(crate) fn new(id: CheckId, expected: &'a str) -> Self {
        Self { id, expected }
    }

    pub(crate) fn system<T, E: Display>(
        &self,
        stage: &str,
        result: std::result::Result<T, E>,
    ) -> std::result::Result<T, Exit> {
        result.map_err(|error| {
            Exit::Verdict(CheckOutcome::system_failed(
                self.id,
                self.expected,
                stage,
                error,
            ))
        })
    }

    pub(crate) fn read_back<T>(
        &self,
        stage: &str,
        result: Result<T>,
    ) -> std::result::Result<T, Exit> {
        match result {
            Ok(value) => Ok(value),
            Err(error @ ConformanceError::NonConformantWire { .. }) => Err(Exit::Verdict(
                CheckOutcome::system_failed(self.id, self.expected, stage, error),
            )),
            Err(error) => Err(Exit::Fixture(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outcome::CheckStatus;

    const ID: CheckId = CheckId::PublisherFloor;

    fn wire_error() -> ConformanceError {
        ConformanceError::NonConformantWire {
            key: "identity/users/not-a-uuid".to_string(),
            ty: "PublishedUser",
            cause: "the key is not a users/{uuid} key".to_string(),
        }
    }

    #[test]
    fn a_failing_call_under_test_is_a_failed_check_not_an_error() {
        let check = Check::new(ID, "expected");
        let run = check
            .system::<(), _>("first reconcile", Err("boom"))
            .map(|()| unreachable!());
        let outcome = finish(run).expect("a system failure is a verdict");
        assert_eq!(outcome.status, CheckStatus::Fail);
        assert!(outcome.observed.contains("first reconcile"));
        assert!(outcome.detail.unwrap().contains("boom"));
    }

    #[test]
    fn a_succeeding_call_under_test_passes_its_value_through() {
        let check = Check::new(ID, "expected");
        assert_eq!(check.system::<_, String>("stage", Ok(7)).ok(), Some(7));
    }

    #[test]
    fn a_non_conformant_read_back_is_a_failed_check_naming_the_key() {
        let check = Check::new(ID, "expected");
        let run = check
            .read_back::<()>("read users", Err(wire_error()))
            .map(|()| unreachable!());
        let outcome = finish(run).expect("a non-conformant read-back is a verdict");
        assert_eq!(outcome.status, CheckStatus::Fail);
        assert!(
            outcome
                .detail
                .unwrap()
                .contains("identity/users/not-a-uuid")
        );
    }

    #[test]
    fn an_infrastructure_error_on_a_read_back_stays_an_error() {
        let check = Check::new(ID, "expected");
        let run = check
            .read_back::<()>(
                "read users",
                Err(ConformanceError::Jetstream("down".to_string())),
            )
            .map(|()| unreachable!());
        assert!(matches!(finish(run), Err(ConformanceError::Jetstream(_))));
    }

    #[test]
    fn a_fixture_error_converts_through_the_question_mark() {
        fn step() -> Run {
            Err(ConformanceError::Directory("migrate".to_string()))?;
            unreachable!()
        }
        assert!(matches!(
            finish(step()),
            Err(ConformanceError::Directory(_))
        ));
    }
}
