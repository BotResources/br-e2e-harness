use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use conformance_directory::{ANCHOR_BINARY, emit_snapshot};

fn scratch(label: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "emit-snapshot-{label}-{}",
        uuid::Uuid::now_v7().simple()
    ));
    std::fs::create_dir_all(&dir).expect("create a scratch directory");
    dir
}

fn install(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write a script");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

fn emit_marker(marker: &str) -> String {
    format!(
        "printf '%s' '{{\"meta\":{{\"key\":\"identity/_meta\",\"value\":{{\"marker\":\"{marker}\"}}}},\"users\":[],\"groups\":[]}}'"
    )
}

async fn marker_of(binary: &Path) -> String {
    let snapshot = emit_snapshot(binary).await.expect("the script anchor runs");
    snapshot.meta.value["marker"]
        .as_str()
        .expect("marker")
        .to_string()
}

#[tokio::test(flavor = "current_thread")]
async fn a_relative_anchor_never_searches_the_current_directory_and_an_absolute_one_leads_the_child_path()
 {
    let cwd = scratch("cwd");
    let on_path = scratch("on-path");
    let absolute_dir = scratch("absolute");

    install(&cwd, ANCHOR_BINARY, &emit_marker("cwd"));
    install(&on_path, ANCHOR_BINARY, &emit_marker("path"));
    install(&absolute_dir, "sibling-tool", &emit_marker("sibling"));
    let absolute = install(&absolute_dir, ANCHOR_BINARY, "sibling-tool");
    std::fs::create_dir_all(cwd.join("bin")).expect("create bin");
    install(&cwd.join("bin"), ANCHOR_BINARY, &emit_marker("relative"));

    std::env::set_current_dir(&cwd).expect("enter the scratch cwd");
    // SAFETY: this file holds a single test, so no other thread of this process reads or writes the environment while it runs.
    unsafe {
        std::env::set_var(
            "PATH",
            std::env::join_paths([on_path.as_path(), Path::new("/usr/bin"), Path::new("/bin")])
                .expect("join PATH"),
        );
    }

    assert_eq!(marker_of(Path::new(ANCHOR_BINARY)).await, "path");
    assert_eq!(
        marker_of(Path::new("./bin").join(ANCHOR_BINARY).as_path()).await,
        "relative"
    );
    assert_eq!(marker_of(&absolute).await, "sibling");

    for dir in [cwd, on_path, absolute_dir] {
        std::fs::remove_dir_all(dir).ok();
    }
}
