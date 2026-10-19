mod common;

use std::time::Duration;

use br_core_integration::{Actor, EventMetadata, IntegrationCommand, UserId};
use br_core_scope::ServiceKey;
use br_scope_declaration_contract::declare_command_coords;
use br_test_harness::FabricTestNats;
use br_test_harness::nats::TestNats;
use common::ReadyzStandIn;
use conformance_scope::checks::{
    CheckContext, declaration_content, declare_well_formed, disabled_mode_ready_without_declare,
    republishes_same_correlation_id,
};
use conformance_scope::{
    AcceptorBehavior, CheckOutcome, DeclareCapture, ExpectedDeclaration, ReadyzProbe,
    declare_subject,
};
use uuid::Uuid;

const SERVICE_KEY: &str = "notifier";

async fn run_against_a_malformed_then_a_valid_declare(
    check: impl for<'a> AsyncFnOnce(&'a CheckContext<'a>) -> CheckOutcome,
) -> CheckOutcome {
    let nats = FabricTestNats::start().await;
    let capture = DeclareCapture::start(&nats).await.expect("capture");
    let raw = TestNats::setup_on(&nats.url()).await;
    raw.publish_raw(
        &declare_subject().expect("declare subject"),
        br#"{"command_id":"not-an-envelope","payload":{}}"#.to_vec(),
    )
    .await;
    let coords = declare_command_coords().expect("declare coordinates");
    let valid = IntegrationCommand::new(
        Uuid::now_v7(),
        "service_scope.declare",
        1,
        chrono::Utc::now(),
        EventMetadata::new(Actor::Human(UserId::from(Uuid::now_v7())), Uuid::now_v7()),
        serde_json::json!({}),
    );
    nats.fabric()
        .publish_command(&coords, &valid)
        .await
        .expect("publish the valid re-publish");

    let stand_in = ReadyzStandIn::start(None).await;
    let readyz = ReadyzProbe::new(stand_in.url.clone()).expect("readyz probe");
    let expected = ExpectedDeclaration::new(SERVICE_KEY, Vec::new());
    let key = ServiceKey::new(SERVICE_KEY).expect("service key");
    let behavior = AcceptorBehavior::Accept;
    let ctx = CheckContext {
        fabric: nats.fabric(),
        readyz: &readyz,
        capture: &capture,
        expected: &expected,
        service_key: &key,
        behavior: &behavior,
        timeout: Duration::from_secs(5),
    };
    let outcome = check(&ctx).await;

    raw.cleanup().await;
    capture.stop().await;
    nats.shutdown().await;
    outcome
}

fn names_the_wire_shape_violation(outcome: &CheckOutcome) {
    assert!(!outcome.is_pass(), "{outcome:?}");
    let report = format!(
        "{} {}",
        outcome.observed,
        outcome.detail.clone().unwrap_or_default()
    );
    assert!(
        report.contains("metadata.correlation_id"),
        "the report must name the missing envelope metadata: {outcome:?}"
    );
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn s1_reports_a_declare_without_metadata_instead_of_hiding_it_behind_a_later_valid_one() {
    names_the_wire_shape_violation(
        &run_against_a_malformed_then_a_valid_declare(declare_well_formed).await,
    );
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn declaration_content_reports_a_declare_without_metadata() {
    names_the_wire_shape_violation(
        &run_against_a_malformed_then_a_valid_declare(declaration_content).await,
    );
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn s3_reports_a_declare_without_metadata() {
    names_the_wire_shape_violation(
        &run_against_a_malformed_then_a_valid_declare(republishes_same_correlation_id).await,
    );
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn s6_reports_a_declare_without_metadata_as_a_published_declare() {
    names_the_wire_shape_violation(
        &run_against_a_malformed_then_a_valid_declare(disabled_mode_ready_without_declare).await,
    );
}
