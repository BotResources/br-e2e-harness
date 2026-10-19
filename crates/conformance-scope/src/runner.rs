use std::time::Duration;

use br_core_scope::ServiceKey;
use br_test_harness::FabricTestNats;

use crate::capture::DeclareCapture;
use crate::checks::{CheckContext, run_scenario};
use crate::error::{ConformanceError, Result};
use crate::expected::ExpectedDeclaration;
use crate::outcome::ConformanceReport;
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
    let nats = FabricTestNats::connect(&target.nats_url).await;
    let fabric = nats.fabric_owned();
    let capture = match DeclareCapture::start(&nats).await {
        Ok(capture) => capture,
        Err(e) => {
            nats.shutdown().await;
            return Err(ConformanceError::Jetstream(format!(
                "binding the declare consumer to the fixed handshake stream failed — in attach \
                 mode the service owns the fixed handshake stream and it must already exist: {e}"
            )));
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

pub(crate) fn service_key(expected: &ExpectedDeclaration) -> Result<ServiceKey> {
    ServiceKey::new(expected.service_key.clone()).map_err(|e| {
        ConformanceError::InvalidInput(format!(
            "service key {:?} is not a valid ServiceKey: {e}",
            expected.service_key
        ))
    })
}
