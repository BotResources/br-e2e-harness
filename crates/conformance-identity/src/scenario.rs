use br_core_scope::{DeclareServiceScopes, RawScopeDeclaration};

use crate::error::{ConformanceError, Result};
use crate::outcome::CheckId;
use crate::wire::{declare, from_raw, raw_manifest, raw_spec};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scenario {
    CleanDeclarationAccepted,
    CrossServiceClaimRejected,
    IntraDeclarationDuplicateRejected,
    PrefixMismatchRejected,
    InvalidScopeKeyRejected,
    IdempotentRedeclareAccepted,
}

pub const ALL: [Scenario; 6] = [
    Scenario::CleanDeclarationAccepted,
    Scenario::CrossServiceClaimRejected,
    Scenario::IntraDeclarationDuplicateRejected,
    Scenario::PrefixMismatchRejected,
    Scenario::InvalidScopeKeyRejected,
    Scenario::IdempotentRedeclareAccepted,
];

impl Scenario {
    pub fn check_id(self) -> CheckId {
        match self {
            Scenario::CleanDeclarationAccepted => CheckId::CleanDeclarationAccepted,
            Scenario::CrossServiceClaimRejected => CheckId::CrossServiceClaimRejected,
            Scenario::IntraDeclarationDuplicateRejected => {
                CheckId::IntraDeclarationDuplicateRejected
            }
            Scenario::PrefixMismatchRejected => CheckId::PrefixMismatchRejected,
            Scenario::InvalidScopeKeyRejected => CheckId::InvalidScopeKeyRejected,
            Scenario::IdempotentRedeclareAccepted => CheckId::IdempotentRedeclareAccepted,
        }
    }

    pub fn code(self) -> &'static str {
        self.check_id().code()
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match CheckId::from_code(code)? {
            CheckId::CleanDeclarationAccepted => Some(Scenario::CleanDeclarationAccepted),
            CheckId::CrossServiceClaimRejected => Some(Scenario::CrossServiceClaimRejected),
            CheckId::IntraDeclarationDuplicateRejected => {
                Some(Scenario::IntraDeclarationDuplicateRejected)
            }
            CheckId::PrefixMismatchRejected => Some(Scenario::PrefixMismatchRejected),
            CheckId::InvalidScopeKeyRejected => Some(Scenario::InvalidScopeKeyRejected),
            CheckId::IdempotentRedeclareAccepted => Some(Scenario::IdempotentRedeclareAccepted),
        }
    }

    pub fn sequence(self, namespace: &str) -> Vec<DeclareServiceScopes> {
        let primary = service_key("notifier", namespace);
        match self {
            Scenario::CleanDeclarationAccepted => vec![declare(
                &primary,
                &[&scope_key(&primary, "read"), &scope_key(&primary, "admin")],
            )],
            Scenario::CrossServiceClaimRejected => {
                let owner = service_key("notifier", namespace);
                let claimer = service_key("billing", namespace);
                let contested = scope_key(&owner, "read");
                vec![
                    declare(&owner, &[&contested]),
                    from_raw(RawScopeDeclaration {
                        manifest: raw_manifest(&claimer),
                        scopes: vec![raw_spec(&contested, false)],
                    }),
                ]
            }
            Scenario::IntraDeclarationDuplicateRejected => {
                let dup = scope_key(&primary, "read");
                vec![from_raw(RawScopeDeclaration {
                    manifest: raw_manifest(&primary),
                    scopes: vec![raw_spec(&dup, false), raw_spec(&dup, false)],
                })]
            }
            Scenario::PrefixMismatchRejected => {
                let foreign = scope_key(&service_key("billing", namespace), "read");
                vec![from_raw(RawScopeDeclaration {
                    manifest: raw_manifest(&primary),
                    scopes: vec![raw_spec(&foreign, false)],
                })]
            }
            Scenario::InvalidScopeKeyRejected => vec![from_raw(RawScopeDeclaration {
                manifest: raw_manifest(&primary),
                scopes: vec![raw_spec(&format!("{primary}:BAD"), false)],
            })],
            Scenario::IdempotentRedeclareAccepted => {
                let decl = declare(&primary, &[&scope_key(&primary, "read")]);
                vec![decl.clone(), decl]
            }
        }
    }
}

fn service_key(base: &str, namespace: &str) -> String {
    if namespace.is_empty() {
        base.to_string()
    } else {
        format!("{base}_{namespace}")
    }
}

fn scope_key(service: &str, capability: &str) -> String {
    format!("{service}:{capability}")
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

pub fn spawn_default() -> Vec<Scenario> {
    ALL.to_vec()
}

pub fn attach_default() -> Vec<Scenario> {
    ALL.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::Verdict;
    use crate::oracle::expected_verdict;
    use br_core_scope::{KeyValidationError, ScopeDeclarationError};

    #[test]
    fn namespacing_makes_service_keys_unique() {
        let a = Scenario::CleanDeclarationAccepted.sequence("run1");
        let b = Scenario::CleanDeclarationAccepted.sequence("run2");
        assert_ne!(a[0].raw().manifest.key, b[0].raw().manifest.key);
        assert!(a[0].raw().manifest.key.ends_with("_run1"));
    }

    #[test]
    fn every_scenario_yields_the_expected_oracle_verdict() {
        let ns = "t";
        for scenario in ALL {
            let verdict = expected_verdict(&scenario.sequence(ns));
            match scenario {
                Scenario::CleanDeclarationAccepted | Scenario::IdempotentRedeclareAccepted => {
                    assert!(matches!(verdict, Verdict::Accepted { .. }), "{scenario:?}");
                }
                Scenario::CrossServiceClaimRejected | Scenario::PrefixMismatchRejected => {
                    assert!(
                        matches!(
                            verdict,
                            Verdict::Rejected {
                                reason: ScopeDeclarationError::ScopePrefixMismatch { .. }
                            }
                        ),
                        "{scenario:?} -> {verdict:?}"
                    );
                }
                Scenario::IntraDeclarationDuplicateRejected => {
                    assert!(matches!(
                        verdict,
                        Verdict::Rejected {
                            reason: ScopeDeclarationError::DuplicateScopeInDeclaration { .. }
                        }
                    ));
                }
                Scenario::InvalidScopeKeyRejected => {
                    assert!(matches!(
                        verdict,
                        Verdict::Rejected {
                            reason: ScopeDeclarationError::InvalidScopeKey {
                                validation: KeyValidationError::InvalidCharset,
                                ..
                            }
                        }
                    ));
                }
            }
        }
    }
}
