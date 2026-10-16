use std::time::Duration;

use br_core_integration::IntegrationEvent;
use br_core_scope::{ServiceKey, ServiceScopesRejected};
use br_scope_declaration_contract::{VERSION, declare_command_coords, rejected_event_coords};
use br_test_harness::FabricTestNats;
use conformance_identity::checks::{CheckContext, run_scenario};
use conformance_identity::declarer::declaring_metadata;
use conformance_identity::{
    CheckOutcome, ConfirmationCapture, Declarer, Scenario, Verdict, expected_step_confirmations,
};

const SHORT: Duration = Duration::from_secs(10);
const NAMESPACE: &str = "forged";

async fn run_against_a_subject_naming(
    scenario: Scenario,
    named_service: Option<&str>,
) -> CheckOutcome {
    let nats = FabricTestNats::start().await;
    let capture = ConfirmationCapture::start(&nats).await.expect("capture");
    let declarer = Declarer::new(nats.fabric_owned());
    let declare_coords = declare_command_coords().expect("declare coordinates");
    let commands = nats.capture_commands(&[&declare_coords]).await;
    let rejected_coords = rejected_event_coords().expect("rejected coordinates");

    let oracle = expected_step_confirmations(&scenario.sequence(NAMESPACE)).expect("oracle");
    let want = oracle.last().expect("one step at least").clone();
    let Verdict::Rejected { reason } = want.verdict.clone() else {
        panic!("this test forges a rejection");
    };
    let service = ServiceKey::new(named_service.unwrap_or(&want.service)).expect("service key");

    let ctx = CheckContext {
        declarer: &declarer,
        capture: &capture,
        namespace: NAMESPACE,
        timeout: SHORT,
    };
    let forger = async {
        let correlation_id = loop {
            if let Some(id) = commands.correlation_ids().first() {
                break *id;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        };
        let event = IntegrationEvent::new(
            uuid::Uuid::now_v7(),
            "service_scope.rejected",
            VERSION,
            chrono::Utc::now(),
            declaring_metadata(correlation_id),
            ServiceScopesRejected::new(service, reason),
        );
        nats.publish_event_envelope(
            &rejected_coords,
            &serde_json::to_vec(&event).expect("serialize the forged confirmation"),
        )
        .await;
    };
    let (outcome, ()) = tokio::join!(run_scenario(scenario, &ctx), forger);

    commands.stop().await;
    capture.stop().await;
    nats.shutdown().await;
    outcome
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_rejection_naming_the_declaring_service_passes() {
    let outcome = run_against_a_subject_naming(Scenario::PrefixMismatchRejected, None).await;
    assert!(outcome.is_pass(), "{outcome:?}");
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_rejection_naming_another_service_fails() {
    let outcome =
        run_against_a_subject_naming(Scenario::PrefixMismatchRejected, Some("billing")).await;
    assert!(!outcome.is_pass(), "{outcome:?}");
    assert!(
        outcome.observed.contains("service=billing"),
        "the report must show the service the subject named: {outcome:?}"
    );
}
