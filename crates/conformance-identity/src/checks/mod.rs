mod judged;

use std::time::Duration;

use crate::capture::ConfirmationCapture;
use crate::declarer::Declarer;
use crate::outcome::CheckOutcome;
use crate::prerequisite::contained;
use crate::scenario::Scenario;

use judged::run_judged_scenario;

pub struct CheckContext<'a> {
    pub declarer: &'a Declarer,
    pub capture: &'a ConfirmationCapture,
    pub namespace: &'a str,
    pub timeout: Duration,
}

pub async fn run_scenario(scenario: Scenario, ctx: &CheckContext<'_>) -> CheckOutcome {
    match contained(run_judged_scenario(scenario, ctx)).await {
        Ok(outcome) => outcome,
        Err(failure) => CheckOutcome::fail(
            scenario.check_id(),
            "a verdict observed from the subject",
            "the harness failed while observing the subject",
            failure,
        ),
    }
}
