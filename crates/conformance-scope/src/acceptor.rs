use async_nats::jetstream;
use br_core_integration::{Actor, EventMetadata, IntegrationEvent, UserId};
use br_core_scope::{
    ScopeDeclarationError, ServiceKey, ServiceScopesAccepted, ServiceScopesRejected,
};
use br_scope_declaration_contract::{
    ACCEPTED, REJECTED, accepted_event_coords, event_type, rejected_event_coords,
};
use br_util_nats_fabric::{EventCoords, Fabric};
use uuid::Uuid;

use crate::error::{ConformanceError, Result};

pub async fn accept(
    js: &jetstream::Context,
    service: &ServiceKey,
    correlation_id: Uuid,
) -> Result<()> {
    let event = IntegrationEvent::new(
        Uuid::now_v7(),
        event_type(ACCEPTED),
        1,
        chrono::Utc::now(),
        reply_metadata(correlation_id),
        ServiceScopesAccepted::new(service.clone()),
    );
    publish(js, accepted_event_coords()?, &event).await
}

pub async fn reject(
    js: &jetstream::Context,
    service: &ServiceKey,
    reason: ScopeDeclarationError,
    correlation_id: Uuid,
) -> Result<()> {
    let event = IntegrationEvent::new(
        Uuid::now_v7(),
        event_type(REJECTED),
        1,
        chrono::Utc::now(),
        reply_metadata(correlation_id),
        ServiceScopesRejected::new(service.clone(), reason),
    );
    publish(js, rejected_event_coords()?, &event).await
}

fn reply_metadata(correlation_id: Uuid) -> EventMetadata {
    EventMetadata::new(Actor::Human(UserId::from(Uuid::now_v7())), correlation_id)
}

async fn publish<T: serde::Serialize + Send + Sync>(
    js: &jetstream::Context,
    coords: EventCoords,
    event: &IntegrationEvent<T>,
) -> Result<()> {
    Fabric::new(js.clone())
        .publish_event(&coords, event)
        .await
        .map_err(|e| ConformanceError::Publish(format!("publish event {coords:?}: {e}")))
}
