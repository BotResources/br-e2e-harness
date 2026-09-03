use std::path::PathBuf;

use br_test_harness::FabricTestNats;
use br_util_nats_fabric::{Fabric, KvKey, PublishedLanguagePublisher};

use crate::build::build_subject;
use crate::error::{ConformanceError, Result};
use crate::seal::{SealedSeeder, seal_key_b64, wrong_seal_key_b64};

pub struct PassportHarness {
    nats: FabricTestNats,
    binary: PathBuf,
}

impl PassportHarness {
    pub async fn start() -> Result<Self> {
        let binary = build_subject().await?;
        Self::start_with_binary(binary).await
    }

    pub async fn start_with_binary(binary: PathBuf) -> Result<Self> {
        let nats = FabricTestNats::start().await;
        let nats = nats.with_published_language().await;
        Ok(Self { nats, binary })
    }

    pub fn nats_url(&self) -> String {
        self.nats.url()
    }

    pub fn binary(&self) -> &PathBuf {
        &self.binary
    }

    pub fn fabric(&self) -> &Fabric {
        self.nats.fabric()
    }

    pub fn seeder(&self) -> SealedSeeder {
        SealedSeeder::new(self.binary.clone(), seal_key_b64())
    }

    pub fn wrong_key_seeder(&self) -> SealedSeeder {
        SealedSeeder::new(self.binary.clone(), wrong_seal_key_b64())
    }

    pub async fn pl_get_raw(&self, key: &KvKey) -> Option<Vec<u8>> {
        self.nats.pl_get_raw(key).await
    }

    pub async fn pl_put_raw(&self, key: &KvKey, bytes: &[u8]) {
        self.nats.pl_put_raw(key, bytes).await
    }

    pub async fn pl_retract(&self, key: &KvKey) -> Result<()> {
        let publisher = PublishedLanguagePublisher::<serde_json::Value>::open(self.nats.fabric())
            .await
            .map_err(|e| ConformanceError::Seed(e.to_string()))?;
        publisher
            .retract(key)
            .await
            .map_err(|e| ConformanceError::Seed(e.to_string()))
    }

    pub async fn delete_published_language(&self) {
        self.nats.delete_published_language().await
    }

    pub async fn shutdown(self) {
        self.nats.shutdown().await;
    }
}
