use std::path::PathBuf;
use std::time::Duration;

use br_test_harness::wait_until;
use uuid::Uuid;

use crate::checks::{CheckContext, run_scenario};
use crate::error::{ConformanceError, Result};
use crate::harness::IdentityHarness;
use crate::outcome::ConformanceReport;
use crate::readyz::ReadyzProbe;
use crate::scenario::Scenario;
use crate::subject::{Subject, SubjectConfig};

pub struct SpawnTarget {
    pub binary: PathBuf,
}

pub async fn run_spawn(
    target: &SpawnTarget,
    scenarios: &[Scenario],
    timeout: Duration,
) -> Result<ConformanceReport> {
    let harness = IdentityHarness::start_with_binary(target.binary.clone()).await?;
    let capture = harness.capture_confirmations().await?;
    let declarer = harness.declarer();
    let config = SubjectConfig::new(&harness.nats_url());
    let subject = Subject::spawn(harness.binary(), &config);
    let readyz = ReadyzProbe::new(format!("{}/readyz", subject.base_url()))?;

    if !wait_until(timeout, || async { readyz.is_ready().await }).await {
        let logs = subject.logs();
        subject.shutdown().await;
        capture.stop().await;
        harness.shutdown().await;
        return Err(ConformanceError::Timeout(format!(
            "the acceptor never reported /readyz=200; logs:\n{logs}"
        )));
    }

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

    subject.shutdown().await;
    capture.stop().await;
    harness.shutdown().await;
    Ok(report)
}
