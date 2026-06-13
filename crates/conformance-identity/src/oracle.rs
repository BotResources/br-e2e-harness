use br_core_scope::DeclareServiceScopes;
use br_identity_domain::{DeclarationOutcome, ScopeRegistry, judge_declaration};

use crate::capture::Verdict;

pub fn expected_verdict(sequence: &[DeclareServiceScopes]) -> Verdict {
    expected_step_verdicts(sequence)
        .pop()
        .expect("a scenario must declare at least once")
}

pub fn expected_step_verdicts(sequence: &[DeclareServiceScopes]) -> Vec<Verdict> {
    let mut registry = ScopeRegistry::new();
    sequence
        .iter()
        .map(|command| outcome_to_verdict(judge_declaration(&mut registry, command.clone())))
        .collect()
}

pub fn outcome_to_verdict(outcome: DeclarationOutcome) -> Verdict {
    match outcome {
        DeclarationOutcome::Accepted { service, .. } => Verdict::Accepted {
            service: service.as_str().to_string(),
        },
        DeclarationOutcome::Rejected { reason } => Verdict::Rejected { reason },
        _ => unreachable!("DeclarationOutcome is non_exhaustive but fully handled above"),
    }
}

pub fn verdict_code(verdict: &Verdict) -> String {
    match verdict {
        Verdict::Accepted { service } => format!("accepted(service={service})"),
        Verdict::Rejected { reason } => format!("rejected({reason})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{declare, from_raw, raw_manifest, raw_spec};
    use br_core_scope::{KeyValidationError, RawScopeDeclaration, ScopeDeclarationError};

    #[test]
    fn clean_declaration_is_accepted() {
        let verdict = expected_verdict(&[declare("notifier", &["notifier:read"])]);
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
        let verdict = expected_verdict(&[decl.clone(), decl]);
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
        let verdict = expected_verdict(&[seed, claim]);
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
        let verdict = expected_verdict(&[dup]);
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
        let verdict = expected_verdict(&[bad]);
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
