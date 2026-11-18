use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub(crate) struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<OsStr> for Scratch {
    fn as_ref(&self) -> &OsStr {
        self.0.as_os_str()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

pub(crate) fn scratch(label: &str) -> Scratch {
    let mut dir = std::env::current_exe().expect("test executable");
    dir.pop();
    let dir = dir.join(format!("scratch-{label}-{}", uuid::Uuid::now_v7().simple()));
    std::fs::create_dir_all(&dir).expect("create a scratch directory");
    Scratch(dir)
}

pub(crate) fn assert_child_ran_one_passing_test(output: &std::process::Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the child must pass: {}\n{stdout}\n{stderr}",
        output.status
    );
    assert!(
        stdout.contains("1 passed;"),
        "the child must run exactly the filtered test (a renamed test runs none):\n{stdout}\n{stderr}"
    );
}
