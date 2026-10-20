mod confirmation;
mod observe;

use std::time::Duration;

use br_core_scope::ServiceKey;
use br_test_harness::wait_until;
use br_util_nats_fabric::Fabric;
use reqwest::StatusCode;

use crate::capture::{CapturedDeclare, DeclareCapture};
use crate::error::ConformanceError;
use crate::expected::ExpectedDeclaration;
use crate::outcome::{CheckId, CheckOutcome};
use crate::readyz::ReadyzProbe;
use crate::scenario::{AcceptorBehavior, Scenario};
use crate::timeout::MAX_TIMEOUT;

pub use confirmation::{
    duplicate_confirmations_tolerated, readiness_gated, rejection_stops_readiness,
    republishes_same_correlation_id,
};
pub use observe::{declaration_content, declare_well_formed, disabled_mode_ready_without_declare};

pub struct CheckContext<'a> {
    pub fabric: &'a Fabric,
    pub readyz: &'a ReadyzProbe,
    pub capture: &'a DeclareCapture,
    pub expected: &'a ExpectedDeclaration,
    pub service_key: &'a ServiceKey,
    pub behavior: &'a AcceptorBehavior,
    pub timeout: Duration,
}

impl CheckContext<'_> {
    pub(crate) fn wait(&self) -> Duration {
        self.timeout.min(MAX_TIMEOUT)
    }
}

pub const QUIET_WINDOW: Duration = Duration::from_secs(3);

pub async fn run_scenario(scenario: Scenario, ctx: &CheckContext<'_>) -> CheckOutcome {
    match scenario {
        Scenario::DeclareWellFormed => declare_well_formed(ctx).await,
        Scenario::DeclarationContent => declaration_content(ctx).await,
        Scenario::ReadinessGated => readiness_gated(ctx).await,
        Scenario::RepublishesSameCorrelationId => republishes_same_correlation_id(ctx).await,
        Scenario::RejectionStopsReadiness => rejection_stops_readiness(ctx).await,
        Scenario::DuplicateConfirmationsTolerated => duplicate_confirmations_tolerated(ctx).await,
        Scenario::DisabledModeReadyWithoutDeclare => disabled_mode_ready_without_declare(ctx).await,
    }
}

pub(crate) enum DeclareWait {
    Timeout,
    Capture(ConformanceError),
}

pub(crate) async fn await_declares(
    ctx: &CheckContext<'_>,
    at_least: usize,
) -> Result<(), DeclareWait> {
    wait_until(ctx.wait(), || async {
        ctx.capture
            .try_count()
            .map(|count| count >= at_least)
            .unwrap_or(true)
    })
    .await;
    match ctx.capture.try_count() {
        Ok(count) if count >= at_least => Ok(()),
        Ok(_) => Err(DeclareWait::Timeout),
        Err(e) => Err(DeclareWait::Capture(e)),
    }
}

pub(crate) async fn await_first_declare(
    ctx: &CheckContext<'_>,
) -> Result<CapturedDeclare, DeclareWait> {
    await_declares(ctx, 1).await?;
    match ctx.capture.try_first() {
        Ok(Some(declare)) => Ok(declare),
        Ok(None) => Err(DeclareWait::Timeout),
        Err(e) => Err(DeclareWait::Capture(e)),
    }
}

pub(crate) fn capture_failed(
    id: CheckId,
    expected: impl Into<String>,
    cause: &ConformanceError,
) -> CheckOutcome {
    CheckOutcome::fail(
        id,
        expected,
        "the declare capture failed before the declare could be read",
        cause.to_string(),
    )
}

pub(crate) fn no_declare(
    id: CheckId,
    expected: &str,
    wait: DeclareWait,
    observed: &str,
    detail: &str,
) -> CheckOutcome {
    match wait {
        DeclareWait::Timeout => CheckOutcome::fail(id, expected, observed, detail),
        DeclareWait::Capture(cause) => capture_failed(id, expected, &cause),
    }
}

pub(crate) fn status_text(status: Option<StatusCode>) -> String {
    match status {
        Some(status) => status.as_u16().to_string(),
        None => "unreachable".to_string(),
    }
}

pub(crate) fn seen_readyz(status: Option<StatusCode>, when: &str) -> String {
    match status {
        Some(status) => format!("/readyz answered {} {when}", status.as_u16()),
        None => format!("/readyz did not answer {when}"),
    }
}

pub(crate) async fn readyz_seen(ctx: &CheckContext<'_>, when: &str) -> String {
    seen_readyz(ctx.readyz.status().await, when)
}

pub(crate) fn wire_excerpt(raw: &[u8]) -> String {
    const MAX: usize = 400;
    let text = String::from_utf8_lossy(raw);
    if text.len() > MAX {
        format!("{}…", text.chars().take(MAX).collect::<String>())
    } else {
        text.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_observed_readyz_states_what_was_seen() {
        assert_eq!(
            seen_readyz(Some(StatusCode::SERVICE_UNAVAILABLE), "after acceptance"),
            "/readyz answered 503 after acceptance"
        );
        assert_eq!(
            seen_readyz(None, "after acceptance"),
            "/readyz did not answer after acceptance"
        );
    }
}
