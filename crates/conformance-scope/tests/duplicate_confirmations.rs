mod common;

use std::time::Duration;

use br_core_integration::{Actor, EventMetadata, IntegrationCommand, UserId};
use br_core_scope::ServiceKey;
use br_scope_declaration_contract::declare_command_coords;
use br_test_harness::FabricTestNats;
use common::ReadyzStandIn;
use conformance_scope::checks::{CheckContext, duplicate_confirmations_tolerated};
use conformance_scope::{AcceptorBehavior, DeclareCapture, ExpectedDeclaration, ReadyzProbe};
use uuid::Uuid;

const SERVICE_KEY: &str = "notifier";

async fn run_s5_against(fails_after: Option<Duration>) -> conformance_scope::CheckOutcome {
    let nats = FabricTestNats::start().await;
    let capture = DeclareCapture::start(&nats).await.expect("capture");
    let coords = declare_command_coords().expect("declare coordinates");
    let declare = IntegrationCommand::new(
        Uuid::now_v7(),
        "service_scope.declare",
        1,
        chrono::Utc::now(),
        EventMetadata::new(Actor::Human(UserId::from(Uuid::now_v7())), Uuid::now_v7()),
        serde_json::json!({}),
    );
    nats.fabric()
        .publish_command(&coords, &declare)
        .await
        .expect("publish the declare");

    let stand_in = ReadyzStandIn::start(fails_after).await;
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
        timeout: Duration::from_secs(10),
    };
    let outcome = duplicate_confirmations_tolerated(&ctx).await;

    capture.stop().await;
    nats.shutdown().await;
    outcome
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_subject_that_drops_to_503_after_the_duplicate_acceptance_fails_s5() {
    let outcome = run_s5_against(Some(Duration::from_secs(1))).await;
    assert!(!outcome.is_pass(), "{outcome:?}");
    let detail = outcome.detail.clone().unwrap_or_default();
    assert!(detail.contains("left 200"), "{outcome:?}");
    assert!(detail.contains("503"), "{outcome:?}");
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_subject_that_stays_ready_through_the_quiet_window_passes_s5() {
    let outcome = run_s5_against(None).await;
    assert!(outcome.is_pass(), "{outcome:?}");
}
