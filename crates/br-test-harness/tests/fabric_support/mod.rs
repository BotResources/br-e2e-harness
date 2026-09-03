#![cfg(feature = "nats-fabric")]

use br_core_integration::{Actor, UserId};
use br_util_nats_fabric::{EventMetadata, IntegrationCommand, IntegrationEvent};
use uuid::Uuid;

fn metadata() -> EventMetadata {
    EventMetadata::new(Actor::Human(UserId::from(Uuid::now_v7())), Uuid::now_v7())
}

pub fn command(kind: &str) -> IntegrationCommand<serde_json::Value> {
    IntegrationCommand::new(
        Uuid::now_v7(),
        kind,
        1,
        chrono::Utc::now(),
        metadata(),
        serde_json::json!({ "kind": kind }),
    )
}

pub fn event(fact: &str) -> IntegrationEvent<serde_json::Value> {
    IntegrationEvent::new(
        Uuid::now_v7(),
        fact,
        1,
        chrono::Utc::now(),
        metadata(),
        serde_json::json!({ "fact": fact }),
    )
}
