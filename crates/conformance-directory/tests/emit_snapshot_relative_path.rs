#[path = "common/child.rs"]
mod child;
#[path = "common/scratch.rs"]
mod scratch;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use conformance_directory::{ANCHOR_BINARY, ConformanceError, emit_snapshot};

const MODE: &str = "CONFORMANCE_DIRECTORY_RELATIVE_PATH_MODE";
const TEST: &str = "a_bare_anchor_name_is_resolved_on_absolute_path_entries_only";
const RAN: &str = "ran-from-cwd";

fn install(dir: &Path, name: &str, body: &str) {
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write a script");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
}

fn emit_marker(marker: &str) -> String {
    format!(
        "printf '%s' '{{\"meta\":{{\"key\":\"identity/_meta\",\"value\":{{\"marker\":\"{marker}\"}}}},\"users\":[],\"groups\":[]}}'"
    )
}

fn child(cwd: &Path, mode: &str, path: &std::ffi::OsStr) {
    let output = Command::new(std::env::current_exe().expect("test executable"))
        .args(["--exact", TEST, "--nocapture"])
        .current_dir(cwd)
        .env(MODE, mode)
        .env("PATH", path)
        .output()
        .expect("run the child test");
    child::assert_child_ran_one_passing_test(&output);
}

fn join(entries: &[&Path]) -> std::ffi::OsString {
    std::env::join_paths(entries).expect("join PATH")
}

#[tokio::test(flavor = "current_thread")]
async fn a_bare_anchor_name_is_resolved_on_absolute_path_entries_only() {
    let Some(mode) = std::env::var_os(MODE) else {
        let cwd = scratch::scratch("relative-path-cwd");
        let on_path = scratch::scratch("relative-path-on-path");
        install(
            &cwd,
            ANCHOR_BINARY,
            &format!("touch {RAN}\n{}", emit_marker("cwd")),
        );
        install(&on_path, ANCHOR_BINARY, &emit_marker("path"));
        let system = [Path::new("/usr/bin"), Path::new("/bin")];
        child(
            &cwd,
            "absent",
            &join(&[Path::new("."), system[0], system[1]]),
        );
        child(
            &cwd,
            "present",
            &join(&[Path::new("."), &on_path, system[0], system[1]]),
        );
        assert!(
            !cwd.join(RAN).exists(),
            "the anchor in the current directory must never run"
        );
        return;
    };

    let result = emit_snapshot(Path::new(ANCHOR_BINARY)).await;
    match mode.to_str().expect("mode") {
        "absent" => match result {
            Err(ConformanceError::AnchorBinaryUnavailable { binary, .. }) => {
                assert_eq!(binary, ANCHOR_BINARY)
            }
            other => panic!("a bare name absent from the absolute PATH entries: {other:?}"),
        },
        "present" => {
            let snapshot = result.expect("the anchor on the absolute PATH entry runs");
            assert_eq!(snapshot.meta.value["marker"], "path");
        }
        other => panic!("unknown mode {other}"),
    }
}
