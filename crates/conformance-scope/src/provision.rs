use std::path::{Path, PathBuf};

use br_test_harness::spawn_fabric_provision;

use crate::error::{ConformanceError, Result};
use crate::prerequisite::contained;

pub fn manifest_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

pub async fn provision(nats_url: &str, manifest: &str) -> Result<()> {
    provision_with(nats_url, &manifest_path(manifest)).await
}

pub(crate) async fn provision_with(nats_url: &str, manifest: &Path) -> Result<()> {
    if !manifest.is_file() {
        return Err(ConformanceError::FabricProvisionUnavailable {
            detail: format!("the manifest {} does not exist", manifest.display()),
        });
    }
    contained(spawn_fabric_provision(nats_url, manifest))
        .await
        .map_err(|panic| ConformanceError::FabricProvisionUnavailable { detail: panic })?
        .map_err(ConformanceError::Jetstream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_scratch::scratch;

    #[tokio::test]
    async fn a_missing_manifest_is_a_typed_error_naming_it() {
        let absent = scratch("absent-manifest").join("scope_declaration.toml");
        match provision_with("nats://127.0.0.1:1", &absent).await {
            Err(error @ ConformanceError::FabricProvisionUnavailable { .. }) => {
                let message = error.to_string();
                assert!(message.contains(&absent.display().to_string()), "{message}");
                assert!(message.contains("fabric-nats"), "{message}");
            }
            other => panic!("expected a typed provisioning error: {other:?}"),
        }
    }
}
