use std::path::PathBuf;

pub(crate) fn scratch(label: &str) -> PathBuf {
    let mut dir = std::env::current_exe().expect("test executable");
    dir.pop();
    let dir = dir.join(format!("scratch-{label}-{}", uuid::Uuid::now_v7().simple()));
    std::fs::create_dir_all(&dir).expect("create a scratch directory");
    dir
}
