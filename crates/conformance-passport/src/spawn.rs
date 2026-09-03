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
    if scenarios.is_empty() {
        return Err(ConformanceError::InvalidInput(
            "run_spawn requires at least one scenario; pass &ALL".to_string(),
        ));
    }
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
        harness: &harness,
        seeder: &seeder,
        endpoint: &endpoint,
        namespace: &namespace,
    };

    let mut report = ConformanceReport::default();
    for scenario in ordered_for_destruction(scenarios) {
        report.push(run_scenario(scenario, &ctx).await);
    }

    subject.shutdown().await;
    harness.shutdown().await;
    Ok(report)
}

fn ordered_for_destruction(scenarios: &[Scenario]) -> Vec<Scenario> {
    let mut ordered: Vec<Scenario> = scenarios
        .iter()
        .filter(|s| !s.is_destructive())
        .copied()
        .collect();
    ordered.extend(scenarios.iter().filter(|s| s.is_destructive()).copied());
    ordered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destructive_scenarios_run_last() {
        let input = [
            Scenario::KvErrorFailsLoud,
            Scenario::ValidBearerResolvesToPassport,
            Scenario::TamperedEnvelopeFailsClosed,
        ];
        let ordered = ordered_for_destruction(&input);
        assert_eq!(*ordered.last().unwrap(), Scenario::KvErrorFailsLoud);
        assert_eq!(ordered.len(), input.len());
    }

    #[tokio::test]
    async fn run_spawn_rejects_an_empty_scenario_set_before_touching_infra() {
        let target = SpawnTarget {
            binary: PathBuf::from("/nonexistent/never-spawned"),
        };
        let err = run_spawn(&target, &[], DEFAULT_TIMEOUT)
            .await
            .expect_err("an empty scenario set must be rejected up front");
        assert!(matches!(err, ConformanceError::InvalidInput(_)));
    }
}
