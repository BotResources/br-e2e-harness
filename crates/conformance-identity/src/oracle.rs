use br_core_scope::DeclareServiceScopes;
use br_identity_domain::{DeclarationOutcome, RejectedIdentity, ScopeRegistry, judge_declaration};

use crate::capture::{DecodedConfirmation, Verdict};
use crate::error::{ConformanceError, Result};

pub fn expected_verdict(sequence: &[DeclareServiceScopes]) -> Result<Verdict> {
    expected_step_verdicts(sequence)?
        .pop()
        .ok_or_else(empty_sequence)
}

pub fn expected_step_verdicts(sequence: &[DeclareServiceScopes]) -> Result<Vec<Verdict>> {
    Ok(expected_step_confirmations(sequence)?
        .into_iter()
        .map(|confirmation| confirmation.verdict)
        .collect())
}

pub fn expected_step_confirmations(
    sequence: &[DeclareServiceScopes],
) -> Result<Vec<DecodedConfirmation>> {
    if sequence.is_empty() {
        return Err(empty_sequence());
    }
    let mut registry = ScopeRegistry::new();
    sequence
        .iter()
        .map(|command| outcome_to_confirmation(judge_declaration(&mut registry, command.clone())))
        .collect()
}

fn empty_sequence() -> ConformanceError {
    ConformanceError::InvalidInput("a scenario must declare at least once".to_string())
}

pub fn outcome_to_confirmation(outcome: DeclarationOutcome) -> Result<DecodedConfirmation> {
    match outcome {
        DeclarationOutcome::Accepted { service, .. } => Ok(DecodedConfirmation {
            service: service.as_str().to_string(),
            verdict: Verdict::Accepted {
                service: service.as_str().to_string(),
            },
        }),
        DeclarationOutcome::Rejected { identity, reason } => Ok(DecodedConfirmation {
            service: match identity {
                RejectedIdentity::Service(service) => service.as_str().to_string(),
                RejectedIdentity::Unrepresentable { raw } => raw,
            },
            verdict: Verdict::Rejected { reason },
        }),
        _ => Err(ConformanceError::OracleOutcomeUnknown),
    }
}

pub fn outcome_to_verdict(outcome: DeclarationOutcome) -> Result<Verdict> {
    Ok(outcome_to_confirmation(outcome)?.verdict)
}

pub fn verdict_code(verdict: &Verdict) -> String {
    match verdict {
        Verdict::Accepted { service } => format!("accepted(service={service})"),
        Verdict::Rejected { reason } => format!("rejected({reason})"),
    }
}

pub fn confirmation_code(confirmation: &DecodedConfirmation) -> String {
    match confirmation.verdict {
        Verdict::Accepted { .. } => verdict_code(&confirmation.verdict),
        Verdict::Rejected { .. } => format!(
            "{} for service={}",
            verdict_code(&confirmation.verdict),
            confirmation.service
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{declare, from_raw, raw_manifest, raw_spec};
    use br_core_scope::{KeyValidationError, RawScopeDeclaration, ScopeDeclarationError};

    #[test]
    fn an_empty_sequence_is_a_typed_error_not_a_panic() {
        assert!(matches!(
            expected_verdict(&[]),
            Err(ConformanceError::InvalidInput(_))
        ));
        assert!(matches!(
            expected_step_confirmations(&[]),
            Err(ConformanceError::InvalidInput(_))
        ));
    }

    #[test]
    fn a_rejection_names_the_declaring_service() {
        let claim = from_raw(RawScopeDeclaration {
            manifest: raw_manifest("billing"),
            scopes: vec![raw_spec("notifier:read", false)],
        });
        let confirmations = expected_step_confirmations(&[claim]).unwrap();
        assert_eq!(confirmations[0].service, "billing");
        assert!(matches!(confirmations[0].verdict, Verdict::Rejected { .. }));
    }

    #[test]
    fn the_confirmation_code_distinguishes_the_service_of_a_rejection() {
        let verdict = Verdict::Rejected {
            reason: ScopeDeclarationError::DuplicateScopeInDeclaration {
                key: "notifier:read".to_string(),
            },
        };
        let own = DecodedConfirmation {
            service: "notifier".to_string(),
            verdict: verdict.clone(),
        };
        let other = DecodedConfirmation {
            service: "billing".to_string(),
            verdict,
        };
        assert_ne!(confirmation_code(&own), confirmation_code(&other));
        assert_ne!(own, other);
    }

    #[test]
    fn clean_declaration_is_accepted() {
        let verdict = expected_verdict(&[declare("notifier", &["notifier:read"])]).unwrap();
        assert_eq!(
            verdict,
            Verdict::Accepted {
                service: "notifier".to_string()
            }
        );
    }

    #[test]
    fn idempotent_redeclaration_is_accepted() {
        let decl = declare("notifier", &["notifier:read"]);
        let verdict = expected_verdict(&[decl.clone(), decl]).unwrap();
        assert_eq!(
            verdict,
            Verdict::Accepted {
                service: "notifier".to_string()
            }
        );
    }

    #[test]
    fn cross_service_claim_is_rejected_as_prefix_mismatch_not_ownership() {
        let seed = declare("notifier", &["notifier:read"]);
        let claim = from_raw(RawScopeDeclaration {
            manifest: raw_manifest("billing"),
            scopes: vec![raw_spec("notifier:read", false)],
        });
        let verdict = expected_verdict(&[seed, claim]).unwrap();
        assert_eq!(
            verdict,
            Verdict::Rejected {
                reason: ScopeDeclarationError::ScopePrefixMismatch {
                    scope_service: "notifier".to_string(),
                    declaring_service: "billing".to_string(),
                }
            }
        );
    }

    #[test]
    fn intra_declaration_duplicate_is_rejected() {
        let dup = from_raw(RawScopeDeclaration {
            manifest: raw_manifest("notifier"),
            scopes: vec![
                raw_spec("notifier:read", false),
                raw_spec("notifier:read", false),
            ],
        });
        let verdict = expected_verdict(&[dup]).unwrap();
        assert_eq!(
            verdict,
            Verdict::Rejected {
                reason: ScopeDeclarationError::DuplicateScopeInDeclaration {
                    key: "notifier:read".to_string(),
                }
            }
        );
    }

    #[test]
    fn invalid_scope_key_is_rejected() {
        let bad = from_raw(RawScopeDeclaration {
            manifest: raw_manifest("notifier"),
            scopes: vec![raw_spec("notifier:BAD", false)],
        });
        let verdict = expected_verdict(&[bad]).unwrap();
        assert_eq!(
            verdict,
            Verdict::Rejected {
                reason: ScopeDeclarationError::InvalidScopeKey {
                    key: "notifier:BAD".to_string(),
                    validation: KeyValidationError::InvalidCharset,
                }
            }
        );
    }
}
