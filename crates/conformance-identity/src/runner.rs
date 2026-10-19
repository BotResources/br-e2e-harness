use std::time::Duration;

use br_test_harness::{FabricTestNats, wait_until};
use uuid::Uuid;

use crate::capture::ConfirmationCapture;
use crate::checks::{CheckContext, run_scenario};
use crate::containment::contained;
use crate::declarer::Declarer;
use crate::error::{ConformanceError, Result};
use crate::outcome::ConformanceReport;
use crate::readyz::ReadyzProbe;
use crate::scenario::{Scenario, require_selection};

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

pub struct AttachTarget {
    pub nats_url: String,
    pub readyz_url: String,
}

pub async fn run_attach(
    target: &AttachTarget,
    scenarios: &[Scenario],
    timeout: Duration,
) -> Result<ConformanceReport> {
    require_selection(scenarios)?;
    let readyz = ReadyzProbe::new(&target.readyz_url)?;
    let fabric_nats = contained(FabricTestNats::attach_without_provisioning(
        &target.nats_url,
    ))
    .await
    .map_err(|e| {
        ConformanceError::Jetstream(format!(
            "connecting the harness to NATS at {}: {e}",
            target.nats_url
        ))
    })?;

    match contained(fabric_nats.fixed_streams_present()).await {
        Ok(true) => {}
        Ok(false) => {
            fabric_nats.shutdown().await;
            return Err(ConformanceError::Jetstream(format!(
                "the fixed streams INTEGRATION_CMD and INTEGRATION_EVT are not both present on \
                 NATS at {}: in attach mode the deployment owns them",
                target.nats_url
            )));
        }
        Err(e) => {
            fabric_nats.shutdown().await;
            return Err(ConformanceError::Jetstream(format!(
                "reading the fixed streams on NATS at {}: {e}",
                target.nats_url
            )));
        }
    }

    if !wait_until(timeout, || async { readyz.is_ready().await }).await {
        fabric_nats.shutdown().await;
        return Err(ConformanceError::Readyz(format!(
            "the attached acceptor at {} never reported /readyz=200",
            target.readyz_url
        )));
    }

    let capture = match ConfirmationCapture::start(&fabric_nats).await {
        Ok(capture) => capture,
        Err(e) => {
            fabric_nats.shutdown().await;
            return Err(ConformanceError::Jetstream(format!(
                "binding the confirmation consumer to the fixed event stream failed on NATS at \
                 {}: {e}",
                target.nats_url
            )));
        }
    };
    let declarer = Declarer::new(fabric_nats.fabric_owned());

    let namespace = Uuid::now_v7().simple().to_string();
    let ctx = CheckContext {
        declarer: &declarer,
        capture: &capture,
        namespace: &namespace,
        timeout,
    };

    let mut report = ConformanceReport::default();
    for scenario in scenarios {
        report.push(run_scenario(*scenario, &ctx).await);
    }
    capture.stop().await;
    fabric_nats.shutdown().await;
    Ok(report)
}
