use std::time::Duration;

use br_core_scope::{ScopeDeclarationError, ServiceKey};
use br_test_harness::wait_until;
use conformance_scope::{ScopeHarness, Subject, SubjectConfig, accept, reject};
use reqwest::StatusCode;

const SERVICE_KEY: &str = "notifier";
const SCOPE_KEYS: &str = "notifier:read,notifier:admin";
const SHORT: Duration = Duration::from_secs(10);

fn config(harness: &ScopeHarness) -> SubjectConfig {
    SubjectConfig::new(&harness.nats_url(), harness.stream_name(), SERVICE_KEY)
        .scope_keys(SCOPE_KEYS)
        .label_key("label.notifier")
        .description_key("desc.notifier")
        .wait_timeout("500ms")
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn s1_declare_on_boot_is_well_formed() {
    let harness = ScopeHarness::start().await.expect("harness");
    let capture = harness.capture_declares().await.expect("capture");

    let subject = Subject::spawn(harness.binary(), &config(&harness));

    let arrived = wait_until(SHORT, || async { capture.count() >= 1 }).await;
    assert!(
        arrived,
        "the subject must declare on boot; logs:\n{}",
        subject.logs()
    );

    let declare = capture.first().expect("a declare was captured");
    let command = declare
        .decode()
        .expect("the declare must deserialize into IntegrationCommand<DeclareServiceScopes>");

    let validated = command
        .payload
        .validate()
        .expect("the declared scopes must validate against the real domain types");
    assert_eq!(validated.manifest().key.as_str(), SERVICE_KEY);

    let owner = ServiceKey::new(SERVICE_KEY).unwrap();
    for spec in validated.scopes() {
        assert!(
            spec.key.is_owned_by(&owner),
            "scope {} must be owned by the declaring service",
            spec.key.as_str()
        );
    }
    let declared: Vec<&str> = validated.scopes().iter().map(|s| s.key.as_str()).collect();
    assert_eq!(declared, vec!["notifier:read", "notifier:admin"]);

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

    assert!(
        wait_until(SHORT, || async { capture.count() >= 1 }).await,
        "a declare must be published before acceptance; logs:\n{}",
        subject.logs()
    );
    assert!(
        subject.not_ready().await,
        "/readyz must be 503 before the acceptor confirms"
    );

    let cid = capture.first().expect("declare").correlation_id;
    let owner = ServiceKey::new(SERVICE_KEY).unwrap();
    accept(harness.jetstream(), &owner, cid)
        .await
        .expect("accept");

    assert!(
        wait_until(SHORT, || async { subject.ready().await }).await,
        "/readyz must reach 200 after acceptance; logs:\n{}",
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

    assert!(
        wait_until(SHORT, || async { capture.count() >= 2 }).await,
        "the subject must re-publish past WAIT_TIMEOUT; logs:\n{}",
        subject.logs()
    );

    let ids = capture.correlation_ids();
    let first = ids[0];
    assert!(
        ids.iter().all(|id| *id == first),
        "every re-publish must carry the SAME correlation_id, got {ids:?}"
    );

    let owner = ServiceKey::new(SERVICE_KEY).unwrap();
    accept(harness.jetstream(), &owner, first)
        .await
        .expect("accept");
    assert!(
        wait_until(SHORT, || async { subject.ready().await }).await,
        "/readyz must reach 200 once the re-published id is accepted; logs:\n{}",
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

    assert!(
        wait_until(SHORT, || async { capture.count() >= 1 }).await,
        "a declare must be published; logs:\n{}",
        subject.logs()
    );
    let cid = capture.first().expect("declare").correlation_id;

    let owner = ServiceKey::new(SERVICE_KEY).unwrap();
    let reason = ScopeDeclarationError::ScopeOwnedByAnotherService {
        key: "notifier:read".to_string(),
        owner: "billing".to_string(),
    };
    let expected_body = format!("scope declaration rejected: {reason}");
    reject(harness.jetstream(), &owner, reason, cid)
        .await
        .expect("reject");

    let received = wait_until(SHORT, || async {
        subject.readyz_body().await.as_deref() == Some(expected_body.as_str())
    })
    .await;
    assert!(
        received,
        "the subject must process the rejection and surface its reason in /readyz; expected {expected_body:?}; logs:\n{}",
        subject.logs()
    );

    let count_at_reject = capture.count();
    let still_publishing = wait_until(Duration::from_secs(3), || async {
        capture.count() > count_at_reject
    })
    .await;
    assert!(
        !still_publishing,
        "the subject must stop re-publishing after the rejection it has now processed"
    );
    assert!(
        subject.not_ready().await,
        "/readyz must stay 503 after a rejection"
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

    assert!(
        wait_until(SHORT, || async { capture.count() >= 1 }).await,
        "a declare must be published; logs:\n{}",
        subject.logs()
    );
    let cid = capture.first().expect("declare").correlation_id;

    let owner = ServiceKey::new(SERVICE_KEY).unwrap();
    accept(harness.jetstream(), &owner, cid)
        .await
        .expect("first accept");
    accept(harness.jetstream(), &owner, cid)
        .await
        .expect("second accept");

    assert!(
        wait_until(SHORT, || async { subject.ready().await }).await,
        "/readyz must reach 200 despite a duplicate acceptance; logs:\n{}",
        subject.logs()
    );
    assert_eq!(
        subject.livez_status().await,
        Some(StatusCode::OK),
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

    assert!(
        wait_until(SHORT, || async { subject.ready().await }).await,
        "/readyz must be 200 immediately in disabled mode; logs:\n{}",
        subject.logs()
    );

    let published = wait_until(Duration::from_secs(2), || async { capture.count() > 0 }).await;
    assert!(
        !published,
        "disabled mode must publish no declare command, saw {}",
        capture.count()
    );

    subject.shutdown().await;
    capture.stop().await;
    harness.shutdown().await;
}
