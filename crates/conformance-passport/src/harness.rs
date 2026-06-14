use std::path::PathBuf;

use async_nats::jetstream::{self, kv};
use br_test_harness::{SpawnedNats, nats::connect};

use crate::build::build_subject;
use crate::error::{ConformanceError, Result};
use crate::seed::{BEARER_BUCKET, BearerSeeder};

pub struct PassportHarness {
    nats: SpawnedNats,
    store: kv::Store,
    binary: PathBuf,
}

impl PassportHarness {
    pub async fn start() -> Result<Self> {
        let binary = build_subject().await?;
        Self::start_with_binary(binary).await
    }

    pub async fn start_with_binary(binary: PathBuf) -> Result<Self> {
        let nats = SpawnedNats::start().await;
        let client = connect(&nats.url())
            .await
            .map_err(|e| ConformanceError::Jetstream(format!("connect: {e}")))?;
        let js = jetstream::new(client);
        let store = js
            .create_key_value(kv::Config {
                bucket: BEARER_BUCKET.to_string(),
                ..Default::default()
            })
            .await
            .map_err(|e| {
                ConformanceError::Jetstream(format!("create bucket '{BEARER_BUCKET}': {e}"))
            })?;

        Ok(Self {
            nats,
            store,
            binary,
        })
    }

    pub fn nats_url(&self) -> String {
        self.nats.url()
    }

    pub fn binary(&self) -> &PathBuf {
        &self.binary
    }

    pub fn seeder(&self) -> BearerSeeder {
        BearerSeeder::new(self.store.clone())
    }

    pub async fn shutdown(self) {
        self.nats.shutdown().await;
    }
}
