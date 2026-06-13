use std::time::Duration;

use br_core_scope::ServiceKey;
use conformance_scope::checks::{
    CheckContext, declaration_content, declare_well_formed, disabled_mode_ready_without_declare,
    duplicate_confirmations_tolerated, readiness_gated, rejection_stops_readiness,
    republishes_same_correlation_id,
};
use conformance_scope::{
    AcceptorBehavior, ExpectedDeclaration, ExpectedScope, ReadyzProbe, ScopeHarness, Subject,
    SubjectConfig,
};

const SERVICE_KEY: &str = "notifier";
const SHORT: Duration = Duration::from_secs(10);

fn expected() -> ExpectedDeclaration {
    ExpectedDeclaration::new(
        SERVICE_KEY,
        vec![
            ExpectedScope {
                key: "notifier:read".to_string(),
                platform_only: false,
            },
            ExpectedScope {
                key: "notifier:admin".to_string(),
                platform_only: false,
            },
        ],
    )
}

fn config(harness: &ScopeHarness) -> SubjectConfig {
    SubjectConfig::new(&harness.nats_url(), harness.stream_name(), SERVICE_KEY)
        .scope_keys(&expected().scope_keys_csv())
        .label_key("label.notifier")
        .description_key("desc.notifier")
        .wait_timeout("500ms")
}

fn service_key() -> ServiceKey {
    ServiceKey::new(SERVICE_KEY).unwrap()
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn s1_declare_on_boot_is_well_formed() {
    let harness = ScopeHarness::start().await.expect("harness");
    let capture = harness.capture_declares().await.expect("capture");
    let subject = Subject::spawn(harness.binary(), &config(&harness));
    let readyz = ReadyzProbe::new(format!("{}/readyz", subject.base_url())).expect("readyz");
    let expected = expected();
    let key = service_key();
    let behavior = AcceptorBehavior::Accept;
    let ctx = CheckContext {
        js: harness.jetstream(),
        readyz: &readyz,
        capture: &capture,
        expected: &expected,
        service_key: &key,
        behavior: &behavior,
        timeout: SHORT,
    };

    let outcome = declare_well_formed(&ctx).await;
    assert!(
        outcome.is_pass(),
        "s1 must pass: {outcome:?}; logs:\n{}",
        subject.logs()
    );

    subject.shutdown().await;
    capture.stop().await;
    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn declaration_content_matches_expected() {
    let harness = ScopeHarness::start().await.expect("harness");
    let capture = harness.capture_declares().await.expect("capture");
    let subject = Subject::spawn(harness.binary(), &config(&harness));
    let readyz = ReadyzProbe::new(format!("{}/readyz", subject.base_url())).expect("readyz");
    let expected = expected();
    let key = service_key();
    let behavior = AcceptorBehavior::Accept;
    let ctx = CheckContext {
        js: harness.jetstream(),
        readyz: &readyz,
        capture: &capture,
        expected: &expected,
        service_key: &key,
        behavior: &behavior,
        timeout: SHORT,
    };

    let outcome = declaration_content(&ctx).await;
    assert!(
        outcome.is_pass(),
        "declaration-content must pass: {outcome:?}; logs:\n{}",
        subject.logs()
    );

    subject.shutdown().await;
    capture.stop().await;
    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn declaration_content_flags_wrong_scopes() {
    let harness = ScopeHarness::start().await.expect("harness");
    let capture = harness.capture_declares().await.expect("capture");
    let subject = Subject::spawn(harness.binary(), &config(&harness));
    let readyz = ReadyzProbe::new(format!("{}/readyz", subject.base_url())).expect("readyz");
    let wrong = ExpectedDeclaration::new(
        SERVICE_KEY,
        vec![ExpectedScope {
            key: "notifier:read".to_string(),
            platform_only: false,
        }],
    );
    let key = service_key();
    let behavior = AcceptorBehavior::Accept;
    let ctx = CheckContext {
        js: harness.jetstream(),
        readyz: &readyz,
        capture: &capture,
        expected: &wrong,
        service_key: &key,
        behavior: &behavior,
        timeout: SHORT,
    };

    let outcome = declaration_content(&ctx).await;
    assert!(
        !outcome.is_pass(),
        "a wrong expected scope set must fail the content check"
    );
    let detail = outcome.detail.unwrap_or_default();
    assert!(
        detail.contains("scope set mismatch"),
        "the failure must read as an expected-vs-observed diff, got {detail:?}"
    );

    subject.shutdown().await;
    capture.stop().await;
    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn s2_readiness_is_gated_on_acceptance() {
    let harness = ScopeHarness::start().await.expect("harness");
    let capture = harness.capture_declares().await.expect("capture");
    let subject = Subject::spawn(harness.binary(), &config(&harness));
    let readyz = ReadyzProbe::new(format!("{}/readyz", subject.base_url())).expect("readyz");
    let expected = expected();
    let key = service_key();
    let behavior = AcceptorBehavior::Accept;
    let ctx = CheckContext {
        js: harness.jetstream(),
        readyz: &readyz,
        capture: &capture,
        expected: &expected,
        service_key: &key,
        behavior: &behavior,
        timeout: SHORT,
    };

    let outcome = readiness_gated(&ctx).await;
    assert!(
        outcome.is_pass(),
        "s2 must pass: {outcome:?}; logs:\n{}",
        subject.logs()
    );

    subject.shutdown().await;
    capture.stop().await;
    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn s3_republishes_same_correlation_id_until_answered() {
    let harness = ScopeHarness::start().await.expect("harness");
    let capture = harness.capture_declares().await.expect("capture");
    let subject = Subject::spawn(harness.binary(), &config(&harness));
    let readyz = ReadyzProbe::new(format!("{}/readyz", subject.base_url())).expect("readyz");
    let expected = expected();
    let key = service_key();
    let behavior = AcceptorBehavior::Accept;
    let ctx = CheckContext {
        js: harness.jetstream(),
        readyz: &readyz,
        capture: &capture,
        expected: &expected,
        service_key: &key,
        behavior: &behavior,
        timeout: SHORT,
    };

    let outcome = republishes_same_correlation_id(&ctx).await;
    assert!(
        outcome.is_pass(),
        "s3 must pass: {outcome:?}; logs:\n{}",
        subject.logs()
    );

    subject.shutdown().await;
    capture.stop().await;
    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn s4_rejection_keeps_subject_unready_and_stops_republishing() {
    let harness = ScopeHarness::start().await.expect("harness");
    let capture = harness.capture_declares().await.expect("capture");
    let subject = Subject::spawn(harness.binary(), &config(&harness));
    let readyz = ReadyzProbe::new(format!("{}/readyz", subject.base_url())).expect("readyz");
    let expected = expected();
    let key = service_key();
    let behavior =
        AcceptorBehavior::reject(Some("scope_owned_by_another_service"), "notifier:read")
            .expect("reject behavior");
    let ctx = CheckContext {
        js: harness.jetstream(),
        readyz: &readyz,
        capture: &capture,
        expected: &expected,
        service_key: &key,
        behavior: &behavior,
        timeout: SHORT,
    };

    let outcome = rejection_stops_readiness(&ctx).await;
    assert!(
        outcome.is_pass(),
        "s4 must pass: {outcome:?}; logs:\n{}",
        subject.logs()
    );

    subject.shutdown().await;
    capture.stop().await;
    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn s5_duplicate_confirmations_are_tolerated() {
    let harness = ScopeHarness::start().await.expect("harness");
    let capture = harness.capture_declares().await.expect("capture");
    let subject = Subject::spawn(harness.binary(), &config(&harness));
    let readyz = ReadyzProbe::new(format!("{}/readyz", subject.base_url())).expect("readyz");
    let expected = expected();
    let key = service_key();
    let behavior = AcceptorBehavior::Accept;
    let ctx = CheckContext {
        js: harness.jetstream(),
        readyz: &readyz,
        capture: &capture,
        expected: &expected,
        service_key: &key,
        behavior: &behavior,
        timeout: SHORT,
    };

    let outcome = duplicate_confirmations_tolerated(&ctx).await;
    assert!(
        outcome.is_pass(),
        "s5 must pass: {outcome:?}; logs:\n{}",
        subject.logs()
    );
    assert!(
        subject.livez_status().await.is_some(),
        "the subject must stay alive after duplicate confirmations"
    );

    subject.shutdown().await;
    capture.stop().await;
    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn s6_disabled_mode_publishes_nothing_and_is_ready_immediately() {
    let harness = ScopeHarness::start().await.expect("harness");
    let capture = harness.capture_declares().await.expect("capture");
    let subject = Subject::spawn(harness.binary(), &config(&harness).enabled(false));
    let readyz = ReadyzProbe::new(format!("{}/readyz", subject.base_url())).expect("readyz");
    let expected = expected();
    let key = service_key();
    let behavior = AcceptorBehavior::Accept;
    let ctx = CheckContext {
        js: harness.jetstream(),
        readyz: &readyz,
        capture: &capture,
        expected: &expected,
        service_key: &key,
        behavior: &behavior,
        timeout: SHORT,
    };

    let outcome = disabled_mode_ready_without_declare(&ctx).await;
    assert!(
        outcome.is_pass(),
        "s6 must pass: {outcome:?}; logs:\n{}",
        subject.logs()
    );

    subject.shutdown().await;
    capture.stop().await;
    harness.shutdown().await;
}
