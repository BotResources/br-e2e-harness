use br_core_scope::DeclareServiceScopes;
use br_test_harness::wait_until;
use uuid::Uuid;

use crate::capture::DecodedConfirmation;
use crate::error::ConformanceError;
use crate::oracle::{confirmation_code, expected_step_confirmations};
use crate::outcome::CheckOutcome;
use crate::scenario::Scenario;
use crate::wire::declaration_label;

use super::CheckContext;

pub async fn run_judged_scenario(scenario: Scenario, ctx: &CheckContext<'_>) -> CheckOutcome {
    let id = scenario.check_id();
    let sequence = scenario.sequence(ctx.namespace);
    let expected = match expected_step_confirmations(&sequence) {
        Ok(expected) => expected,
        Err(e) => {
            return CheckOutcome::fail(
                id,
                "a lib-computed oracle verdict",
                "the oracle could not map the lib's DeclarationOutcome",
                format!("{e}"),
            );
        }
    };

    let mut last_observed = None;
    for (step, (command, want)) in sequence.iter().zip(expected.iter()).enumerate() {
        let want_label = confirmation_code(want);
        let correlation_id = match ctx
            .declarer
            .declare_with_correlation(command.clone(), Uuid::now_v7())
            .await
        {
            Ok(id) => id,
            Err(e) => {
                return CheckOutcome::fail(
                    id,
                    want_label,
                    "the declare command could not be published",
                    format!("step {step} ({}): {e}", declaration_label(command)),
                );
            }
        };

        let mut decoded = None;
        wait_until(ctx.timeout, || {
            decoded = ctx.capture.confirmation_decoded_for(correlation_id);
            let arrived = decoded.is_some();
            async move { arrived }
        })
        .await;
        let observed = match decoded {
            Some(Ok(observed)) => observed,
            Some(Err(e)) => return decode_failure(scenario, &want_label, step, command, e),
            None => {
                return CheckOutcome::fail(
                    id,
                    want_label,
                    "no confirmation arrived within the timeout",
                    format!(
                        "step {step} ({}) got no accepted/rejected echoing its correlation_id",
                        declaration_label(command)
                    ),
                );
            }
        };

        if let Some(divergence) = divergence(want, &observed) {
            return CheckOutcome::fail(
                id,
                want_label,
                confirmation_code(&observed),
                format!(
                    "step {step} ({}) {divergence}:\n  oracle:  {}\n  subject: {}",
                    declaration_label(command),
                    confirmation_code(want),
                    confirmation_code(&observed),
                ),
            );
        }
        last_observed = Some((want_label, observed));
    }

    match last_observed {
        Some((want_label, observed)) => {
            CheckOutcome::pass(id, want_label, confirmation_code(&observed))
        }
        None => CheckOutcome::fail(
            id,
            "at least one declaration",
            "the scenario declared nothing",
            "an empty scenario cannot pass",
        ),
    }
}

fn divergence(want: &DecodedConfirmation, observed: &DecodedConfirmation) -> Option<&'static str> {
    if observed.verdict != want.verdict {
        Some("verdict diverged from the lib oracle")
    } else if observed.service != want.service {
        Some("names another service than the lib oracle")
    } else {
        None
    }
}

fn decode_failure(
    scenario: Scenario,
    expected_label: &str,
    step: usize,
    command: &DeclareServiceScopes,
    error: ConformanceError,
) -> CheckOutcome {
    let observed = match error {
        ConformanceError::NonConformantConfirmation(_) => {
            "the confirmation did not deserialize into the real IntegrationEvent type"
        }
        _ => "the confirmation capture failed before a confirmation could be read",
    };
    CheckOutcome::fail(
        scenario.check_id(),
        expected_label,
        observed,
        format!("step {step} ({}): {error}", declaration_label(command)),
    )
}

#[cfg(test)]
mod tests {
    use br_core_scope::ScopeDeclarationError;

    use super::*;
    use crate::capture::Verdict;

    fn declaration() -> DeclareServiceScopes {
        crate::wire::declare("notifier", &["notifier:read"])
    }

    fn rejected(service: &str) -> DecodedConfirmation {
        DecodedConfirmation {
            service: service.to_string(),
            verdict: Verdict::Rejected {
                reason: ScopeDeclarationError::DuplicateScopeInDeclaration {
                    key: "notifier:read".to_string(),
                },
            },
        }
    }

    #[test]
    fn the_same_rejection_naming_another_service_diverges() {
        assert_eq!(
            divergence(&rejected("notifier"), &rejected("notifier")),
            None
        );
        assert_eq!(
            divergence(&rejected("notifier"), &rejected("billing")),
            Some("names another service than the lib oracle")
        );
    }

    #[test]
    fn a_different_verdict_diverges_before_the_service_is_compared() {
        let accepted = DecodedConfirmation {
            service: "notifier".to_string(),
            verdict: Verdict::Accepted {
                service: "notifier".to_string(),
            },
        };
        assert_eq!(
            divergence(&rejected("notifier"), &accepted),
            Some("verdict diverged from the lib oracle")
        );
    }

    #[test]
    fn the_observed_text_follows_the_error_kind() {
        let command = declaration();
        let undecodable = decode_failure(
            Scenario::CleanDeclarationAccepted,
            "accepted",
            0,
            &command,
            ConformanceError::NonConformantConfirmation("accepted: bad".to_string()),
        );
        assert_eq!(
            undecodable.observed,
            "the confirmation did not deserialize into the real IntegrationEvent type"
        );
        let unreadable = decode_failure(
            Scenario::CleanDeclarationAccepted,
            "accepted",
            0,
            &command,
            ConformanceError::Jetstream("reading the confirmation capture: gone".to_string()),
        );
        assert_eq!(
            unreadable.observed,
            "the confirmation capture failed before a confirmation could be read"
        );
        assert!(
            unreadable
                .detail
                .as_deref()
                .is_some_and(|d| d.contains("reading the confirmation capture: gone"))
        );
    }
}
