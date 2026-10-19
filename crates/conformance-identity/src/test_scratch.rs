use std::path::PathBuf;

pub(crate) fn scratch(label: &str) -> PathBuf {
    let mut dir = std::env::current_exe().expect("test executable");
    dir.pop();
    let dir = dir.join(format!("scratch-{label}-{}", uuid::Uuid::now_v7().simple()));
    std::fs::create_dir_all(&dir).expect("create a scratch directory");
    dir
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
