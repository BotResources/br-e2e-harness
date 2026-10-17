use std::time::{Duration, Instant};

use tempfile::TempDir;
use tokio::process::{Child, Command};

const LOOPBACK: &str = "127.0.0.1";

pub struct SpawnedNats {
    child: Child,
    port: u16,
    _store_dir: TempDir,
}

impl SpawnedNats {
    pub async fn start() -> Self {
        if Command::new("nats-server")
            .arg("--version")
            .output()
            .await
            .is_err()
        {
            panic!(
                "nats-server not found on PATH — install it (brew install nats-server) \
                 to run e2e tests"
            );
        }

        let store_dir = TempDir::new().expect("failed to create NATS store TempDir");

        let mut child = Command::new("nats-server")
            .arg("-js")
            .arg("-a")
            .arg(LOOPBACK)
            .arg("-p")
            .arg("-1")
            .arg("-sd")
            .arg(store_dir.path())
            .arg("--ports_file_dir")
            .arg(store_dir.path())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("failed to spawn nats-server");

        let pid = child.id().expect("spawned nats-server has no pid");
        crate::orphan_guard::spawn(pid, Some(store_dir.path()));
        let ports_file = store_dir.path().join(format!("nats-server_{pid}.ports"));

        let port = await_port(&mut child, &ports_file).await;
        await_accept(&mut child, port).await;

        Self {
            child,
            port,
            _store_dir: store_dir,
        }
    }

    pub fn url(&self) -> String {
        format!("nats://{LOOPBACK}:{}", self.port)
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn shutdown(mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}

async fn await_port(child: &mut Child, ports_file: &std::path::Path) -> u16 {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            let stderr = drain_stderr(child).await;
            panic!(
                "nats-server exited early (status {status}) before reporting its port; stderr:\n{stderr}"
            );
        }
        if let Some(port) = std::fs::read_to_string(ports_file)
            .ok()
            .as_deref()
            .and_then(parse_client_port)
        {
            return port;
        }
        if Instant::now() >= deadline {
            let stderr = drain_stderr(child).await;
            let _ = child.kill().await;
            panic!("nats-server did not report its port within 10s; stderr:\n{stderr}");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn await_accept(child: &mut Child, port: u16) {
    let addr = format!("{LOOPBACK}:{port}");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if tokio::net::TcpStream::connect(&addr).await.is_ok() {
            return;
        }
        if let Ok(Some(status)) = child.try_wait() {
            let stderr = drain_stderr(child).await;
            panic!(
                "nats-server exited early (status {status}) before accepting; stderr:\n{stderr}"
            );
        }
        if Instant::now() >= deadline {
            let stderr = drain_stderr(child).await;
            let _ = child.kill().await;
            panic!("nats-server did not become ready within 10s on {addr}; stderr:\n{stderr}");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn parse_client_port(ports_file: &str) -> Option<u16> {
    let parsed: serde_json::Value = serde_json::from_str(ports_file).ok()?;
    let first = parsed.get("nats")?.as_array()?.first()?.as_str()?;
    first.rsplit(':').next()?.parse().ok()
}

async fn drain_stderr(child: &mut Child) -> String {
    let Some(stderr) = child.stderr.take() else {
        return String::from("<stderr unavailable>");
    };
    read_diagnostic(stderr, Duration::from_millis(500)).await
}

async fn read_diagnostic<R>(mut reader: R, window: Duration) -> String
where
    R: tokio::io::AsyncRead + Unpin,
{
    use tokio::io::AsyncReadExt;
    let mut buf = Vec::new();
    let outcome = tokio::time::timeout(window, reader.read_to_end(&mut buf)).await;
    let mut text = String::from_utf8_lossy(&buf).into_owned();
    if let Ok(Err(e)) = outcome {
        text.push_str(&format!("\n<stderr read failed: {e}>"));
    }
    text
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use std::time::Duration;

    use tokio::io::{AsyncRead, ReadBuf};

    use super::{parse_client_port, read_diagnostic};

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
    async fn a_failed_stderr_read_is_named_in_the_diagnostic() {
        let text = read_diagnostic(BrokenPipe, Duration::from_millis(500)).await;

        assert!(text.contains("<stderr read failed: pipe broke>"), "{text}");
    }

    #[tokio::test]
    async fn a_complete_stderr_read_is_returned_as_is() {
        let text = read_diagnostic(&b"boom"[..], Duration::from_millis(500)).await;

        assert_eq!(text, "boom");
    }

    #[tokio::test]
    async fn a_silent_stderr_stays_quiet_at_the_window() {
        let (_writer, reader) = tokio::io::duplex(64);

        let text = read_diagnostic(reader, Duration::from_millis(50)).await;

        assert_eq!(text, "");
    }

    #[test]
    fn reads_the_bound_port_the_server_reports() {
        let contents = r#"{"nats":["nats://127.0.0.1:49262","nats://[::1]:49262","nats://192.168.1.123:49262"]}"#;
        assert_eq!(parse_client_port(contents), Some(49262));
    }

    #[test]
    fn malformed_or_empty_report_yields_none() {
        assert_eq!(parse_client_port("not json"), None);
        assert_eq!(parse_client_port(""), None);
        assert_eq!(parse_client_port(r#"{"nats":[]}"#), None);
        assert_eq!(
            parse_client_port(r#"{"monitoring":["http://127.0.0.1:8222"]}"#),
            None
        );
    }
}
