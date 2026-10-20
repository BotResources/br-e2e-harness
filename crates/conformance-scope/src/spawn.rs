use std::path::PathBuf;
use std::time::Duration;

use crate::checks::{CheckContext, run_scenario};
use crate::error::{ConformanceError, Result};
use crate::expected::{ExpectedDeclaration, SAMPLE_FALLBACK_SCOPE};
use crate::harness::ScopeHarness;
use crate::outcome::{CheckOutcome, ConformanceReport};
use crate::preflight::{check_spawn_input, uniform_platform_only};
use crate::prerequisite::contained;
use crate::readyz::ReadyzProbe;
use crate::scenario::{AcceptorBehavior, Scenario};
use crate::subject::{BootWatch, Subject, SubjectConfig};

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
    check_spawn_input(target, expected, scenarios, timeout)?;
    let service_key = expected.validated_service_key()?;
    let platform_only = uniform_platform_only(expected)?;
    let mut report = ConformanceReport::default();

    let observed: Vec<Scenario> = scenarios
        .iter()
        .copied()
        .filter(|s| !s.requires_subject_lifecycle())
        .collect();
    if !observed.is_empty() {
        let harness = ScopeHarness::start_with_binary(target.binary.clone()).await?;
        let outcomes = observe_batch(
            &harness,
            expected,
            platform_only,
            behavior,
            &service_key,
            &observed,
            timeout,
        )
        .await;
        harness.shutdown().await;
        for outcome in outcomes? {
            report.push(outcome);
        }
    }

    for scenario in scenarios
        .iter()
        .copied()
        .filter(|s| s.requires_subject_lifecycle())
    {
        report.push(
            run_lifecycle_scenario(
                target,
                expected,
                platform_only,
                behavior,
                &service_key,
                scenario,
                timeout,
            )
            .await?,
        );
    }

    Ok(report)
}

async fn observe_batch(
    harness: &ScopeHarness,
    expected: &ExpectedDeclaration,
    platform_only: bool,
    behavior: &AcceptorBehavior,
    service_key: &br_core_scope::ServiceKey,
    scenarios: &[Scenario],
    timeout: Duration,
) -> Result<Vec<CheckOutcome>> {
    crate::provision::provision(&harness.nats_url(), "scope_declaration.toml").await?;
    let capture = harness.capture_declares().await?;
    let config = enabled_config(harness, expected, platform_only);
    let subject = match start_subject(harness, &config, timeout).await {
        Ok(subject) => subject,
        Err(e) => {
            capture.stop().await;
            return Err(e);
        }
    };
    let outcomes = match ReadyzProbe::new(format!("{}/readyz", subject.base_url())) {
        Ok(readyz) => {
            let ctx = CheckContext {
                fabric: harness.fabric(),
                readyz: &readyz,
                capture: &capture,
                expected,
                service_key,
                behavior,
                timeout,
            };
            let mut outcomes = Vec::new();
            for scenario in scenarios {
                outcomes.push(run_scenario(*scenario, &ctx).await);
            }
            Ok(outcomes)
        }
        Err(e) => Err(e),
    };
    subject.shutdown().await;
    capture.stop().await;
    outcomes
}

async fn start_subject(
    harness: &ScopeHarness,
    config: &SubjectConfig,
    timeout: Duration,
) -> Result<Subject> {
    let watch = BootWatch {
        lookout: timeout,
        ..BootWatch::default()
    };
    contained(Subject::start_with(harness.binary(), config, watch))
        .await
        .map_err(|e| {
            ConformanceError::InvalidInput(format!(
                "starting the subject binary {}: {e}",
                harness.binary().display()
            ))
        })?
}

async fn run_lifecycle_scenario(
    target: &SpawnTarget,
    expected: &ExpectedDeclaration,
    platform_only: bool,
    behavior: &AcceptorBehavior,
    service_key: &br_core_scope::ServiceKey,
    scenario: Scenario,
    timeout: Duration,
) -> Result<CheckOutcome> {
    let harness = ScopeHarness::start_with_binary(target.binary.clone()).await?;
    let scenario_behavior = lifecycle_behavior(scenario, behavior, expected);
    let config = match scenario {
        Scenario::DisabledModeReadyWithoutDeclare => {
            enabled_config(&harness, expected, platform_only).enabled(false)
        }
        _ => enabled_config(&harness, expected, platform_only),
    };
    let outcome = observe_lifecycle(
        &harness,
        expected,
        &config,
        &scenario_behavior,
        service_key,
        scenario,
        timeout,
    )
    .await;
    harness.shutdown().await;
    outcome
}

async fn observe_lifecycle(
    harness: &ScopeHarness,
    expected: &ExpectedDeclaration,
    config: &SubjectConfig,
    behavior: &AcceptorBehavior,
    service_key: &br_core_scope::ServiceKey,
    scenario: Scenario,
    timeout: Duration,
) -> Result<CheckOutcome> {
    crate::provision::provision(&harness.nats_url(), "scope_declaration.toml").await?;
    let capture = harness.capture_declares().await?;
    let subject = match start_subject(harness, config, timeout).await {
        Ok(subject) => subject,
        Err(e) => {
            capture.stop().await;
            return Err(e);
        }
    };
    let outcome = match ReadyzProbe::new(format!("{}/readyz", subject.base_url())) {
        Ok(readyz) => {
            let ctx = CheckContext {
                fabric: harness.fabric(),
                readyz: &readyz,
                capture: &capture,
                expected,
                service_key,
                behavior,
                timeout,
            };
            Ok(run_scenario(scenario, &ctx).await)
        }
        Err(e) => Err(e),
    };
    subject.shutdown().await;
    capture.stop().await;
    outcome
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

fn enabled_config(
    harness: &ScopeHarness,
    expected: &ExpectedDeclaration,
    platform_only: bool,
) -> SubjectConfig {
    SubjectConfig::new(
        &harness.nats_url(),
        harness.stream_name(),
        harness.event_stream_name(),
        &expected.service_key,
    )
    .scope_keys(&expected.scope_keys_csv())
    .label_key("label.example")
    .description_key("desc.example")
    .platform_only(platform_only)
    .wait_timeout(SPAWN_WAIT_TIMEOUT)
}
