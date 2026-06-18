use std::path::PathBuf;
use std::time::Duration;

use crate::checks::{CheckContext, run_scenario};
use crate::error::Result;
use crate::expected::{ExpectedDeclaration, SAMPLE_FALLBACK_SCOPE};
use crate::harness::ScopeHarness;
use crate::outcome::ConformanceReport;
use crate::readyz::ReadyzProbe;
use crate::runner::service_key;
use crate::scenario::{AcceptorBehavior, Scenario};
use crate::subject::{Subject, SubjectConfig};

const SPAWN_WAIT_TIMEOUT: &str = "500ms";

pub struct SpawnTarget {
    pub binary: PathBuf,
}

pub async fn run_spawn(
    target: &SpawnTarget,
    expected: &ExpectedDeclaration,
    behavior: &AcceptorBehavior,
    scenarios: &[Scenario],
    timeout: Duration,
) -> Result<ConformanceReport> {
    let service_key = service_key(expected)?;
    let mut report = ConformanceReport::default();

    let observed: Vec<Scenario> = scenarios
        .iter()
        .copied()
        .filter(|s| !s.requires_subject_lifecycle())
        .collect();
    if !observed.is_empty() {
        let harness = ScopeHarness::start_with_binary(target.binary.clone()).await?;
        crate::provision::provision(&harness.nats_url(), "scope_declaration.toml").await?;
        let capture = harness.capture_declares().await?;
        let subject = Subject::spawn(harness.binary(), &enabled_config(&harness, expected));
        let readyz = ReadyzProbe::new(format!("{}/readyz", subject.base_url()))?;
        let ctx = CheckContext {
            fabric: harness.fabric(),
            readyz: &readyz,
            capture: &capture,
            expected,
            service_key: &service_key,
            behavior,
            timeout,
        };
        for scenario in &observed {
            report.push(run_scenario(*scenario, &ctx).await);
        }
        subject.shutdown().await;
        capture.stop().await;
        harness.shutdown().await;
    }

    for scenario in scenarios
        .iter()
        .copied()
        .filter(|s| s.requires_subject_lifecycle())
    {
        report.push(
            run_lifecycle_scenario(target, expected, behavior, &service_key, scenario, timeout)
                .await?,
        );
    }

    Ok(report)
}

async fn run_lifecycle_scenario(
    target: &SpawnTarget,
    expected: &ExpectedDeclaration,
    behavior: &AcceptorBehavior,
    service_key: &br_core_scope::ServiceKey,
    scenario: Scenario,
    timeout: Duration,
) -> Result<crate::outcome::CheckOutcome> {
    let harness = ScopeHarness::start_with_binary(target.binary.clone()).await?;
    crate::provision::provision(&harness.nats_url(), "scope_declaration.toml").await?;
    let capture = harness.capture_declares().await?;
    let config = match scenario {
        Scenario::DisabledModeReadyWithoutDeclare => {
            enabled_config(&harness, expected).enabled(false)
        }
        _ => enabled_config(&harness, expected),
    };
    let subject = Subject::spawn(harness.binary(), &config);
    let readyz = ReadyzProbe::new(format!("{}/readyz", subject.base_url()))?;
    let scenario_behavior = lifecycle_behavior(scenario, behavior, expected);
    let ctx = CheckContext {
        fabric: harness.fabric(),
        readyz: &readyz,
        capture: &capture,
        expected,
        service_key,
        behavior: &scenario_behavior,
        timeout,
    };
    let outcome = run_scenario(scenario, &ctx).await;
    subject.shutdown().await;
    capture.stop().await;
    harness.shutdown().await;
    Ok(outcome)
}

fn lifecycle_behavior(
    scenario: Scenario,
    global: &AcceptorBehavior,
    expected: &ExpectedDeclaration,
) -> AcceptorBehavior {
    match scenario {
        Scenario::RejectionStopsReadiness => {
            let sample = expected
                .scopes
                .first()
                .map(|s| s.key.as_str())
                .unwrap_or(SAMPLE_FALLBACK_SCOPE);
            global.spawn_rejection(sample)
        }
        _ => AcceptorBehavior::Accept,
    }
}

fn enabled_config(harness: &ScopeHarness, expected: &ExpectedDeclaration) -> SubjectConfig {
    SubjectConfig::new(
        &harness.nats_url(),
        harness.stream_name(),
        harness.event_stream_name(),
        &expected.service_key,
    )
    .scope_keys(&expected.scope_keys_csv())
    .label_key("label.example")
    .description_key("desc.example")
    .platform_only(expected.scopes.iter().all(|s| s.platform_only) && !expected.scopes.is_empty())
    .wait_timeout(SPAWN_WAIT_TIMEOUT)
}
