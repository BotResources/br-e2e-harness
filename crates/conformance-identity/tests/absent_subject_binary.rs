use std::process::Command;

mod common;
mod scratch;

use conformance_identity::{ConformanceError, IdentityHarness, SUBJECT_BINARY, locate_subject};

const CHILD: &str = "CONFORMANCE_IDENTITY_NO_SUBJECT_BINARY_CHILD";
const TEST: &str = "a_missing_subject_binary_is_a_typed_error_naming_it_and_nothing_is_built";

#[tokio::test(flavor = "current_thread")]
async fn a_missing_subject_binary_is_a_typed_error_naming_it_and_nothing_is_built() {
    if std::env::var_os(CHILD).is_none() {
        let empty = scratch::scratch("no-subject");
        let output = Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, "1")
            .env("PATH", &*empty)
            .env_remove("XDG_CACHE_HOME")
            .env_remove("HOME")
            .output()
            .expect("run the child test");
        common::assert_child_ran_one_passing_test(&output);
        return;
    }

    match locate_subject().await {
        Err(error @ ConformanceError::SubjectBinaryUnavailable { .. }) => {
            let message = error.to_string();
            assert!(message.contains(SUBJECT_BINARY), "{message}");
            assert!(message.contains("PATH"), "{message}");
        }
        other => panic!("locate_subject must be a typed error: {other:?}"),
    }
    match IdentityHarness::start().await {
        Err(ConformanceError::SubjectBinaryUnavailable { binary, .. }) => {
            assert_eq!(binary, SUBJECT_BINARY);
        }
        Err(other) => panic!("start must report the missing binary: {other:?}"),
        Ok(_) => panic!("start must not succeed without the binary"),
    }
}
