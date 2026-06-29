use std::path::PathBuf;

use br_test_harness::FabricTestNats;
use br_util_nats_fabric::{Fabric, KvKey};

use crate::build::build_subject;
use crate::error::Result;
use crate::provision::provision;
use crate::seal::{SealedSeeder, seal_key, wrong_seal_key};

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
        provision(&nats.url(), "published_language.toml").await?;
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

    pub async fn seeder(&self) -> Result<SealedSeeder> {
        SealedSeeder::open(self.nats.fabric(), seal_key()).await
    }

    pub async fn wrong_key_seeder(&self) -> Result<SealedSeeder> {
        SealedSeeder::open(self.nats.fabric(), wrong_seal_key()).await
    }

    pub async fn pl_get_raw(&self, key: &KvKey) -> Option<Vec<u8>> {
        self.nats.pl_get_raw(key).await
    }

    pub async fn pl_put_raw(&self, key: &KvKey, bytes: &[u8]) {
        self.nats.pl_put_raw(key, bytes).await
    }

    pub async fn delete_published_language(&self) {
        self.nats.delete_published_language().await
    }

    pub async fn shutdown(self) {
        self.nats.shutdown().await;
    }
}
