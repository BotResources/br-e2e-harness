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
        let port = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("free port");
            listener.local_addr().expect("local addr").port()
        };
        let store = dir.path().join("store");
        let config = format!(
            r#"
listen: 127.0.0.1:{port}
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
        let child = Command::new("nats-server")
            .arg("-c")
            .arg(&config_path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn nats-server");
        let deadline = Instant::now() + Duration::from_secs(10);
        while tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_err()
        {
            assert!(
                Instant::now() < deadline,
                "the restricted nats-server did not accept within 10s"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Self {
            child,
            port,
            _dir: dir,
        }
    }

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
