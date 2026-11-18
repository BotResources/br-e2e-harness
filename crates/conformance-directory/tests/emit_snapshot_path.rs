#[path = "common/child.rs"]
mod child;
#[path = "common/scratch.rs"]
mod scratch;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use conformance_directory::{ANCHOR_BINARY, emit_snapshot};

const CHILD: &str = "CONFORMANCE_DIRECTORY_EMIT_SNAPSHOT_PATH_CHILD";
const TEST: &str = "a_relative_anchor_never_searches_the_current_directory_and_an_absolute_one_leads_the_child_path";

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
    if let Some(absolute) = std::env::var_os(CHILD) {
        assert_eq!(marker_of(Path::new(ANCHOR_BINARY)).await, "path");
        assert_eq!(
            marker_of(Path::new("./bin").join(ANCHOR_BINARY).as_path()).await,
            "relative"
        );
        assert_eq!(marker_of(Path::new(&absolute)).await, "sibling");
        return;
    }

    let cwd = scratch::scratch("emit-snapshot-cwd");
    let on_path = scratch::scratch("emit-snapshot-on-path");
    let absolute_dir = scratch::scratch("emit-snapshot-absolute");

    install(&cwd, ANCHOR_BINARY, &emit_marker("cwd"));
    install(&on_path, ANCHOR_BINARY, &emit_marker("path"));
    install(&absolute_dir, "sibling-tool", &emit_marker("sibling"));
    let absolute = install(&absolute_dir, ANCHOR_BINARY, "sibling-tool");
    std::fs::create_dir_all(cwd.join("bin")).expect("create bin");
    install(&cwd.join("bin"), ANCHOR_BINARY, &emit_marker("relative"));

    let path = std::env::join_paths([&*on_path, Path::new("/usr/bin"), Path::new("/bin")])
        .expect("join PATH");
    let output = Command::new(std::env::current_exe().expect("test executable"))
        .args(["--exact", TEST, "--nocapture"])
        .current_dir(&*cwd)
        .env(CHILD, absolute.as_os_str())
        .env("PATH", path)
        .output()
        .expect("run the child test");
    child::assert_child_ran_one_passing_test(&output);
}
