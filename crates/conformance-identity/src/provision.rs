use std::path::{Path, PathBuf};
use std::time::Duration;

use br_test_harness::run_once;

use crate::error::{ConformanceError, Result};

const PROVISION_TIMEOUT: Duration = Duration::from_secs(30);

pub fn fabric_nats_bin() -> PathBuf {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let bin = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(profile)
        .join("fabric-nats");
    bin.canonicalize().unwrap_or(bin)
}

pub fn manifest_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

pub async fn provision(nats_url: &str, manifest: &str) -> Result<()> {
    let bin = fabric_nats_bin();
    let manifest = manifest_path(manifest);
    let output = run_once(
        &bin.to_string_lossy(),
        &[
            "provision",
            "--nats",
            nats_url,
            "--manifest",
            &manifest.to_string_lossy(),
        ],
        &[],
        PROVISION_TIMEOUT,
    )
    .await
    .map_err(|e| ConformanceError::Jetstream(format!("spawning fabric-nats provision: {e}")))?;
    if !output.status.success() {
        return Err(ConformanceError::Jetstream(format!(
            "fabric-nats provision failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}
