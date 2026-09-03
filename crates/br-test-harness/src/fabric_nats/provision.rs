use async_nats::jetstream::consumer;
use br_core_integration::{CommandCoords, EventCoords};
use br_util_nats_fabric::{INTEGRATION_CMD, INTEGRATION_EVT, command_subject, event_subject};

use super::{FabricTestNats, WidenedDurable};

impl FabricTestNats {
    pub async fn with_command_durable(self, coords: &CommandCoords, durable_name: &str) -> Self {
        let subject = command_subject(coords);
        self.create_durable(INTEGRATION_CMD, &self.ns.durable(durable_name), &subject)
            .await;
        self
    }

    pub async fn with_event_durable(self, coords: &EventCoords, durable_name: &str) -> Self {
        let subject = event_subject(coords);
        self.create_durable(INTEGRATION_EVT, &self.ns.durable(durable_name), &subject)
            .await;
        self
    }

    pub async fn provision_command_durable(&self, coords: &CommandCoords, durable: &str) {
        self.create_durable(INTEGRATION_CMD, durable, &command_subject(coords))
            .await;
    }

    pub async fn provision_event_durable(&self, coords: &EventCoords, durable: &str) {
        self.create_durable(INTEGRATION_EVT, durable, &event_subject(coords))
            .await;
    }

    pub async fn with_widened_durable(
        self,
        stream_name: &'static str,
        durable_name: &str,
        widened_filter: &str,
    ) -> (Self, WidenedDurable) {
        let durable = self.ns.durable(durable_name);
        self.create_durable(stream_name, &durable, widened_filter)
            .await;
        let marker = WidenedDurable {
            stream: stream_name,
            durable,
        };
        (self, marker)
    }

    async fn create_durable(&self, stream_name: &'static str, durable: &str, filter: &str) {
        let stream = self.js.get_stream(stream_name).await.unwrap_or_else(|e| {
            panic!("fixed stream {stream_name} must exist before binding: {e}")
        });
        stream
            .create_consumer(consumer::pull::Config {
                durable_name: Some(durable.to_string()),
                filter_subjects: vec![filter.to_string()],
                ack_policy: consumer::AckPolicy::Explicit,
                ack_wait: std::time::Duration::from_secs(2),
                ..Default::default()
            })
            .await
            .unwrap_or_else(|e| {
                panic!("create durable {durable} on {stream_name} filtering {filter}: {e}")
            });
    }
}
