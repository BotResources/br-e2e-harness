use std::any::Any;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::time::Duration;

use br_test_harness::run_once;
use futures_util::FutureExt;

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
    if binary.is_file() {
        Ok(())
    } else {
        Err(ConformanceError::InvalidInput(format!(
            "the acceptor binary {} does not exist or is not a file",
            binary.display()
        )))
    }
}

pub(crate) async fn contained<T>(
    future: impl Future<Output = T>,
) -> std::result::Result<T, String> {
    AssertUnwindSafe(future)
        .catch_unwind()
        .await
        .map_err(panic_message)
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(payload) => match payload.downcast::<&'static str>() {
            Ok(message) => (*message).to_string(),
            Err(_) => "the harness panicked with a non-string payload".to_string(),
        },
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
        std::fs::remove_dir_all(target).ok();
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
        std::fs::remove_dir_all(target).ok();
    }

    #[test]
    fn a_missing_subject_binary_is_a_typed_input_error() {
        let absent = scratch("absent-subject").join("identity-acceptor");
        match subject_binary(&absent) {
            Err(ConformanceError::InvalidInput(detail)) => {
                assert!(detail.contains(&absent.display().to_string()));
            }
            other => panic!("expected a typed input error: {other:?}"),
        }
        assert!(subject_binary(&std::env::current_exe().unwrap()).is_ok());
    }

    #[tokio::test]
    async fn a_panic_in_the_harness_is_contained_with_its_message() {
        let formatted = contained(async { panic!("harness exploded: {}", 7) }).await;
        assert_eq!(formatted.unwrap_err(), "harness exploded: 7");
        let literal = contained(async { panic!("plain literal") }).await;
        assert_eq!(literal.unwrap_err(), "plain literal");
    }

    #[tokio::test]
    async fn a_value_passes_through_containment() {
        assert_eq!(contained(async { 41 + 1 }).await.unwrap(), 42);
    }
}
