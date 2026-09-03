use br_core_integration::{CommandCoords, EventCoords};
use br_util_nats_fabric::{INTEGRATION_CMD, INTEGRATION_EVT, command_subject, event_subject};

use super::observe::FixedStream;
use super::tuning::{DurableConfig, pull_config};
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
        self.provision_command_durable_with(coords, durable, &DurableConfig::harness())
            .await;
    }

    pub async fn provision_event_durable(&self, coords: &EventCoords, durable: &str) {
        self.provision_event_durable_with(coords, durable, &DurableConfig::harness())
            .await;
    }

    pub async fn provision_command_durable_with(
        &self,
        coords: &CommandCoords,
        durable: &str,
        config: &DurableConfig,
    ) {
        self.create_durable_with(INTEGRATION_CMD, durable, &command_subject(coords), config)
            .await;
    }

    pub async fn provision_event_durable_with(
        &self,
        coords: &EventCoords,
        durable: &str,
        config: &DurableConfig,
    ) {
        self.create_durable_with(INTEGRATION_EVT, durable, &event_subject(coords), config)
            .await;
    }

    pub async fn delete_durable(&self, stream: FixedStream, durable: &str) {
        let name = stream.name();
        self.js
            .get_stream(name)
            .await
            .unwrap_or_else(|e| panic!("get fixed stream {name} to delete durable {durable}: {e}"))
            .delete_consumer(durable)
            .await
            .unwrap_or_else(|e| panic!("delete durable {durable} on {name}: {e}"));
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
        self.create_durable_with(stream_name, durable, filter, &DurableConfig::harness())
            .await;
    }

    async fn create_durable_with(
        &self,
        stream_name: &'static str,
        durable: &str,
        filter: &str,
        config: &DurableConfig,
    ) {
        let stream = self.js.get_stream(stream_name).await.unwrap_or_else(|e| {
            panic!("fixed stream {stream_name} must exist before binding: {e}")
        });
        stream
            .create_consumer(pull_config(durable, filter, config))
            .await
            .unwrap_or_else(|e| {
                panic!("create durable {durable} on {stream_name} filtering {filter}: {e}")
            });
    }
}
