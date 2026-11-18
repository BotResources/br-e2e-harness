use std::path::{Path, PathBuf};
use std::time::Duration;

use br_test_harness::{run_once, workspace_bin};

use crate::containment::{contained, panic_message};
use crate::error::{ConformanceError, Result};

const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) async fn nats_server() -> Result<()> {
    match contained(run_once("nats-server", &["--version"], &[], PROBE_TIMEOUT)).await {
        Ok(Ok(output)) if output.status.success() => Ok(()),
        Ok(Ok(output)) => Err(ConformanceError::Jetstream(format!(
            "nats-server is on PATH but `--version` exited with {}",
            output.status
        ))),
        Ok(Err(e)) => Err(ConformanceError::Jetstream(format!(
            "the nats-server probe failed (the real-infra batteries need nats-server on PATH): {e}"
        ))),
        Err(panic) => Err(ConformanceError::Jetstream(format!(
            "the nats-server probe could not run (the real-infra batteries need nats-server on PATH and a spawnable `sh`): {panic}"
        ))),
    }
}

pub(crate) fn fabric_nats() -> Result<PathBuf> {
    std::panic::catch_unwind(|| workspace_bin("fabric-nats")).map_err(|panic| {
        ConformanceError::Jetstream(format!(
            "the fabric-nats CLI is not where br-test-harness looks for it: build \
             br-test-harness with its nats-fabric feature in the same target directory ({})",
            panic_message(panic)
        ))
    })
}

pub(crate) fn subject_binary(binary: &Path) -> Result<()> {
    if crate::binary::is_executable_file(binary) {
        Ok(())
    } else {
        Err(ConformanceError::InvalidInput(format!(
            "the acceptor binary {} does not exist, is not a file or is not executable",
            binary.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_scratch::scratch;

    #[test]
    fn a_missing_subject_binary_is_a_typed_input_error() {
        let scratch_dir = scratch("absent-subject");
        let absent = scratch_dir.join("identity-acceptor");
        match subject_binary(&absent) {
            Err(ConformanceError::InvalidInput(detail)) => {
                assert!(detail.contains(&absent.display().to_string()));
            }
            other => panic!("expected a typed input error: {other:?}"),
        }
        assert!(subject_binary(&std::env::current_exe().unwrap()).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn a_subject_binary_without_the_execute_bit_is_a_typed_input_error() {
        use std::os::unix::fs::PermissionsExt;
        let scratch_dir = scratch("not-executable-subject");
        let file = scratch_dir.join("identity-acceptor");
        std::fs::write(&file, b"#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        match subject_binary(&file) {
            Err(ConformanceError::InvalidInput(detail)) => {
                assert!(detail.contains("not executable"), "{detail}");
                assert!(detail.contains(&file.display().to_string()));
            }
            other => panic!("expected a typed input error: {other:?}"),
        }
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(subject_binary(&file).is_ok());
    }
}
