use async_nats::jetstream;
use br_util_nats_fabric::{INTEGRATION_CMD, INTEGRATION_EVT, KV_PUBLISHED_LANGUAGE};

use crate::nats::connect;
use crate::spawned_nats::SpawnedNats;

pub struct WidenedDurable {
    pub stream: &'static str,
    pub durable: String,
}

pub struct BareFabricNats {
    nats: SpawnedNats,
    js: jetstream::Context,
}

impl BareFabricNats {
    pub async fn without_fixed_streams() -> Self {
        let nats = SpawnedNats::start().await;
        let client = connect(&nats.url())
            .await
            .expect("connect to the bare spawned NATS");
        let js = jetstream::new(client);
        Self { nats, js }
    }

    pub async fn with_only_command_stream() -> Self {
        let this = Self::without_fixed_streams().await;
        this.create_stream(INTEGRATION_CMD, "integration.cmd.>")
            .await;
        this
    }

    pub async fn with_only_event_stream() -> Self {
        let this = Self::without_fixed_streams().await;
        this.create_stream(INTEGRATION_EVT, "integration.evt.>")
            .await;
        this
    }

    pub fn jetstream(&self) -> &jetstream::Context {
        &self.js
    }

    pub fn url(&self) -> String {
        self.nats.url()
    }

    pub async fn published_language_absent(&self) -> bool {
        self.js.get_key_value(KV_PUBLISHED_LANGUAGE).await.is_err()
    }

    pub async fn shutdown(self) {
        self.nats.shutdown().await;
    }

    async fn create_stream(&self, name: &'static str, bind: &str) {
        self.js
            .create_stream(jetstream::stream::Config {
                name: name.to_string(),
                subjects: vec![bind.to_string()],
                ..Default::default()
            })
            .await
            .unwrap_or_else(|e| panic!("create stream {name}: {e}"));
    }
}
