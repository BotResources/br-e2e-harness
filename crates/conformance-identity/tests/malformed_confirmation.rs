use std::time::Duration;

use br_core_integration::IntegrationEvent;
use br_scope_declaration_contract::{VERSION, declare_command_coords, rejected_event_coords};
use br_test_harness::FabricTestNats;
use conformance_identity::checks::{CheckContext, run_scenario};
use conformance_identity::declarer::declaring_metadata;
use conformance_identity::{CheckOutcome, ConfirmationCapture, Declarer, Scenario};

const SHORT: Duration = Duration::from_secs(5);
const NAMESPACE: &str = "malformed";

enum Forgery {
    NotJson,
    EnvelopeWithAnUnreadablePayload,
}

async fn run_against_a_subject_answering(forgery: Forgery) -> CheckOutcome {
    let nats = FabricTestNats::start().await;
    let capture = ConfirmationCapture::start(&nats).await.expect("capture");
    let declarer = Declarer::new(nats.fabric_owned());
    let declare_coords = declare_command_coords().expect("declare coordinates");
    let commands = nats.capture_commands(&[&declare_coords]).await;
    let rejected_coords = rejected_event_coords().expect("rejected coordinates");

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
        let body = match forgery {
            Forgery::NotJson => b"this is not an envelope".to_vec(),
            Forgery::EnvelopeWithAnUnreadablePayload => {
                let event = IntegrationEvent::new(
                    uuid::Uuid::now_v7(),
                    "service_scope.rejected",
                    VERSION,
                    chrono::Utc::now(),
                    declaring_metadata(correlation_id),
                    serde_json::json!({"unexpected": "shape"}),
                );
                serde_json::to_vec(&event).expect("serialize the forged confirmation")
            }
        };
        nats.publish_event_envelope(&rejected_coords, &body).await;
    };
    let (outcome, ()) = tokio::join!(run_scenario(Scenario::PrefixMismatchRejected, &ctx), forger);

    commands.stop().await;
    capture.stop().await;
    nats.shutdown().await;
    outcome
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_body_that_is_not_an_envelope_is_reported_as_a_wire_shape_violation_not_as_absence() {
    let outcome = run_against_a_subject_answering(Forgery::NotJson).await;
    assert!(!outcome.is_pass(), "{outcome:?}");
    let report = format!(
        "{} {}",
        outcome.observed,
        outcome.detail.clone().unwrap_or_default()
    );
    assert!(
        report.contains("not an envelope"),
        "the report must name the wire-shape violation: {outcome:?}"
    );
    assert!(
        !report.contains("no confirmation arrived"),
        "a malformed confirmation is not absence: {outcome:?}"
    );
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn an_envelope_with_an_unreadable_payload_is_reported_as_a_wire_shape_violation() {
    let outcome = run_against_a_subject_answering(Forgery::EnvelopeWithAnUnreadablePayload).await;
    assert!(!outcome.is_pass(), "{outcome:?}");
    assert!(
        outcome
            .observed
            .contains("did not deserialize into the real IntegrationEvent"),
        "{outcome:?}"
    );
}
