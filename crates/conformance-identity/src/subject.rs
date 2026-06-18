use std::net::TcpListener;
use std::path::Path;

use br_test_harness::SpawnedProcess;
use reqwest::StatusCode;

pub struct SubjectConfig {
    pub nats_url: String,
    pub enabled: bool,
}

impl SubjectConfig {
    pub fn new(nats_url: &str) -> Self {
        Self {
            nats_url: nats_url.to_string(),
            enabled: true,
        }
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

pub struct Subject {
    process: SpawnedProcess,
    base_url: String,
    client: reqwest::Client,
}

impl Subject {
    pub fn spawn(binary: &Path, config: &SubjectConfig) -> Self {
        let addr = free_loopback_addr();
        let base_url = format!("http://{addr}");
        let enabled = config.enabled.to_string();
        let envs = [
            ("NATS_URL", config.nats_url.as_str()),
            ("HTTP_ADDR", addr.as_str()),
            ("SCOPE_ACCEPTANCE_ENABLED", enabled.as_str()),
        ];

        let process = SpawnedProcess::spawn(&binary.to_string_lossy(), &[], &envs);
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .expect("failed to build reqwest client");

        Self {
            process,
            base_url,
            client,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn logs(&self) -> String {
        self.process.logs()
    }

    pub async fn readyz_status(&self) -> Option<StatusCode> {
        self.status_of("/readyz").await
    }

    pub async fn livez_status(&self) -> Option<StatusCode> {
        self.status_of("/livez").await
    }

    pub async fn ready(&self) -> bool {
        self.readyz_status().await == Some(StatusCode::OK)
    }

    async fn status_of(&self, path: &str) -> Option<StatusCode> {
        self.client
            .get(format!("{}{path}", self.base_url))
            .send()
            .await
            .ok()
            .map(|resp| resp.status())
    }

    pub async fn shutdown(self) {
        self.process.shutdown().await;
    }
}

fn free_loopback_addr() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to reserve a loopback port");
    let port = listener
        .local_addr()
        .expect("reserved listener has no address")
        .port();
    format!("127.0.0.1:{port}")
}
