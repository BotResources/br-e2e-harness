mod namespace;
mod negative;

pub use namespace::RunNamespace;
pub use negative::{BareFabricNats, WidenedDurable};

use async_nats::jetstream::{self, consumer, stream};
use br_core_integration::{CommandCoords, EventCoords};
use br_util_nats_fabric::{
    Fabric, INTEGRATION_CMD, INTEGRATION_EVT, KV_PUBLISHED_LANGUAGE, KvPrefix, command_subject,
    event_subject,
};

use crate::spawned_nats::SpawnedNats;

pub struct FabricTestNats {
    nats: SpawnedNats,
    client: async_nats::Client,
    js: jetstream::Context,
    fabric: Fabric,
    ns: RunNamespace,
}

impl FabricTestNats {
    pub async fn start() -> Self {
        let nats = SpawnedNats::start().await;
        let client = crate::nats::connect(&nats.url())
            .await
            .expect("failed to connect to the spawned NATS for the fabric harness");
        let js = jetstream::new(client.clone());

        create_fixed_stream(&js, INTEGRATION_CMD, "integration.cmd.>").await;
        create_fixed_stream(&js, INTEGRATION_EVT, "integration.evt.>").await;

        let fabric = Fabric::new(js.clone());
        let ns = RunNamespace::mint();

        Self {
            nats,
            client,
            js,
            fabric,
            ns,
        }
    }

    pub async fn with_published_language(self) -> Self {
        get_or_create_published_language(&self.js).await;
        self
    }

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

    pub fn fabric(&self) -> &Fabric {
        &self.fabric
    }

    pub fn jetstream(&self) -> &jetstream::Context {
        &self.js
    }

    pub fn client(&self) -> &async_nats::Client {
        &self.client
    }

    pub fn url(&self) -> String {
        self.nats.url()
    }

    pub fn durable(&self, logical: &str) -> String {
        self.ns.durable(logical)
    }

    pub fn key_prefix(&self) -> KvPrefix {
        self.ns.key_prefix()
    }

    pub fn correlation(&self) -> uuid::Uuid {
        self.ns.correlation()
    }

    pub async fn shutdown(self) {
        self.nats.shutdown().await;
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

async fn create_fixed_stream(js: &jetstream::Context, name: &'static str, bind: &str) {
    js.create_stream(stream::Config {
        name: name.to_string(),
        subjects: vec![bind.to_string()],
        ..Default::default()
    })
    .await
    .unwrap_or_else(|e| panic!("create fixed fabric stream {name} binding {bind}: {e}"));
}

async fn get_or_create_published_language(js: &jetstream::Context) {
    if js.get_key_value(KV_PUBLISHED_LANGUAGE).await.is_ok() {
        return;
    }
    js.create_key_value(jetstream::kv::Config {
        bucket: KV_PUBLISHED_LANGUAGE.to_string(),
        history: 8,
        ..Default::default()
    })
    .await
    .unwrap_or_else(|e| panic!("get-or-create published-language bucket: {e}"));
}
