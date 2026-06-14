use std::path::PathBuf;

use async_nats::jetstream;
use br_test_harness::{SpawnedNats, nats::connect};

use crate::build::build_subject;
use crate::capture::ConfirmationCapture;
use crate::declarer::Declarer;
use crate::error::{ConformanceError, Result};
use crate::stream::create_handshake_stream;

pub const DEFAULT_STREAM_NAME: &str = "IDENTITY";

pub struct IdentityHarness {
    nats: SpawnedNats,
    js: jetstream::Context,
    stream_name: String,
    binary: PathBuf,
}

impl IdentityHarness {
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
        let stream_name = DEFAULT_STREAM_NAME.to_string();
        create_handshake_stream(&js, &stream_name).await?;

        Ok(Self {
            nats,
            js,
            stream_name,
            binary,
        })
    }

    pub fn nats_url(&self) -> String {
        self.nats.url()
    }

    pub fn jetstream(&self) -> &jetstream::Context {
        &self.js
    }

    pub fn stream_name(&self) -> &str {
        &self.stream_name
    }

    pub fn binary(&self) -> &PathBuf {
        &self.binary
    }

    pub fn declarer(&self) -> Declarer {
        Declarer::new(self.js.clone())
    }

    pub async fn capture_confirmations(&self) -> Result<ConfirmationCapture> {
        ConfirmationCapture::start(&self.js, &self.stream_name).await
    }

    pub async fn shutdown(self) {
        self.nats.shutdown().await;
    }
}
