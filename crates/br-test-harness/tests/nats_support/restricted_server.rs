use std::time::{Duration, Instant};

use async_nats::ConnectOptions;
use async_nats::jetstream;
use br_test_harness::SpawnedProcess;
use tempfile::TempDir;

pub struct RestrictedServer {
    process: SpawnedProcess,
    port: u16,
    _dir: TempDir,
}

impl RestrictedServer {
    pub async fn start(denied_publish: &str) -> Self {
        let dir = TempDir::new().expect("temp dir");
        let store = dir.path().join("store");
        let config = format!(
            r#"
listen: 127.0.0.1:-1
jetstream {{ store_dir: "{store}" }}
authorization {{
  users = [
    {{ user: admin, password: admin }}
    {{ user: restricted, password: restricted,
       permissions: {{
         publish: {{ allow: [">"], deny: ["{denied_publish}"] }}
         subscribe: {{ allow: [">"] }}
       }} }}
  ]
}}
"#,
            store = store.display()
        );
        let config_path = dir.path().join("server.conf");
        std::fs::write(&config_path, config).expect("write config");
        let process = SpawnedProcess::spawn(
            "nats-server",
            &[
                "-c",
                config_path.to_str().expect("utf-8 config path"),
                "--ports_file_dir",
                dir.path().to_str().expect("utf-8 temp dir"),
            ],
            &[],
        );
        let port = await_port(&process, dir.path()).await;
        await_accept(&process, port).await;
        Self {
            process,
            port,
            _dir: dir,
        }
    }

    #[allow(dead_code)] // each test binary that includes this module uses a subset of it
    pub fn url_as(&self, user: &str) -> String {
        format!("nats://{user}:{user}@127.0.0.1:{}", self.port)
    }

    #[allow(dead_code)] // each test binary that includes this module uses a subset of it
    pub async fn jetstream_as(&self, user: &str) -> jetstream::Context {
        let client = ConnectOptions::with_user_and_password(user.into(), user.into())
            .request_timeout(Some(Duration::from_secs(1)))
            .connect(format!("nats://127.0.0.1:{}", self.port))
            .await
            .expect("connect to the restricted server");
        jetstream::new(client)
    }

    pub async fn shutdown(self) {
        self.process.shutdown().await;
    }
}

async fn await_port(process: &SpawnedProcess, dir: &std::path::Path) -> u16 {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(port) = ports_file_in(dir)
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|contents| parse_client_port(&contents))
        {
            return port;
        }
        assert!(
            Instant::now() < deadline,
            "the restricted nats-server did not report its port within 10s; logs:\n{}",
            process.logs()
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn ports_file_in(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("nats-server_") && name.ends_with(".ports"))
        })
}

async fn await_accept(process: &SpawnedProcess, port: u16) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .is_err()
    {
        assert!(
            Instant::now() < deadline,
            "the restricted nats-server did not accept within 10s; logs:\n{}",
            process.logs()
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn parse_client_port(ports_file: &str) -> Option<u16> {
    let parsed: serde_json::Value = serde_json::from_str(ports_file).ok()?;
    let first = parsed.get("nats")?.as_array()?.first()?.as_str()?;
    first.rsplit(':').next()?.parse().ok()
}
