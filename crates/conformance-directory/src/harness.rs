use br_test_harness::FabricTestNats;
use br_util_nats_fabric::Fabric;

use crate::error::Result;
use crate::provision::provision;

pub struct DirectoryHarness {
    nats: FabricTestNats,
}

impl DirectoryHarness {
    pub async fn start() -> Result<Self> {
        let nats = FabricTestNats::start().await;
        provision(&nats.url(), "published_language.toml").await?;
        Ok(Self { nats })
    }

    pub fn nats(&self) -> &FabricTestNats {
        &self.nats
    }

    pub fn nats_url(&self) -> String {
        self.nats.url()
    }

    pub fn fabric(&self) -> &Fabric {
        self.nats.fabric()
    }

    pub async fn shutdown(self) {
        self.nats.shutdown().await;
    }
}
