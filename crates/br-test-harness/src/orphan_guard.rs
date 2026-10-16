use std::path::Path;

#[cfg(unix)]
const GUARD: &str = r#"parent="$1"; target="$2"; store="$3"
while kill -0 "$parent" 2>/dev/null && kill -0 "$target" 2>/dev/null; do sleep 1; done
if kill -0 "$parent" 2>/dev/null; then exit 0; fi
if kill -0 "$target" 2>/dev/null; then
  kill -9 "$target"
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    kill -0 "$target" 2>/dev/null || break
    sleep 0.1
  done
fi
if [ -n "$store" ]; then rm -rf -- "$store"; fi"#;

#[cfg(unix)]
pub(crate) fn spawn(target_pid: u32, store_dir: Option<&Path>) {
    let guard = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(GUARD)
        .arg("sh")
        .arg(std::process::id().to_string())
        .arg(target_pid.to_string())
        .arg(store_dir.unwrap_or(Path::new("")))
        .process_group(0)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("failed to spawn the orphan guard of a spawned process");
    drop(guard);
}

#[cfg(not(unix))]
pub(crate) fn spawn(_target_pid: u32, _store_dir: Option<&Path>) {}
