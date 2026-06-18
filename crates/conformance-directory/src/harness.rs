use async_nats::jetstream::kv;
use br_test_harness::FabricTestNats;
use br_util_nats_fabric::{Fabric, KV_PUBLISHED_LANGUAGE};

use crate::error::{ConformanceError, Result};

pub struct DirectoryHarness {
    nats: FabricTestNats,
    store: kv::Store,
}

impl DirectoryHarness {
    pub async fn start() -> Result<Self> {
        let nats = FabricTestNats::start()
            .await
            .with_published_language()
            .await;
        let store = nats
            .jetstream()
            .get_key_value(KV_PUBLISHED_LANGUAGE)
            .await
            .map_err(|e| {
                ConformanceError::Jetstream(format!(
                    "open published-language bucket '{KV_PUBLISHED_LANGUAGE}': {e}"
                ))
            })?;
        Ok(Self { nats, store })
    }

    pub fn nats_url(&self) -> String {
        self.nats.url()
    }

    pub fn fabric(&self) -> &Fabric {
        self.nats.fabric()
    }

    pub fn store(&self) -> &kv::Store {
        &self.store
    }

    pub async fn shutdown(self) {
        self.nats.shutdown().await;
    }
}
