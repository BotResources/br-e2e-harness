use br_core_integration::{Aggregate, Bc, CommandCoords, EventCoords, PastFact, Verb};
use br_test_harness::{BareFabricNats, FabricTestNats, WidenedDurable};
use br_util_nats_fabric::{
    FabricError, INTEGRATION_CMD, INTEGRATION_EVT, PublishErrorKind, command_subject, event_subject,
};

use crate::error::Result;
use crate::wire::{FrozenCommandSubject, FrozenEventSubject};

pub fn rust_command_subject(go: &FrozenCommandSubject) -> Result<String> {
    let coords = CommandCoords {
        receiver: Bc::new(go.receiver.clone()).map_err(FabricError::from)?,
        aggregate: Aggregate::new(go.aggregate.clone()).map_err(FabricError::from)?,
        verb: Verb::new(go.verb.clone()).map_err(FabricError::from)?,
        version: go.version,
    };
    Ok(command_subject(&coords))
}

pub fn rust_event_subject(go: &FrozenEventSubject) -> Result<String> {
    let coords = EventCoords {
        producer: Bc::new(go.producer.clone()).map_err(FabricError::from)?,
        aggregate: Aggregate::new(go.aggregate.clone()).map_err(FabricError::from)?,
        fact: PastFact::new(go.fact.clone()).map_err(FabricError::from)?,
        version: go.version,
    };
    Ok(event_subject(&coords))
}

pub async fn assert_widened_durable_rejected(
    harness: &FabricTestNats,
    marker: &WidenedDurable,
) -> FabricError {
    let coords = sample_event_coords();
    let err = harness
        .fabric()
        .verify_event_durable(&coords, &marker.durable)
        .await
        .expect_err("a durable widened to integration.evt.> must be rejected, not bound");
    assert!(
        matches!(err, FabricError::FilterMismatch { .. }),
        "expected FilterMismatch, got {err:?}"
    );
    err
}

pub async fn assert_missing_stream_fails_loud(bare: &BareFabricNats) -> FabricError {
    let coords = sample_event_coords();
    let err = bare.assert_missing_stream(&coords, "any-durable").await;
    assert!(
        matches!(
            err,
            FabricError::Consume {
                kind: br_util_nats_fabric::ConsumeErrorKind::NoStream,
                ..
            }
        ),
        "expected Consume(NoStream), got {err:?}"
    );
    err
}

pub async fn assert_dead_grammar_fails_loud(
    harness: &FabricTestNats,
    dead_subject: &str,
) -> PublishErrorKind {
    let payload = serde_json::json!({ "probe": "dead-grammar" });
    let bytes = serde_json::to_vec(&payload).expect("probe serializes");
    let kind = harness.publish_dead_subject(dead_subject, &bytes).await;
    assert_eq!(
        kind,
        PublishErrorKind::NoStream,
        "dead grammar must land on no fixed stream, got {kind:?}"
    );
    kind
}

pub async fn assert_no_fixed_stream_captured(
    harness: &FabricTestNats,
    dead_subject: &str,
) -> Result<()> {
    for stream_name in [INTEGRATION_CMD, INTEGRATION_EVT] {
        assert!(
            harness.raw_message_absent(stream_name, dead_subject).await,
            "no fixed stream may have captured the dead-grammar subject {dead_subject}"
        );
    }
    Ok(())
}

fn sample_event_coords() -> EventCoords {
    EventCoords {
        producer: Bc::new("identity").expect("valid bc"),
        aggregate: Aggregate::new("user").expect("valid aggregate"),
        fact: PastFact::new("created").expect("valid fact"),
        version: 1,
    }
}

pub fn exact_event_filter() -> String {
    event_subject(&sample_event_coords())
}

pub async fn widen(
    harness: FabricTestNats,
    durable_name: &str,
) -> (FabricTestNats, WidenedDurable) {
    harness
        .with_widened_durable(INTEGRATION_EVT, durable_name, "integration.evt.>")
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{FrozenCommandSubject, FrozenEventSubject};

    #[test]
    fn rust_renders_the_go_command_subject_byte_for_byte() {
        let go = FrozenCommandSubject {
            receiver: "notifier".to_string(),
            aggregate: "notification".to_string(),
            verb: "deliver".to_string(),
            version: 1,
            subject: "integration.cmd.notifier.notification.deliver.v1".to_string(),
        };
        assert_eq!(rust_command_subject(&go).unwrap(), go.subject);
    }

    #[test]
    fn rust_renders_the_go_event_subject_byte_for_byte() {
        let go = FrozenEventSubject {
            producer: "identity".to_string(),
            aggregate: "group".to_string(),
            fact: "renamed".to_string(),
            version: 3,
            subject: "integration.evt.identity.group.renamed.v3".to_string(),
        };
        assert_eq!(rust_event_subject(&go).unwrap(), go.subject);
    }

    #[test]
    fn the_exact_filter_is_a_single_concrete_coordinate() {
        let filter = exact_event_filter();
        assert_eq!(filter, "integration.evt.identity.user.created.v1");
        assert!(!filter.ends_with('>'));
    }
}
