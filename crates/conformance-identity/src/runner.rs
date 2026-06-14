use std::time::Duration;

use async_nats::jetstream;
use br_test_harness::{nats::connect, wait_until};
use uuid::Uuid;

use crate::capture::ConfirmationCapture;
use crate::checks::{CheckContext, run_scenario};
use crate::declarer::Declarer;
use crate::error::{ConformanceError, Result};
use crate::outcome::ConformanceReport;
use crate::readyz::ReadyzProbe;
use crate::scenario::Scenario;

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

pub struct AttachTarget {
    pub nats_url: String,
    pub readyz_url: String,
    pub stream_name: String,
}

pub async fn run_attach(
    target: &AttachTarget,
    scenarios: &[Scenario],
    timeout: Duration,
) -> Result<ConformanceReport> {
    let client = connect(&target.nats_url)
        .await
        .map_err(|e| ConformanceError::Jetstream(format!("connect '{}': {e}", target.nats_url)))?;
    let js = jetstream::new(client);
    let readyz = ReadyzProbe::new(&target.readyz_url)?;

    if !wait_until(timeout, || async { readyz.is_ready().await }).await {
        return Err(ConformanceError::Readyz(format!(
            "the attached acceptor at {} never reported /readyz=200",
            target.readyz_url
        )));
    }

    let capture = ConfirmationCapture::start(&js, &target.stream_name)
        .await
        .map_err(|e| {
            ConformanceError::Jetstream(format!(
                "binding the confirmation consumer to stream '{}' failed — in attach mode the \
                 acceptor owns the handshake stream and it must already exist: {e}",
                target.stream_name
            ))
        })?;
    let declarer = Declarer::new(js.clone());

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
    Ok(report)
}
