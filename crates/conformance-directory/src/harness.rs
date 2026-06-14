use async_nats::jetstream::{self, kv};
use br_test_harness::{SpawnedNats, nats::connect};

use crate::error::{ConformanceError, Result};

pub const DEFAULT_DIRECTORY_BUCKET: &str = "identity_directory";

pub struct DirectoryHarness {
    nats: SpawnedNats,
    store: kv::Store,
}

impl DirectoryHarness {
    pub async fn start() -> Result<Self> {
        let nats = SpawnedNats::start().await;
        let client = connect(&nats.url())
            .await
            .map_err(|e| ConformanceError::Jetstream(format!("connect: {e}")))?;
        let js = jetstream::new(client);
        let store = js
            .create_key_value(kv::Config {
                bucket: DEFAULT_DIRECTORY_BUCKET.to_string(),
                history: 1,
                ..Default::default()
            })
            .await
            .map_err(|e| {
                ConformanceError::Jetstream(format!(
                    "create bucket '{DEFAULT_DIRECTORY_BUCKET}': {e}"
                ))
            })?;
        Ok(Self { nats, store })
    }

    pub fn nats_url(&self) -> String {
        self.nats.url()
    }

    pub fn store(&self) -> &kv::Store {
        &self.store
    }

    pub async fn shutdown(self) {
        self.nats.shutdown().await;
    }
}
