#![cfg(feature = "nats-fabric")]

use std::path::{Path, PathBuf};

use br_test_harness::workspace_bin;

fn built_by_cargo() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_fabric-nats"))
        .canonicalize()
        .expect("cargo built fabric-nats for this test")
}

#[test]
fn workspace_bin_finds_the_binary_cargo_built_for_this_test() {
    let found = workspace_bin("fabric-nats");

    assert_eq!(
        found.canonicalize().expect("the path found exists"),
        built_by_cargo()
    );
}

#[test]
fn workspace_bin_panics_naming_where_an_absent_binary_was_expected() {
    let expected = built_by_cargo().with_file_name("no-such-binary");

    let panic = std::panic::catch_unwind(|| workspace_bin("no-such-binary"))
        .expect_err("no such binary is built");

    let text = panic
        .downcast_ref::<String>()
        .expect("the panic carries its text");
    assert!(
        text.starts_with("no-such-binary must be built by the workspace"),
        "{text}"
    );
    let looked: Vec<PathBuf> = text
        .lines()
        .filter_map(|line| line.strip_prefix("  "))
        .map(PathBuf::from)
        .collect();
    assert!(
        looked
            .iter()
            .any(|path| path.parent().and_then(|dir| dir.canonicalize().ok())
                == expected.parent().map(Path::to_path_buf)),
        "{text}"
    );
}
