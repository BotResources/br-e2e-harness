#[path = "common/scratch.rs"]
mod scratch;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use conformance_directory::{ANCHOR_BINARY, ConformanceError, emit_snapshot};

async fn assert_unavailable(what: &str, path: &Path) {
    match emit_snapshot(path).await {
        Err(ConformanceError::AnchorBinaryUnavailable { binary, detail }) => {
            assert_eq!(binary, ANCHOR_BINARY, "{what}");
            assert!(
                detail.contains(&path.display().to_string()),
                "{what}: {detail}"
            );
            assert!(
                detail.contains("not an executable file"),
                "{what}: {detail}"
            );
        }
        other => panic!("{what}: expected AnchorBinaryUnavailable, got {other:?}"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn an_explicit_anchor_path_that_cannot_run_is_a_typed_unavailable_error() {
    let dir = scratch::scratch("emit-snapshot-not-executable");

    assert_unavailable("a missing file", &dir.join(ANCHOR_BINARY)).await;

    let plain = dir.join("plain");
    std::fs::write(&plain, b"#!/bin/sh\nexit 0\n").expect("write a script");
    std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o644)).expect("chmod");
    assert_unavailable("a file without the execute bit", &plain).await;

    let directory = dir.join("a-directory");
    std::fs::create_dir_all(&directory).expect("create a directory");
    assert_unavailable("a directory", &directory).await;
}
