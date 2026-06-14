use async_nats::jetstream;
use br_core_integration::{
    Actor, EventMetadata, IntegrationCommand, IntegrationPublisherExt, NatsIntegrationPublisher,
    ServiceAccountId,
};
use br_core_scope::DeclareServiceScopes;
use br_scope_declaration_contract::{VERSION, command_type};
use uuid::Uuid;

use crate::error::{ConformanceError, Result};
use crate::subjects::declare_subject;

pub struct Declarer {
    js: jetstream::Context,
}

impl Declarer {
    pub fn new(js: jetstream::Context) -> Self {
        Self { js }
    }

    pub async fn declare(&self, command: DeclareServiceScopes) -> Result<Uuid> {
        self.declare_with_correlation(command, Uuid::now_v7()).await
    }

    pub async fn declare_with_correlation(
        &self,
        payload: DeclareServiceScopes,
        correlation_id: Uuid,
    ) -> Result<Uuid> {
        let command = IntegrationCommand::new(
            Uuid::now_v7(),
            command_type(),
            VERSION,
            chrono::Utc::now(),
            declaring_metadata(correlation_id),
            payload,
        );
        let subject = declare_subject()?;
        NatsIntegrationPublisher::new(self.js.clone())
            .publish_command(&subject, &command)
            .await
            .map_err(|e| ConformanceError::Publish(format!("publish to '{subject}': {e}")))?;
        Ok(correlation_id)
    }
}

pub fn declaring_metadata(correlation_id: Uuid) -> EventMetadata {
    EventMetadata::new(
        Actor::Service(ServiceAccountId::from(Uuid::now_v7())),
        correlation_id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::declare;

    #[test]
    fn command_carries_the_contract_type_and_version() {
        let correlation_id = Uuid::now_v7();
        let command = IntegrationCommand::new(
            Uuid::now_v7(),
            command_type(),
            VERSION,
            chrono::Utc::now(),
            declaring_metadata(correlation_id),
            declare("notifier", &["notifier:read"]),
        );
        assert_eq!(command.command_type, "service_scope.declare");
        assert_eq!(command.version, 1);
        assert_eq!(command.metadata.correlation_id, correlation_id);
        assert!(command.metadata.actor.is_service());
    }

    #[test]
    fn command_round_trips_through_the_real_wire() {
        let command = IntegrationCommand::new(
            Uuid::now_v7(),
            command_type(),
            VERSION,
            chrono::Utc::now(),
            declaring_metadata(Uuid::now_v7()),
            declare("notifier", &["notifier:read"]),
        );
        let json = serde_json::to_vec(&command).unwrap();
        let back: IntegrationCommand<DeclareServiceScopes> = serde_json::from_slice(&json).unwrap();
        assert_eq!(back.payload, command.payload);
    }
}
