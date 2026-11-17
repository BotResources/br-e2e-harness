use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
#[cfg(feature = "http-client")]
use std::time::Instant;

use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::task::JoinHandle;

pub struct SpawnedProcess {
    child: Child,
    logs: Arc<Mutex<String>>,
    drains: Vec<JoinHandle<()>>,
}

#[derive(Debug)]
pub enum BootOutcome {
    Ready,
    Exited(std::process::ExitStatus),
    TimedOut,
}

impl BootOutcome {
    pub fn is_ready(&self) -> bool {
        matches!(self, BootOutcome::Ready)
    }

    pub fn exit_status(&self) -> Option<std::process::ExitStatus> {
        match self {
            BootOutcome::Exited(status) => Some(*status),
            BootOutcome::Ready | BootOutcome::TimedOut => None,
        }
    }
}

impl SpawnedProcess {
    pub fn spawn(program: &str, args: &[&str], envs: &[(&str, &str)]) -> Self {
        let mut command = Command::new(program);
        command
            .args(args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        for (k, v) in envs {
            command.env(k, v);
        }

        let mut child = command
            .spawn()
            .unwrap_or_else(|e| panic!("failed to spawn '{program}': {e}"));
        if let Some(pid) = child.id() {
            crate::orphan_guard::spawn(pid, None);
        }

        let logs = Arc::new(Mutex::new(String::new()));
        let mut drains = Vec::new();

        if let Some(stdout) = child.stdout.take() {
            drains.push(spawn_drain(stdout, logs.clone()));
        }
        if let Some(stderr) = child.stderr.take() {
            drains.push(spawn_drain(stderr, logs.clone()));
        }

        Self {
            child,
            logs,
            drains,
        }
    }

    #[cfg(feature = "http-client")]
    pub async fn wait_for_http_ok(&mut self, url: &str, timeout: Duration) -> Result<(), String> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .expect("failed to build reqwest client");
        let deadline = Instant::now() + timeout;
        loop {
            if let Ok(resp) = client.get(url).send().await
                && resp.status().is_success()
            {
                return Ok(());
            }
            if let Ok(Some(status)) = self.child.try_wait() {
                return Err(format!(
                    "process exited early (status {status}) before {url} returned 200; logs:\n{}",
                    self.logs()
                ));
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "{url} did not return 200 within {timeout:?}; logs:\n{}",
                    self.logs()
                ));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    #[cfg(feature = "http-client")]
    pub async fn await_boot(&mut self, url: &str, timeout: Duration) -> BootOutcome {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .expect("failed to build reqwest client");
        let deadline = Instant::now() + timeout;
        loop {
            if let Ok(Some(status)) = self.child.try_wait() {
                self.drain_to_eof().await;
                return BootOutcome::Exited(status);
            }
            if let Ok(resp) = client.get(url).send().await
                && resp.status().is_success()
            {
                return BootOutcome::Ready;
            }
            if Instant::now() >= deadline {
                return BootOutcome::TimedOut;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    #[cfg(feature = "http-client")]
    async fn drain_to_eof(&mut self) {
        for handle in self.drains.drain(..) {
            let _ = handle.await;
        }
    }

    pub fn logs(&self) -> String {
        self.logs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    #[cfg(feature = "sse")]
    pub(crate) fn logs_handle(&self) -> Arc<Mutex<String>> {
        self.logs.clone()
    }

    pub async fn shutdown(mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
        for handle in self.drains.drain(..) {
            let _ = handle.await;
        }
    }
}

fn spawn_drain<R>(mut reader: R, logs: Arc<Mutex<String>>) -> JoinHandle<()>
where
    R: AsyncReadExt + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break,
                Err(e) => {
                    append_log(&logs, &format!("\n[harness] log capture stopped: {e}\n"));
                    break;
                }
                Ok(n) => append_log(&logs, &String::from_utf8_lossy(&buf[..n])),
            }
        }
    })
}

fn append_log(logs: &Mutex<String>, text: &str) {
    logs.lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push_str(text);
}

/// The binary `name` the workspace built, in the profile directory
/// (`<target>/<profile>/`) above the running test executable. Panics when it
/// is not there.
pub fn workspace_bin(name: &str) -> PathBuf {
    let test_exe = std::env::current_exe().expect("test executable path");
    let bin = profile_dir(&test_exe).join(name);
    assert!(
        bin.exists(),
        "{name} must be built by the workspace at {}",
        bin.display()
    );
    bin
}

/// The profile directory above a test executable. Cargo before 1.100 puts
/// test executables in `<target>/<profile>/deps/`; Cargo 1.100's build-dir
/// layout puts them in `<target>/<profile>/build/<package>/<hash>/out/`.
/// Anywhere else, the executable's own directory is taken as the profile
/// directory. A build dir set apart from the target dir (`build.build-dir`)
/// is not supported: the binaries are not above the test executable there.
fn profile_dir(test_exe: &Path) -> PathBuf {
    let mut dir = test_exe.parent().map(Path::to_path_buf).unwrap_or_default();
    if dir.ends_with("deps") {
        dir.pop();
    } else if dir.ends_with("out")
        && dir
            .ancestors()
            .nth(3)
            .is_some_and(|build| build.ends_with("build"))
    {
        // out, <hash>, <package>, build
        for _ in 0..4 {
            dir.pop();
        }
    }
    dir
}

pub async fn spawn_fabric_provision(nats_url: &str, manifest_path: &Path) -> Result<(), String> {
    let bin = workspace_bin("fabric-nats");
    let output = run_once(
        &bin.to_string_lossy(),
        &[
            "provision",
            "--nats",
            nats_url,
            "--manifest",
            &manifest_path.to_string_lossy(),
        ],
        &[],
        Duration::from_secs(30),
    )
    .await
    .map_err(|e| format!("spawning fabric-nats provision: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "fabric-nats provision failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

pub async fn run_once(
    program: &str,
    args: &[&str],
    envs: &[(&str, &str)],
    timeout: Duration,
) -> Result<std::process::Output, String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    for (k, v) in envs {
        command.env(k, v);
    }

    let child = command
        .spawn()
        .map_err(|e| format!("failed to spawn '{program}': {e}"))?;
    if let Some(pid) = child.id() {
        crate::orphan_guard::spawn(pid, None);
    }

    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => Err(format!("'{program}' failed to run: {e}")),
        Err(_) => Err(format!("'{program}' did not complete within {timeout:?}")),
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use tokio::io::{AsyncRead, ReadBuf};

    use super::*;

    struct BrokenPipe;

    impl AsyncRead for BrokenPipe {
        fn poll_read(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            Poll::Ready(Err(io::Error::other("pipe broke")))
        }
    }

    #[tokio::test]
    async fn a_failed_log_read_is_recorded_in_the_logs_not_dropped() {
        let logs = Arc::new(Mutex::new(String::new()));

        spawn_drain(BrokenPipe, logs.clone())
            .await
            .expect("drain task");

        assert!(
            logs.lock()
                .unwrap()
                .contains("log capture stopped: pipe broke")
        );
    }

    #[tokio::test]
    async fn a_poisoned_log_mutex_still_receives_and_returns_output() {
        let mut process = SpawnedProcess::spawn("sh", &["-c", "sleep 0.3; echo hello"], &[]);
        let poisoner = process.logs.clone();
        let _ = std::thread::spawn(move || {
            let _guard = poisoner.lock().unwrap();
            panic!("poison the mutex");
        })
        .join();

        for handle in process.drains.drain(..) {
            handle.await.expect("drain task");
        }

        assert_eq!(process.logs().trim(), "hello");
    }

    #[test]
    fn the_profile_dir_is_above_a_test_executable_in_deps() {
        assert_eq!(
            profile_dir(Path::new("/w/target/debug/deps/spec-0123abcd")),
            PathBuf::from("/w/target/debug")
        );
    }

    #[test]
    fn the_profile_dir_is_above_a_test_executable_in_the_cargo_1_100_build_dir() {
        assert_eq!(
            profile_dir(Path::new(
                "/w/target/debug/build/br-test-harness/cf48c2adcc928678/out/spec-0123abcd"
            )),
            PathBuf::from("/w/target/debug")
        );
    }

    #[test]
    fn an_out_dir_that_is_not_under_build_is_taken_as_the_profile_dir() {
        assert_eq!(
            profile_dir(Path::new("/w/target/release/out/spec-0123abcd")),
            PathBuf::from("/w/target/release/out")
        );
    }
}
