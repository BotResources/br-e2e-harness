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
        self.logs.lock().expect("logs mutex poisoned").clone()
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
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let chunk = String::from_utf8_lossy(&buf[..n]);
                    if let Ok(mut guard) = logs.lock() {
                        guard.push_str(&chunk);
                    }
                }
            }
        }
    })
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

    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => Err(format!("'{program}' failed to run: {e}")),
        Err(_) => Err(format!("'{program}' did not complete within {timeout:?}")),
    }
}
