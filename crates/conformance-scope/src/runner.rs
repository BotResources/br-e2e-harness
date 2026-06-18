use std::time::Duration;

use async_nats::jetstream;
use br_core_scope::ServiceKey;
use br_test_harness::nats::connect;

use crate::capture::DeclareCapture;
use crate::checks::{CheckContext, run_scenario};
use crate::error::{ConformanceError, Result};
use crate::expected::ExpectedDeclaration;
use crate::harness::COMMAND_STREAM_NAME;
use crate::outcome::ConformanceReport;
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
    let service_key = service_key(expected)?;
    let client = connect(&target.nats_url)
        .await
        .map_err(|e| ConformanceError::Jetstream(format!("connect '{}': {e}", target.nats_url)))?;
    let js = jetstream::new(client);
    let readyz = ReadyzProbe::new(&target.readyz_url)?;
    let capture = DeclareCapture::start(&js, COMMAND_STREAM_NAME)
        .await
        .map_err(|e| {
            ConformanceError::Jetstream(format!(
                "binding the declare consumer to stream '{COMMAND_STREAM_NAME}' failed — in attach \
                 mode the service owns the fixed handshake stream and it must already exist: {e}"
            ))
        })?;

    let ctx = CheckContext {
        js: &js,
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
