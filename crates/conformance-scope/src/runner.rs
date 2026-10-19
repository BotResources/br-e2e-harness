use std::time::Duration;

use br_core_scope::ServiceKey;
use br_test_harness::FabricTestNats;

use crate::capture::DeclareCapture;
use crate::checks::{CheckContext, run_scenario};
use crate::error::{ConformanceError, Result};
use crate::expected::ExpectedDeclaration;
use crate::outcome::ConformanceReport;
use crate::prerequisite::contained;
use crate::readyz::ReadyzProbe;
use crate::scenario::{AcceptorBehavior, Scenario, require_selection};

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
    require_selection(scenarios)?;
    let service_key = service_key(expected)?;
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
        report.push(run_scenario(*scenario, &ctx).await);
    }
    capture.stop().await;
    nats.shutdown().await;
    Ok(report)
}

fn binding_failure(cause: impl std::fmt::Display) -> ConformanceError {
    ConformanceError::Jetstream(format!(
        "binding the declare consumer to the fixed handshake stream failed — in attach mode \
         the service owns the fixed handshake stream and it must already exist: {cause}"
    ))
}

pub(crate) fn service_key(expected: &ExpectedDeclaration) -> Result<ServiceKey> {
    ServiceKey::new(expected.service_key.clone()).map_err(|e| {
        ConformanceError::InvalidInput(format!(
            "service key {:?} is not a valid ServiceKey: {e}",
            expected.service_key
        ))
    })
}
