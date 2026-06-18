use std::path::PathBuf;

use async_nats::jetstream;
use br_test_harness::FabricTestNats;
use br_util_nats_fabric::{INTEGRATION_CMD, INTEGRATION_EVT};

use crate::build::build_subject;
use crate::capture::DeclareCapture;
use crate::error::Result;

pub const COMMAND_STREAM_NAME: &str = INTEGRATION_CMD;
pub const EVENT_STREAM_NAME: &str = INTEGRATION_EVT;

pub struct ScopeHarness {
    nats: FabricTestNats,
    binary: PathBuf,
}

impl ScopeHarness {
    pub async fn start() -> Result<Self> {
        let binary = build_subject().await?;
        Self::start_with_binary(binary).await
    }

    pub async fn start_with_binary(binary: PathBuf) -> Result<Self> {
        let nats = FabricTestNats::start().await;
        Ok(Self { nats, binary })
    }

    pub fn nats_url(&self) -> String {
        self.nats.url()
    }

    pub fn jetstream(&self) -> &jetstream::Context {
        self.nats.jetstream()
    }

    pub fn stream_name(&self) -> &str {
        COMMAND_STREAM_NAME
    }

    pub fn event_stream_name(&self) -> &str {
        EVENT_STREAM_NAME
    }

    pub fn binary(&self) -> &PathBuf {
        &self.binary
    }

    pub async fn capture_declares(&self) -> Result<DeclareCapture> {
        DeclareCapture::start(self.jetstream(), COMMAND_STREAM_NAME).await
    }

    pub async fn shutdown(self) {
        self.nats.shutdown().await;
    }
}
