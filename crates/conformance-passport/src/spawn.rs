use std::path::PathBuf;
use std::time::Duration;

use br_test_harness::wait_until;
use uuid::Uuid;

use crate::checks::{CheckContext, run_scenario};
use crate::endpoint::PassportEndpoint;
use crate::error::{ConformanceError, Result};
use crate::harness::PassportHarness;
use crate::outcome::ConformanceReport;
use crate::readyz::ReadyzProbe;
use crate::scenario::Scenario;
use crate::subject::{Subject, SubjectConfig};

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

pub struct SpawnTarget {
    pub binary: PathBuf,
}

pub async fn run_spawn(
    target: &SpawnTarget,
    scenarios: &[Scenario],
    timeout: Duration,
) -> Result<ConformanceReport> {
    let harness = PassportHarness::start_with_binary(target.binary.clone()).await?;
    let seeder = harness.seeder();
    let config = SubjectConfig::new(&harness.nats_url());
    let subject = Subject::spawn(harness.binary(), &config);
    let readyz = ReadyzProbe::new(format!("{}/readyz", subject.base_url()))?;

    if !wait_until(timeout, || async { readyz.is_ready().await }).await {
        let logs = subject.logs();
        subject.shutdown().await;
        harness.shutdown().await;
        return Err(ConformanceError::Timeout(format!(
            "the passport subject never reported /readyz=200; logs:\n{logs}"
        )));
    }

    let endpoint = PassportEndpoint::new(subject.base_url())?;
    let namespace = Uuid::now_v7().simple().to_string();
    let ctx = CheckContext {
        seeder: &seeder,
        endpoint: &endpoint,
        namespace: &namespace,
    };

    let mut report = ConformanceReport::default();
    for scenario in scenarios {
        report.push(run_scenario(*scenario, &ctx).await);
    }

    subject.shutdown().await;
    harness.shutdown().await;
    Ok(report)
}
