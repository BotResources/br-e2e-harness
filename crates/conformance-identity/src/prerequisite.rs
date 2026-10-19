use std::path::{Path, PathBuf};
use std::time::Duration;

use br_test_harness::run_once;

use crate::error::{ConformanceError, Result};

const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) async fn nats_server() -> Result<()> {
    match run_once("nats-server", &["--version"], &[], PROBE_TIMEOUT).await {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(ConformanceError::Jetstream(format!(
            "nats-server is on PATH but `--version` exited with {}",
            output.status
        ))),
        Err(e) => Err(ConformanceError::Jetstream(format!(
            "nats-server is unavailable on PATH (install it to run the real-infra batteries): {e}"
        ))),
    }
}

pub(crate) fn fabric_nats() -> Result<PathBuf> {
    let exe = std::env::current_exe().map_err(|e| {
        ConformanceError::Jetstream(format!(
            "cannot locate the running executable to find the fabric-nats CLI: {e}"
        ))
    })?;
    fabric_nats_beside(&exe)
}

pub(crate) fn fabric_nats_beside(exe: &Path) -> Result<PathBuf> {
    let mut dir = exe.to_path_buf();
    dir.pop();
    if dir.ends_with("deps") {
        dir.pop();
    }
    let binary = dir.join("fabric-nats");
    if binary.exists() {
        Ok(binary)
    } else {
        Err(ConformanceError::Jetstream(format!(
            "the fabric-nats CLI is not built at {}: build br-test-harness with its nats-fabric \
             feature in the same target directory",
            binary.display()
        )))
    }
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
    fn a_missing_fabric_nats_binary_is_a_typed_error() {
        let target = scratch("no-fabric-nats");
        let exe = target.join("deps").join("conformance-test");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        match fabric_nats_beside(&exe) {
            Err(ConformanceError::Jetstream(detail)) => assert!(detail.contains("fabric-nats")),
            other => panic!("expected a typed fabric-nats error: {other:?}"),
        }
    }

    #[test]
    fn fabric_nats_is_found_beside_the_deps_directory() {
        let target = scratch("with-fabric-nats");
        let exe = target.join("deps").join("conformance-test");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(target.join("fabric-nats"), b"").unwrap();
        assert_eq!(
            fabric_nats_beside(&exe).unwrap(),
            target.join("fabric-nats")
        );
    }

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
