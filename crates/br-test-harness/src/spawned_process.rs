use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};

pub struct SpawnedProcess {
    child: Child,
    logs: Arc<Mutex<String>>,
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

        if let Some(stdout) = child.stdout.take() {
            spawn_drain(stdout, logs.clone());
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_drain(stderr, logs.clone());
        }

        Self { child, logs }
    }

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

    pub fn logs(&self) -> String {
        self.logs.lock().expect("logs mutex poisoned").clone()
    }

    pub async fn shutdown(mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}

fn spawn_drain<R>(mut reader: R, logs: Arc<Mutex<String>>)
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
    });
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
