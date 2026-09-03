#![allow(dead_code)]

use br_core_integration::{
    Aggregate, Bc, CommandCoords, EventCoords, IntegrationCommand, IntegrationEvent, PastFact, Verb,
};
use br_util_nats_fabric::{FabricError, PublishErrorKind};
use serde_json::Value;
use uuid::Uuid;

pub fn created() -> EventCoords {
    EventCoords {
        producer: Bc::new("identity").unwrap(),
        aggregate: Aggregate::new("user").unwrap(),
        fact: PastFact::new("created").unwrap(),
        version: 1,
    }
}

pub fn renamed() -> EventCoords {
    EventCoords {
        producer: Bc::new("identity").unwrap(),
        aggregate: Aggregate::new("user").unwrap(),
        fact: PastFact::new("renamed").unwrap(),
        version: 1,
    }
}

pub fn deleted() -> EventCoords {
    EventCoords {
        producer: Bc::new("identity").unwrap(),
        aggregate: Aggregate::new("user").unwrap(),
        fact: PastFact::new("deleted").unwrap(),
        version: 1,
    }
}

pub fn deliver() -> CommandCoords {
    CommandCoords {
        receiver: Bc::new("notifier").unwrap(),
        aggregate: Aggregate::new("notification").unwrap(),
        verb: Verb::new("deliver").unwrap(),
        version: 1,
    }
}

pub fn envelope() -> IntegrationEvent<Value> {
    marked_envelope("delivery-outage")
}

pub fn marked_envelope(marker: &str) -> IntegrationEvent<Value> {
    serde_json::from_value(serde_json::json!({
        "event_id": Uuid::now_v7(),
        "event_type": "outage.probe",
        "version": 1,
        "occurred_at": "2026-09-03T00:00:00Z",
        "metadata": {
            "actor_id": Uuid::now_v7(),
            "actor_kind": "service",
            "correlation_id": Uuid::now_v7()
        },
        "payload": { "probe": marker }
    }))
    .expect("the probe envelope deserializes into the lib's IntegrationEvent")
}

pub fn marker_of(event: &IntegrationEvent<Value>) -> &str {
    event.payload["probe"]
        .as_str()
        .expect("the probe payload carries its marker")
}

pub fn command_envelope() -> IntegrationCommand<Value> {
    serde_json::from_value(serde_json::json!({
        "command_id": Uuid::now_v7(),
        "command_type": "outage.probe",
        "version": 1,
        "issued_at": "2026-09-03T00:00:00Z",
        "metadata": {
            "actor_id": Uuid::now_v7(),
            "actor_kind": "service",
            "correlation_id": Uuid::now_v7()
        },
        "payload": { "probe": "delivery-outage" }
    }))
    .expect("the probe envelope deserializes into the lib's IntegrationCommand")
}

pub fn assert_no_stream(err: FabricError, subject: &str) {
    assert!(
        matches!(
            &err,
            FabricError::Publish {
                kind: PublishErrorKind::NoStream,
                ..
            }
        ),
        "a publish on the withheld '{subject}' must fail Publish(NoStream), got {err:?}"
    );
}

pub fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return (*message).to_string();
    }
    payload
        .downcast_ref::<String>()
        .cloned()
        .unwrap_or_else(|| "<non-string panic payload>".to_string())
}
