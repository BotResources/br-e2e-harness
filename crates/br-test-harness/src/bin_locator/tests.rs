use std::path::{Path, PathBuf};

use super::*;

struct Tree(PathBuf);

impl Tree {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "br-harness-bin-locator-{}",
            uuid::Uuid::now_v7().simple()
        ));
        std::fs::create_dir_all(&root).expect("create the scratch tree");
        Tree(root)
    }

    fn file(&self, relative: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().expect("a file has a parent"))
            .expect("create the parent directories");
        std::fs::write(&path, b"").expect("write the file");
        path
    }

    fn dir(&self, relative: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::create_dir_all(&path).expect("create the directory");
        path
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn no_cargo() -> Result<CargoDirs, String> {
    Err("not asked in this test".to_owned())
}

fn cargo_says(target: PathBuf, build: PathBuf) -> impl FnOnce() -> Result<CargoDirs, String> {
    move || Ok(CargoDirs { target, build })
}

fn looked(miss: Miss) -> Vec<PathBuf> {
    match miss {
        Miss::Searched { looked, .. } => looked,
        Miss::Pinned { bin, .. } => panic!("expected a search, got a pinned miss at {bin:?}"),
    }
}

#[test]
fn the_binary_is_found_from_a_test_executable_of_either_build_dir_layout() {
    for (exe, bin) in [
        ("target/debug/deps/e2e-0a1b", "target/debug/fabric-nats"),
        (
            "target/debug/build/svc-charter/0a1b2c3d/out/e2e-0a1b2c3d",
            "target/debug/fabric-nats",
        ),
        ("target/release/deps/e2e-0a1b", "target/release/fabric-nats"),
        (
            "target/release/build/svc-charter/0a1b2c3d/out/e2e-0a1b2c3d",
            "target/release/fabric-nats",
        ),
        (
            "target/aarch64-apple-darwin/debug/deps/e2e-0a1b",
            "target/aarch64-apple-darwin/debug/fabric-nats",
        ),
        (
            "target/aarch64-apple-darwin/debug/build/svc-charter/0a1b2c3d/out/e2e-0a1b2c3d",
            "target/aarch64-apple-darwin/debug/fabric-nats",
        ),
        ("target/debug/e2e", "target/debug/fabric-nats"),
        (
            "target/debug/examples/demo",
            "target/debug/examples/fabric-nats",
        ),
    ] {
        let tree = Tree::new();
        tree.file("target/CACHEDIR.TAG");
        let exe = tree.file(exe);
        let bin = tree.file(bin);

        let found = locate("fabric-nats", &exe, None, no_cargo);

        assert_eq!(found.ok(), Some(bin), "{}", exe.display());
    }
}

#[test]
fn a_package_named_like_a_layout_directory_does_not_hide_the_binary() {
    for package in ["build", "deps", "out"] {
        let tree = Tree::new();
        tree.file("target/CACHEDIR.TAG");
        let exe = tree.file(&format!(
            "target/debug/build/{package}/0a1b2c3d/out/e2e-0a1b"
        ));
        let bin = tree.file("target/debug/fabric-nats");

        let found = locate("fabric-nats", &exe, None, no_cargo);

        assert_eq!(found.ok(), Some(bin), "{package}");
    }
}

#[test]
fn an_absent_binary_names_every_path_looked_at_and_the_test_executable() {
    let tree = Tree::new();
    tree.file("target/CACHEDIR.TAG");
    let exe = tree.file("target/debug/build/svc-charter/0a1b2c3d/out/e2e-0a1b2c3d");

    let miss = locate("fabric-nats", &exe, None, no_cargo).expect_err("nothing was built");

    let text = miss.to_string();
    for expected in [
        tree.path("target/debug/fabric-nats"),
        tree.path("target/debug/build/svc-charter/0a1b2c3d/out/fabric-nats"),
        exe.clone(),
    ] {
        assert!(text.contains(&*expected.to_string_lossy()), "{text}");
    }
    assert!(
        text.starts_with("fabric-nats must be built by the workspace"),
        "{text}"
    );
    assert!(text.contains("BR_TEST_HARNESS_BIN_DIR"), "{text}");
    assert!(
        text.contains("cargo metadata: not asked in this test"),
        "{text}"
    );
}

#[test]
fn the_search_stops_below_the_cargo_output_root() {
    for exe in [
        "ws/target/debug/deps/e2e-0a1b",
        "ws/target/debug/build/svc-charter/0a1b2c3d/out/e2e-0a1b2c3d",
    ] {
        let tree = Tree::new();
        tree.file("ws/target/CACHEDIR.TAG");
        tree.file("ws/target/fabric-nats");
        tree.file("ws/fabric-nats");
        tree.file("fabric-nats");
        let exe = tree.file(exe);

        let miss = locate("fabric-nats", &exe, None, no_cargo).expect_err("only strays exist");

        let looked = looked(miss);
        assert_eq!(
            looked.last(),
            Some(&tree.path("ws/target/debug/fabric-nats"))
        );
        assert!(
            looked
                .iter()
                .all(|path| path.starts_with(tree.path("ws/target/debug")))
        );
    }
}

#[test]
fn without_an_output_root_marker_the_search_stops_four_directories_above_the_executable() {
    let tree = Tree::new();
    let exe = tree.file("a/b/c/d/e/out/e2e-0a1b2c3d");
    tree.file("a/fabric-nats");
    tree.file("fabric-nats");

    let miss = locate("fabric-nats", &exe, None, no_cargo).expect_err("the strays are too far up");

    assert_eq!(
        looked(miss),
        ["a/b/c/d/e/out", "a/b/c/d/e", "a/b/c/d", "a/b/c", "a/b"]
            .map(|dir| tree.path(dir).join("fabric-nats"))
    );
}

#[test]
fn a_directory_named_like_the_binary_is_not_the_binary() {
    let tree = Tree::new();
    tree.file("target/CACHEDIR.TAG");
    let exe = tree.file("target/debug/build/svc-charter/0a1b2c3d/out/e2e-0a1b2c3d");
    tree.dir("target/debug/build/svc-charter/fabric-nats");
    let bin = tree.file("target/debug/fabric-nats");

    let found = locate("fabric-nats", &exe, None, no_cargo);

    assert_eq!(found.ok(), Some(bin));
}

#[test]
fn a_separate_build_dir_is_bridged_to_the_target_dir_cargo_reports() {
    for (exe, bin) in [
        (
            "cache/build/debug/build/svc-charter/0a1b2c3d/out/e2e-0a1b2c3d",
            "ws/target/debug/fabric-nats",
        ),
        (
            "cache/build/aarch64-apple-darwin/release/build/svc-charter/0a1b2c3d/out/e2e-0a1b2c3d",
            "ws/target/aarch64-apple-darwin/release/fabric-nats",
        ),
        (
            "cache/build/debug/deps/e2e-0a1b",
            "ws/target/debug/fabric-nats",
        ),
    ] {
        let tree = Tree::new();
        tree.file("cache/build/CACHEDIR.TAG");
        tree.file("ws/target/CACHEDIR.TAG");
        let exe = tree.file(exe);
        let bin = tree.file(bin);
        let cargo = cargo_says(tree.path("ws/target"), tree.path("cache/build"));

        let found = locate("fabric-nats", &exe, None, cargo);

        assert_eq!(found.ok(), Some(bin), "{}", exe.display());
    }
}

#[test]
fn a_separate_build_dir_never_reads_the_target_dir_root_itself() {
    let tree = Tree::new();
    let exe = tree.file("cache/build/debug/build/svc-charter/0a1b2c3d/out/e2e-0a1b2c3d");
    tree.file("ws/target/fabric-nats");
    let cargo = cargo_says(tree.path("ws/target"), tree.path("cache/build"));

    let miss = locate("fabric-nats", &exe, None, cargo).expect_err("only a stray exists");

    let text = miss.to_string();
    let looked = looked(miss);
    assert!(looked.contains(&tree.path("ws/target/debug/fabric-nats")));
    assert!(!looked.contains(&tree.path("ws/target/fabric-nats")));
    assert!(text.contains("cargo metadata: target-dir"), "{text}");
    assert!(text.contains("build-dir"), "{text}");
}

#[test]
fn a_test_executable_outside_the_reported_build_dir_adds_no_path() {
    assert_eq!(
        rerooted(
            Path::new("/elsewhere/debug/deps/e2e-0a1b"),
            "fabric-nats",
            Path::new("/ws/target"),
            Path::new("/cache/build"),
        ),
        Vec::<PathBuf>::new()
    );
}

#[test]
fn a_pinned_directory_is_the_only_place_read() {
    let tree = Tree::new();
    tree.file("target/CACHEDIR.TAG");
    let exe = tree.file("target/debug/deps/e2e-0a1b");
    tree.file("target/debug/fabric-nats");
    let pinned = tree.file("bin/fabric-nats");

    let found = locate("fabric-nats", &exe, Some(&tree.path("bin")), no_cargo);
    assert_eq!(found.ok(), Some(pinned));

    let miss = locate("fabric-nats", &exe, Some(&tree.path("empty")), no_cargo)
        .expect_err("the pinned directory holds nothing");
    let text = miss.to_string();
    assert!(
        text.contains(&*tree.path("empty/fabric-nats").to_string_lossy()),
        "{text}"
    );
    assert!(text.contains("BR_TEST_HARNESS_BIN_DIR"), "{text}");
    assert!(matches!(miss, Miss::Pinned { .. }));
}

#[test]
fn cargo_metadata_gives_the_target_dir_and_the_build_dir() {
    let both = br#"{"packages":[],"target_directory":"/ws/target","build_directory":"/cache/build","version":1}"#;
    assert_eq!(
        parse_cargo_dirs(both),
        Ok(CargoDirs {
            target: "/ws/target".into(),
            build: "/cache/build".into(),
        })
    );

    let before_build_dir = br#"{"packages":[],"target_directory":"/ws/target","version":1}"#;
    assert_eq!(
        parse_cargo_dirs(before_build_dir),
        Ok(CargoDirs {
            target: "/ws/target".into(),
            build: "/ws/target".into(),
        })
    );
}

#[test]
fn unusable_cargo_metadata_is_an_error_not_a_guess() {
    assert!(
        parse_cargo_dirs(b"error: could not find `Cargo.toml`")
            .expect_err("not JSON")
            .starts_with("output is not JSON")
    );
    assert_eq!(
        parse_cargo_dirs(br#"{"packages":[]}"#),
        Err("output has no target_directory".to_owned())
    );
}
