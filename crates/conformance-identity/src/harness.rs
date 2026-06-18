use std::path::PathBuf;

use br_test_harness::FabricTestNats;
use br_util_nats_fabric::{Fabric, INTEGRATION_CMD, INTEGRATION_EVT};

use crate::build::build_subject;
use crate::capture::ConfirmationCapture;
use crate::declarer::Declarer;
use crate::error::Result;

pub const COMMAND_STREAM_NAME: &str = INTEGRATION_CMD;
pub const EVENT_STREAM_NAME: &str = INTEGRATION_EVT;

pub struct IdentityHarness {
    fabric_nats: FabricTestNats,
    binary: PathBuf,
}

impl IdentityHarness {
    pub async fn start() -> Result<Self> {
        let binary = build_subject().await?;
        Self::start_with_binary(binary).await
    }

    pub async fn start_with_binary(binary: PathBuf) -> Result<Self> {
        let fabric_nats = FabricTestNats::start().await;
        Ok(Self {
            fabric_nats,
            binary,
        })
    }

    pub fn nats_url(&self) -> String {
        self.fabric_nats.url()
    }

    pub fn fabric(&self) -> &Fabric {
        self.fabric_nats.fabric()
    }

    pub fn command_stream_name(&self) -> &str {
        COMMAND_STREAM_NAME
    }

    pub fn event_stream_name(&self) -> &str {
        EVENT_STREAM_NAME
    }

    pub fn binary(&self) -> &PathBuf {
        &self.binary
    }

    pub fn declarer(&self) -> Declarer {
        Declarer::new(self.fabric_nats.fabric_owned())
    }

    pub async fn capture_confirmations(&self) -> Result<ConfirmationCapture> {
        ConfirmationCapture::start(&self.fabric_nats).await
    }

    pub async fn shutdown(self) {
        self.fabric_nats.shutdown().await;
    }
}
