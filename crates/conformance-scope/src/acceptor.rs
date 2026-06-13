use async_nats::jetstream;
use br_core_integration::{Actor, EventMetadata, IntegrationEvent, UserId};
use br_core_scope::{
    ScopeDeclarationError, ServiceKey, ServiceScopesAccepted, ServiceScopesRejected,
};
use uuid::Uuid;

use crate::error::{ConformanceError, Result};
use crate::subjects::{ACCEPTED_SUBJECT, REJECTED_SUBJECT};

pub async fn accept(
    js: &jetstream::Context,
    service: &ServiceKey,
    correlation_id: Uuid,
) -> Result<()> {
    let event = IntegrationEvent::new(
        Uuid::now_v7(),
        "service_scope.accepted",
        1,
        chrono::Utc::now(),
        reply_metadata(correlation_id),
        ServiceScopesAccepted::new(service.clone()),
    );
    publish(js, ACCEPTED_SUBJECT, &event).await
}

pub async fn reject(
    js: &jetstream::Context,
    service: &ServiceKey,
    reason: ScopeDeclarationError,
    correlation_id: Uuid,
) -> Result<()> {
    let event = IntegrationEvent::new(
        Uuid::now_v7(),
        "service_scope.rejected",
        1,
        chrono::Utc::now(),
        reply_metadata(correlation_id),
        ServiceScopesRejected::new(service.clone(), reason),
    );
    publish(js, REJECTED_SUBJECT, &event).await
}

fn reply_metadata(correlation_id: Uuid) -> EventMetadata {
    EventMetadata::new(Actor::Human(UserId::from(Uuid::now_v7())), correlation_id)
}

async fn publish<T: serde::Serialize>(
    js: &jetstream::Context,
    subject: &str,
    event: &IntegrationEvent<T>,
) -> Result<()> {
    let bytes = serde_json::to_vec(event).map_err(|e| ConformanceError::Publish(e.to_string()))?;
    let ack = js
        .publish(subject.to_string(), bytes.into())
        .await
        .map_err(|e| ConformanceError::Publish(format!("publish to '{subject}': {e}")))?;
    ack.await
        .map_err(|e| ConformanceError::Publish(format!("ack from '{subject}': {e}")))?;
    Ok(())
}
