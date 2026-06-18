use std::path::PathBuf;

use br_test_harness::{BearerSeeder, FabricTestNats};

use crate::build::build_subject;
use crate::error::Result;
use crate::provision::provision;

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
        provision(&nats.url(), "bearer_tokens.toml").await?;
        let nats = nats.with_bearer_tokens().await;
        Ok(Self { nats, binary })
    }

    pub fn nats_url(&self) -> String {
        self.nats.url()
    }

    pub fn binary(&self) -> &PathBuf {
        &self.binary
    }

    pub fn seeder(&self) -> BearerSeeder {
        self.nats.bearer_seeder()
    }

    pub async fn shutdown(self) {
        self.nats.shutdown().await;
    }
}
