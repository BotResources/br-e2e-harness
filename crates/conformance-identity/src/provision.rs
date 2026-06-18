use std::path::{Path, PathBuf};

use br_test_harness::spawn_fabric_provision;

use crate::error::{ConformanceError, Result};

pub fn manifest_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

pub async fn provision(nats_url: &str, manifest: &str) -> Result<()> {
    spawn_fabric_provision(nats_url, &manifest_path(manifest))
        .await
        .map_err(ConformanceError::Jetstream)
}
