use br_test_harness::wait_until;
use uuid::Uuid;

use crate::capture::Verdict;
use crate::error::ConformanceError;
use crate::oracle::{expected_step_verdicts, verdict_code};
use crate::outcome::CheckOutcome;
use crate::scenario::Scenario;
use crate::wire::declaration_label;

use super::CheckContext;

pub async fn run_judged_scenario(scenario: Scenario, ctx: &CheckContext<'_>) -> CheckOutcome {
    let id = scenario.check_id();
    let sequence = scenario.sequence(ctx.namespace);
    let expected = expected_step_verdicts(&sequence);
    let final_expected = expected.last().expect("scenario declares at least once");
    let expected_label = verdict_code(final_expected);

    for (step, (command, want)) in sequence.iter().zip(expected.iter()).enumerate() {
        let correlation_id = match ctx
            .declarer
            .declare_with_correlation(command.clone(), Uuid::now_v7())
            .await
        {
            Ok(id) => id,
            Err(e) => {
                return CheckOutcome::fail(
                    id,
                    &expected_label,
                    "the declare command could not be published",
                    format!("step {step} ({}): {e}", declaration_label(command)),
                );
            }
        };

        let arrived = wait_until(ctx.timeout, || async {
            ctx.capture.confirmation_for(correlation_id).is_some()
        })
        .await;
        if !arrived {
            return CheckOutcome::fail(
                id,
                &expected_label,
                "no confirmation arrived within the timeout",
                format!(
                    "step {step} ({}) got no accepted/rejected echoing its correlation_id",
                    declaration_label(command)
                ),
            );
        }

        let observed = match ctx.capture.verdict_for(correlation_id) {
            Some(Ok(verdict)) => verdict,
            Some(Err(e)) => {
                return decode_failure(scenario, &expected_label, step, command, e);
            }
            None => unreachable!("confirmation was just observed present"),
        };

        if &observed != want {
            return CheckOutcome::fail(
                id,
                &expected_label,
                verdict_code(&observed),
                format!(
                    "step {step} ({}) verdict diverged from the lib oracle:\n  oracle:  {}\n  subject: {}",
                    declaration_label(command),
                    verdict_code(want),
                    verdict_code(&observed),
                ),
            );
        }
    }

    let observed_label = match expected.last() {
        Some(Verdict::Accepted { service }) => format!("accepted(service={service})"),
        Some(verdict) => verdict_code(verdict),
        None => unreachable!("scenario declares at least once"),
    };
    CheckOutcome::pass(id, expected_label, observed_label)
}

fn decode_failure(
    scenario: Scenario,
    expected_label: &str,
    step: usize,
    command: &br_core_scope::DeclareServiceScopes,
    error: ConformanceError,
) -> CheckOutcome {
    CheckOutcome::fail(
        scenario.check_id(),
        expected_label,
        "the confirmation did not deserialize into the real IntegrationEvent type",
        format!("step {step} ({}): {error}", declaration_label(command)),
    )
}
