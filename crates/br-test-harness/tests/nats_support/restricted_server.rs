use std::time::{Duration, Instant};

use async_nats::ConnectOptions;
use async_nats::jetstream;
use tempfile::TempDir;
use tokio::process::{Child, Command};

pub struct RestrictedServer {
    child: Child,
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
        let mut child = Command::new("nats-server")
            .arg("-c")
            .arg(&config_path)
            .arg("--ports_file_dir")
            .arg(dir.path())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn nats-server");
        let pid = child.id().expect("spawned nats-server has no pid");
        let ports_file = dir.path().join(format!("nats-server_{pid}.ports"));
        let port = await_port(&mut child, &ports_file).await;
        await_accept(&mut child, port).await;
        Self {
            child,
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

    pub async fn shutdown(mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}

async fn await_port(child: &mut Child, ports_file: &std::path::Path) -> u16 {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            panic!("the restricted nats-server exited early (status {status})");
        }
        if let Some(port) = std::fs::read_to_string(ports_file)
            .ok()
            .and_then(|contents| parse_client_port(&contents))
        {
            return port;
        }
        assert!(
            Instant::now() < deadline,
            "the restricted nats-server did not report its port within 10s"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn await_accept(child: &mut Child, port: u16) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .is_err()
    {
        if let Ok(Some(status)) = child.try_wait() {
            panic!("the restricted nats-server exited early (status {status})");
        }
        assert!(
            Instant::now() < deadline,
            "the restricted nats-server did not accept within 10s"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn parse_client_port(ports_file: &str) -> Option<u16> {
    let parsed: serde_json::Value = serde_json::from_str(ports_file).ok()?;
    let first = parsed.get("nats")?.as_array()?.first()?.as_str()?;
    first.rsplit(':').next()?.parse().ok()
}
