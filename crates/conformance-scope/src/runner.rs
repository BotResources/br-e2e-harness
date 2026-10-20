use std::time::Duration;

use br_test_harness::FabricTestNats;

use crate::capture::DeclareCapture;
use crate::checks::{CheckContext, run_scenario};
use crate::error::{ConformanceError, Result};
use crate::expected::ExpectedDeclaration;
use crate::outcome::{CheckOutcome, CheckStatus, ConformanceReport};
use crate::preflight::check_attach_input;
use crate::prerequisite::contained;
use crate::readyz::ReadyzProbe;
use crate::scenario::{AcceptorBehavior, Scenario};

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

pub struct AttachTarget {
    pub nats_url: String,
    pub readyz_url: String,
}

pub async fn run_attach(
    target: &AttachTarget,
    expected: &ExpectedDeclaration,
    behavior: &AcceptorBehavior,
    scenarios: &[Scenario],
    timeout: Duration,
) -> Result<ConformanceReport> {
    check_attach_input(target, expected, scenarios, timeout)?;
    let service_key = expected.validated_service_key()?;
    let readyz = ReadyzProbe::new(&target.readyz_url)?;
    let nats = contained(FabricTestNats::attach_without_provisioning(
        &target.nats_url,
    ))
    .await
    .map_err(|e| {
        ConformanceError::Jetstream(format!(
            "connecting the harness to NATS at {}: {e}",
            target.nats_url
        ))
    })?;
    match contained(nats.fixed_streams_present()).await {
        Ok(true) => {}
        Ok(false) => {
            nats.shutdown().await;
            return Err(missing_streams());
        }
        Err(panic) => {
            nats.shutdown().await;
            return Err(ConformanceError::Jetstream(format!(
                "checking the fixed streams INTEGRATION_CMD and INTEGRATION_EVT on {}: {panic}",
                target.nats_url
            )));
        }
    }
    let fabric = nats.fabric_owned();
    let capture = match contained(DeclareCapture::start(&nats)).await {
        Ok(Ok(capture)) => capture,
        Ok(Err(e)) => {
            nats.shutdown().await;
            return Err(binding_failure(e));
        }
        Err(panic) => {
            nats.shutdown().await;
            return Err(binding_failure(panic));
        }
    };

    let ctx = CheckContext {
        fabric: &fabric,
        readyz: &readyz,
        capture: &capture,
        expected,
        service_key: &service_key,
        behavior,
        timeout,
    };

    let mut report = ConformanceReport::default();
    for scenario in scenarios {
        let outcome = run_scenario(*scenario, &ctx).await;
        report.push(name_the_attach_prerequisite(*scenario, outcome));
    }
    capture.stop().await;
    nats.shutdown().await;
    Ok(report)
}

fn name_the_attach_prerequisite(scenario: Scenario, mut outcome: CheckOutcome) -> CheckOutcome {
    if outcome.status != CheckStatus::Fail {
        return outcome;
    }
    if let Some(prerequisite) = scenario.attach_prerequisite() {
        let detail = outcome.detail.take().unwrap_or_default();
        outcome.detail = Some(format!("{detail}; in attach mode, {prerequisite}"));
    }
    outcome
}

fn missing_streams() -> ConformanceError {
    ConformanceError::Jetstream(
        "the fixed streams INTEGRATION_CMD and INTEGRATION_EVT must already exist in \
         attach mode; the battery never provisions infra"
            .to_string(),
    )
}

fn binding_failure(cause: impl std::fmt::Display) -> ConformanceError {
    ConformanceError::Jetstream(format!(
        "binding the declare consumer to the fixed handshake stream failed — in attach mode \
         the service owns the fixed handshake stream and it must already exist: {cause}"
    ))
}
