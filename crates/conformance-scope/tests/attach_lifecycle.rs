use std::time::Duration;

use conformance_scope::{
    AcceptorBehavior, AttachTarget, CheckStatus, ConformanceReport, ExpectedDeclaration,
    ExpectedScope, Scenario, ScopeHarness, Subject, SubjectConfig, provision, run_attach,
};

const SERVICE_KEY: &str = "notifier";

fn expected() -> ExpectedDeclaration {
    ExpectedDeclaration::new(
        SERVICE_KEY,
        vec![ExpectedScope {
            key: "notifier:read".to_string(),
            platform_only: false,
        }],
    )
}

fn config(harness: &ScopeHarness, wait_timeout: &str) -> SubjectConfig {
    SubjectConfig::new(
        &harness.nats_url(),
        harness.stream_name(),
        harness.event_stream_name(),
        SERVICE_KEY,
    )
    .scope_keys(&expected().scope_keys_csv())
    .label_key("label.notifier")
    .description_key("desc.notifier")
    .wait_timeout(wait_timeout)
}

async fn attach(
    harness: &ScopeHarness,
    subject: &Subject,
    scenario: Scenario,
    timeout: Duration,
) -> ConformanceReport {
    let target = AttachTarget {
        nats_url: harness.nats_url(),
        readyz_url: format!("{}/readyz", subject.base_url()),
    };
    run_attach(
        &target,
        &expected(),
        &AcceptorBehavior::Accept,
        &[scenario],
        timeout,
    )
    .await
    .expect("attach run")
}

async fn booted(
    config_for: impl FnOnce(&ScopeHarness) -> SubjectConfig,
) -> (ScopeHarness, Subject) {
    let harness = ScopeHarness::start().await.expect("harness");
    provision(&harness.nats_url(), "scope_declaration.toml")
        .await
        .expect("fabric-nats provision");
    let subject = Subject::start(harness.binary(), &config_for(&harness))
        .await
        .expect("start the subject");
    (harness, subject)
}

fn only_outcome(report: &ConformanceReport) -> &conformance_scope::CheckOutcome {
    assert_eq!(report.outcomes.len(), 1, "{report:?}");
    &report.outcomes[0]
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server`, `fabric-nats` and the `scope-service` binary on PATH"]
async fn s3_passes_in_attach_mode_for_a_service_that_republishes_within_the_timeout() {
    let (harness, subject) = booted(|h| config(h, "500ms")).await;

    let report = attach(
        &harness,
        &subject,
        Scenario::RepublishesSameCorrelationId,
        Duration::from_secs(10),
    )
    .await;

    subject.shutdown().await;
    harness.shutdown().await;
    assert_eq!(
        only_outcome(&report).status,
        CheckStatus::Pass,
        "{report:?}"
    );
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server`, `fabric-nats` and the `scope-service` binary on PATH"]
async fn s3_in_attach_mode_names_its_prerequisite_when_the_service_waits_longer_than_the_timeout() {
    let (harness, subject) = booted(|h| config(h, "30s")).await;

    let report = attach(
        &harness,
        &subject,
        Scenario::RepublishesSameCorrelationId,
        Duration::from_secs(2),
    )
    .await;

    subject.shutdown().await;
    harness.shutdown().await;
    let outcome = only_outcome(&report);
    assert_eq!(outcome.status, CheckStatus::Fail, "{report:?}");
    let detail = outcome.detail.clone().unwrap_or_default();
    assert!(detail.contains("in attach mode"), "{detail}");
    assert!(detail.contains("--timeout above"), "{detail}");
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server`, `fabric-nats` and the `scope-service` binary on PATH"]
async fn s6_passes_in_attach_mode_for_a_service_booted_with_declaration_disabled() {
    let (harness, subject) = booted(|h| config(h, "500ms").enabled(false)).await;

    let report = attach(
        &harness,
        &subject,
        Scenario::DisabledModeReadyWithoutDeclare,
        Duration::from_secs(10),
    )
    .await;

    subject.shutdown().await;
    harness.shutdown().await;
    assert_eq!(
        only_outcome(&report).status,
        CheckStatus::Pass,
        "{report:?}"
    );
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server`, `fabric-nats` and the `scope-service` binary on PATH"]
async fn s6_in_attach_mode_names_its_prerequisite_for_a_service_booted_with_declaration_enabled() {
    let (harness, subject) = booted(|h| config(h, "500ms")).await;

    let report = attach(
        &harness,
        &subject,
        Scenario::DisabledModeReadyWithoutDeclare,
        Duration::from_secs(2),
    )
    .await;

    subject.shutdown().await;
    harness.shutdown().await;
    let outcome = only_outcome(&report);
    assert_eq!(outcome.status, CheckStatus::Fail, "{report:?}");
    let detail = outcome.detail.clone().unwrap_or_default();
    assert!(detail.contains("in attach mode"), "{detail}");
    assert!(
        detail.contains("booted with scope declaration disabled"),
        "{detail}"
    );
}
