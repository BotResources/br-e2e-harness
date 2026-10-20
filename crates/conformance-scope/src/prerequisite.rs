use std::any::Any;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::path::Path;

use futures_util::FutureExt;

use crate::binary::is_executable_file;
use crate::error::{ConformanceError, Result};

pub(crate) fn subject_binary(binary: &Path) -> Result<()> {
    if is_executable_file(binary) {
        Ok(())
    } else {
        Err(ConformanceError::InvalidInput(format!(
            "the subject binary {} does not exist, is not a file or is not executable",
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

pub(crate) fn panic_message(payload: Box<dyn Any + Send>) -> String {
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
    fn a_missing_subject_binary_is_a_typed_input_error() {
        let absent = scratch("absent-subject").join("scope-service");
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
    fn a_subject_binary_without_the_execute_bit_is_refused_naming_it() {
        use std::os::unix::fs::PermissionsExt;

        let path = scratch("non-executable-subject").join("scope-service");
        std::fs::write(&path, b"#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        match subject_binary(&path) {
            Err(ConformanceError::InvalidInput(detail)) => {
                assert!(detail.contains(&path.display().to_string()), "{detail}");
                assert!(detail.contains("not executable"), "{detail}");
            }
            other => panic!("expected a typed input error: {other:?}"),
        }
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(subject_binary(&path).is_ok());
    }

    #[tokio::test]
    async fn a_panic_is_contained_with_its_message() {
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
