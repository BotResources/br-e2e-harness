mod confirmation;
mod observe;

use std::time::Duration;

use async_nats::jetstream;
use br_core_scope::ServiceKey;
use br_test_harness::wait_until;
use uuid::Uuid;

use crate::capture::DeclareCapture;
use crate::expected::ExpectedDeclaration;
use crate::outcome::CheckOutcome;
use crate::readyz::ReadyzProbe;
use crate::scenario::{AcceptorBehavior, Scenario};

pub use confirmation::{
    duplicate_confirmations_tolerated, readiness_gated, rejection_stops_readiness,
    republishes_same_correlation_id,
};
pub use observe::{declaration_content, declare_well_formed, disabled_mode_ready_without_declare};

pub struct CheckContext<'a> {
    pub js: &'a jetstream::Context,
    pub readyz: &'a ReadyzProbe,
    pub capture: &'a DeclareCapture,
    pub expected: &'a ExpectedDeclaration,
    pub service_key: &'a ServiceKey,
    pub behavior: &'a AcceptorBehavior,
    pub timeout: Duration,
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

pub(crate) async fn await_first_correlation(ctx: &CheckContext<'_>) -> Option<Uuid> {
    if !wait_until(ctx.timeout, || async { ctx.capture.count() >= 1 }).await {
        return None;
    }
    ctx.capture.first().map(|d| d.correlation_id)
}

pub(crate) async fn readyz_status(ctx: &CheckContext<'_>) -> String {
    match ctx.readyz.status().await {
        Some(status) => status.as_u16().to_string(),
        None => "unreachable".to_string(),
    }
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
