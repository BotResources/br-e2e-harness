use br_test_harness::FabricTestNats;
use br_util_nats_fabric::Fabric;

use crate::containment::contained;
use crate::error::{ConformanceError, Result};
use crate::pg::ConsumerDb;
use crate::prerequisite;
use crate::provision::provision;

pub struct DirectoryHarness {
    nats: FabricTestNats,
}

impl DirectoryHarness {
    pub async fn start() -> Result<Self> {
        prerequisite::nats_server().await?;
        prerequisite::fabric_nats()?;
        let nats = contained(FabricTestNats::start())
            .await
            .map_err(|e| ConformanceError::Jetstream(format!("start the throwaway NATS: {e}")))?;
        if let Err(e) = provision(&nats.url(), "published_language.toml").await {
            nats.shutdown().await;
            return Err(e);
        }
        match contained(nats.published_language_present()).await {
            Ok(true) => Ok(Self { nats }),
            Ok(false) => {
                nats.shutdown().await;
                Err(ConformanceError::Jetstream(
                    "the PUBLISHED_LANGUAGE bucket is absent after provisioning".to_string(),
                ))
            }
            Err(e) => {
                nats.shutdown().await;
                Err(ConformanceError::Jetstream(format!(
                    "probe the PUBLISHED_LANGUAGE bucket after provisioning: {e}"
                )))
            }
        }
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

pub(crate) struct ConsumerEnv {
    pub(crate) harness: DirectoryHarness,
    pub(crate) db: ConsumerDb,
}

impl ConsumerEnv {
    pub(crate) async fn start() -> Result<Self> {
        prerequisite::nats_server().await?;
        prerequisite::fabric_nats()?;
        prerequisite::postgres().await?;
        let harness = DirectoryHarness::start().await?;
        match ConsumerDb::provision().await {
            Ok(db) => Ok(Self { harness, db }),
            Err(e) => {
                harness.shutdown().await;
                Err(e)
            }
        }
    }

    pub(crate) async fn shutdown(self) {
        self.db.cleanup().await;
        self.harness.shutdown().await;
    }
}
