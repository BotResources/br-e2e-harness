use std::path::Path;

#[cfg(unix)]
const GUARD: &str = r#"parent="$1"; target="$2"; store="$3"
while kill -0 "$parent" 2>/dev/null && kill -0 "$target" 2>/dev/null; do sleep 1; done
if kill -0 "$target" 2>/dev/null; then
  kill -9 "$target"
  if [ -n "$store" ]; then rm -rf -- "$store"; fi
fi"#;

#[cfg(unix)]
pub(crate) fn spawn(target_pid: u32, store_dir: Option<&Path>) {
    let guard = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(GUARD)
        .arg("sh")
        .arg(std::process::id().to_string())
        .arg(target_pid.to_string())
        .arg(store_dir.unwrap_or(Path::new("")))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("failed to spawn the orphan guard of a spawned process");
    drop(guard);
}

#[cfg(not(unix))]
pub(crate) fn spawn(_target_pid: u32, _store_dir: Option<&Path>) {}
