use br_core_scope::ScopeDeclarationError;

use crate::error::{ConformanceError, Result};
use crate::expected::ExpectedDeclaration;
use crate::outcome::CheckId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scenario {
    DeclareWellFormed,
    DeclarationContent,
    ReadinessGated,
    RepublishesSameCorrelationId,
    RejectionStopsReadiness,
    DuplicateConfirmationsTolerated,
    DisabledModeReadyWithoutDeclare,
}

impl Scenario {
    pub fn check_id(self) -> CheckId {
        match self {
            Scenario::DeclareWellFormed => CheckId::DeclareWellFormed,
            Scenario::DeclarationContent => CheckId::DeclarationContent,
            Scenario::ReadinessGated => CheckId::ReadinessGated,
            Scenario::RepublishesSameCorrelationId => CheckId::RepublishesSameCorrelationId,
            Scenario::RejectionStopsReadiness => CheckId::RejectionStopsReadiness,
            Scenario::DuplicateConfirmationsTolerated => CheckId::DuplicateConfirmationsTolerated,
            Scenario::DisabledModeReadyWithoutDeclare => CheckId::DisabledModeReadyWithoutDeclare,
        }
    }

    pub fn code(self) -> &'static str {
        self.check_id().code()
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match CheckId::from_code(code)? {
            CheckId::DeclareWellFormed => Some(Scenario::DeclareWellFormed),
            CheckId::DeclarationContent => Some(Scenario::DeclarationContent),
            CheckId::ReadinessGated => Some(Scenario::ReadinessGated),
            CheckId::RepublishesSameCorrelationId => Some(Scenario::RepublishesSameCorrelationId),
            CheckId::RejectionStopsReadiness => Some(Scenario::RejectionStopsReadiness),
            CheckId::DuplicateConfirmationsTolerated => {
                Some(Scenario::DuplicateConfirmationsTolerated)
            }
            CheckId::DisabledModeReadyWithoutDeclare => {
                Some(Scenario::DisabledModeReadyWithoutDeclare)
            }
        }
    }

    pub fn attach_prerequisite(self) -> Option<&'static str> {
        match self {
            Scenario::RepublishesSameCorrelationId => Some(
                "s3 holds only for a service that is still unconfirmed and re-publishes its declare \
                 within --timeout: raise --timeout above the service's own wait timeout",
            ),
            Scenario::RejectionStopsReadiness => Some(
                "s4 holds only with a rejecting acceptor (--reject) for a service that is still \
                 awaiting acceptance",
            ),
            Scenario::DisabledModeReadyWithoutDeclare => {
                Some("s6 holds only for a service that was booted with scope declaration disabled")
            }
            _ => None,
        }
    }

    pub fn requires_subject_lifecycle(self) -> bool {
        !matches!(
            self,
            Scenario::DeclareWellFormed | Scenario::DeclarationContent | Scenario::ReadinessGated
        )
    }
}

pub fn attach_default() -> Vec<Scenario> {
    vec![
        Scenario::DeclareWellFormed,
        Scenario::ReadinessGated,
        Scenario::DeclarationContent,
    ]
}

pub fn spawn_default() -> Vec<Scenario> {
    vec![
        Scenario::DeclareWellFormed,
        Scenario::ReadinessGated,
        Scenario::RepublishesSameCorrelationId,
        Scenario::RejectionStopsReadiness,
        Scenario::DuplicateConfirmationsTolerated,
        Scenario::DisabledModeReadyWithoutDeclare,
        Scenario::DeclarationContent,
    ]
}

pub fn parse_scenarios(raw: &str) -> Result<Vec<Scenario>> {
    let mut scenarios = Vec::new();
    for part in raw.split(',') {
        let code = part.trim();
        if code.is_empty() {
            continue;
        }
        let scenario = Scenario::from_code(code)
            .ok_or_else(|| ConformanceError::InvalidInput(format!("unknown scenario {code:?}")))?;
        if !scenarios.contains(&scenario) {
            scenarios.push(scenario);
        }
    }
    require_selection(&scenarios)?;
    Ok(scenarios)
}

pub(crate) fn require_selection(scenarios: &[Scenario]) -> Result<()> {
    if scenarios.is_empty() {
        return Err(ConformanceError::InvalidInput(
            "no scenarios selected".to_string(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub enum AcceptorBehavior {
    Accept,
    Reject(ScopeDeclarationError),
}

impl AcceptorBehavior {
    pub fn spawn_rejection(&self, sample_scope_key: &str) -> Self {
        match self {
            AcceptorBehavior::Reject(reason) => AcceptorBehavior::Reject(reason.clone()),
            AcceptorBehavior::Accept => {
                AcceptorBehavior::Reject(ScopeDeclarationError::ScopeOwnedByAnotherService {
                    key: sample_scope_key.to_string(),
                    owner: "another-service".to_string(),
                })
            }
        }
    }

    pub fn spawn_rejection_for(&self, expected: &ExpectedDeclaration) -> Self {
        self.spawn_rejection(expected.sample_scope_key())
    }

    pub fn reject_for(expected: &ExpectedDeclaration, reason_code: Option<&str>) -> Result<Self> {
        Self::reject(reason_code, expected.sample_scope_key())
    }

    pub fn reject(reason_code: Option<&str>, sample_scope_key: &str) -> Result<Self> {
        let reason = match reason_code {
            None | Some("scope_owned_by_another_service") => {
                ScopeDeclarationError::ScopeOwnedByAnotherService {
                    key: sample_scope_key.to_string(),
                    owner: "another-service".to_string(),
                }
            }
            Some("duplicate_scope_in_declaration") => {
                ScopeDeclarationError::DuplicateScopeInDeclaration {
                    key: sample_scope_key.to_string(),
                }
            }
            Some("scope_prefix_mismatch") => ScopeDeclarationError::ScopePrefixMismatch {
                scope_service: "another-service".to_string(),
                declaring_service: "this-service".to_string(),
            },
            Some(other) => {
                return Err(ConformanceError::InvalidInput(format!(
                    "unknown reject reason code {other:?}; use one of \
                     scope_owned_by_another_service, duplicate_scope_in_declaration, \
                     scope_prefix_mismatch"
                )));
            }
        };
        Ok(AcceptorBehavior::Reject(reason))
    }
}

#[cfg(test)]
mod tests {
    use crate::expected::{ExpectedScope, SAMPLE_FALLBACK_SCOPE};

    use super::*;

    fn expecting(keys: &[&str]) -> ExpectedDeclaration {
        ExpectedDeclaration::new(
            "notifier",
            keys.iter()
                .map(|key| ExpectedScope {
                    key: (*key).to_string(),
                    platform_only: false,
                })
                .collect(),
        )
    }

    fn rejected_key(behavior: &AcceptorBehavior) -> String {
        match behavior {
            AcceptorBehavior::Reject(ScopeDeclarationError::ScopeOwnedByAnotherService {
                key,
                ..
            }) => key.clone(),
            other => panic!("expected a scope_owned_by_another_service rejection: {other:?}"),
        }
    }

    #[test]
    fn given_an_expectation_with_scopes_when_the_rejection_is_built_then_it_names_the_first_expected_scope()
     {
        let expected = expecting(&["notifier:read", "notifier:admin"]);

        let reject = AcceptorBehavior::reject_for(&expected, None).expect("rejection");
        let spawned = AcceptorBehavior::Accept.spawn_rejection_for(&expected);

        assert_eq!(rejected_key(&reject), "notifier:read");
        assert_eq!(rejected_key(&spawned), "notifier:read");
    }

    #[test]
    fn given_an_expectation_without_scopes_when_the_rejection_is_built_then_it_names_the_fallback_scope()
     {
        let expected = expecting(&[]);

        let reject = AcceptorBehavior::reject_for(&expected, None).expect("rejection");
        let spawned = AcceptorBehavior::Accept.spawn_rejection_for(&expected);

        assert_eq!(rejected_key(&reject), SAMPLE_FALLBACK_SCOPE);
        assert_eq!(rejected_key(&spawned), SAMPLE_FALLBACK_SCOPE);
    }

    #[test]
    fn given_an_unknown_reason_code_when_the_rejection_is_built_then_it_is_refused() {
        let expected = expecting(&["notifier:read"]);

        assert!(AcceptorBehavior::reject_for(&expected, Some("nope")).is_err());
    }

    #[test]
    fn given_a_rejecting_global_behavior_when_the_spawn_rejection_is_built_then_it_keeps_the_reason()
     {
        let expected = expecting(&["notifier:read"]);
        let global = AcceptorBehavior::reject_for(&expected, Some("scope_prefix_mismatch"))
            .expect("rejection");

        let spawned = global.spawn_rejection_for(&expected);

        assert!(matches!(
            spawned,
            AcceptorBehavior::Reject(ScopeDeclarationError::ScopePrefixMismatch { .. })
        ));
    }
}
