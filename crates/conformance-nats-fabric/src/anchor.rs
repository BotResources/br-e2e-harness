use std::path::Path;
use std::time::Duration;

use br_test_harness::run_once;

use crate::binary::{
    ANCHOR_BINARY, find_in_path, locate_anchor, path_with_directory_first, unavailable,
};
use crate::error::{ConformanceError, Result};
use crate::wire::FrozenWire;

const ANCHOR_TIMEOUT: Duration = Duration::from_secs(60);

pub(crate) async fn render_wire(binary: &Path) -> Result<FrozenWire> {
    let inherited = std::env::var_os("PATH");
    let program = match binary.parent() {
        Some(parent) if parent.as_os_str().is_empty() => {
            let name = binary.to_string_lossy();
            inherited
                .as_deref()
                .and_then(|path| find_in_path(&name, path))
                .ok_or_else(|| {
                    unavailable(
                        ANCHOR_BINARY,
                        "the name is not an executable in an absolute PATH entry",
                    )
                })?
        }
        _ => binary.to_path_buf(),
    };
    let path = match program.parent().filter(|dir| dir.is_absolute()) {
        Some(directory) => Some(path_with_directory_first(directory, inherited.as_deref())?),
        None => None,
    };
    let envs: Vec<(&str, &str)> = path.iter().map(|path| ("PATH", path.as_str())).collect();
    let output = run_once(&program.to_string_lossy(), &[], &envs, ANCHOR_TIMEOUT)
        .await
        .map_err(ConformanceError::Run)?;

    if !output.status.success() {
        return Err(ConformanceError::Run(format!(
            "the anchor exited with {} ; stderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|e| ConformanceError::Anchor(format!("anchor stdout is not frozen wire: {e}")))
}

pub async fn frozen_wire() -> Result<FrozenWire> {
    let binary = locate_anchor().await?;
    render_wire(&binary).await
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::process::Command;

    use super::*;
    use crate::test_scratch::{assert_child_ran_one_passing_test, scratch};

    const CHILD: &str = "CONFORMANCE_NATS_FABRIC_RENDER_WIRE_PATH_CHILD";
    const TEST: &str = "anchor::tests::a_relative_anchor_never_searches_the_current_directory_and_an_absolute_one_leads_the_child_path";

    fn install(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write a script");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        path
    }

    fn emit_marker(marker: &str) -> String {
        format!(
            "printf '%s' '{{\"command_subjects\":[],\"event_subjects\":[],\"published_users\":[],\"poison_user_key\":\"{marker}\",\"poison_user_value\":\"\"}}'"
        )
    }

    async fn marker_of(binary: &Path) -> String {
        render_wire(binary)
            .await
            .expect("the script anchor runs")
            .poison_user_key
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

        let cwd = scratch("cwd");
        let on_path = scratch("on-path");
        let absolute_dir = scratch("absolute");

        install(&cwd, ANCHOR_BINARY, &emit_marker("cwd"));
        install(&on_path, ANCHOR_BINARY, &emit_marker("path"));
        install(&absolute_dir, "sibling-tool", &emit_marker("sibling"));
        let absolute = install(&absolute_dir, ANCHOR_BINARY, "sibling-tool");
        std::fs::create_dir_all(cwd.join("bin")).expect("create bin");
        install(&cwd.join("bin"), ANCHOR_BINARY, &emit_marker("relative"));

        let path =
            std::env::join_paths([on_path.as_path(), Path::new("/usr/bin"), Path::new("/bin")])
                .expect("join PATH");
        let output = Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", TEST, "--nocapture"])
            .current_dir(&cwd)
            .env(CHILD, absolute.as_os_str())
            .env("PATH", path)
            .output()
            .expect("run the child test");
        for dir in [cwd, on_path, absolute_dir] {
            std::fs::remove_dir_all(dir).ok();
        }
        assert_child_ran_one_passing_test(&output);
    }
}
