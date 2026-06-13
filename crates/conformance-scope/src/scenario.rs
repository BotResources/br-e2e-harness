use br_core_scope::ScopeDeclarationError;

use crate::error::{ConformanceError, Result};
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
    if scenarios.is_empty() {
        return Err(ConformanceError::InvalidInput(
            "no scenarios selected".to_string(),
        ));
    }
    Ok(scenarios)
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
