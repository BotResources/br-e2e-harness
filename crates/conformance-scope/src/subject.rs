use std::net::TcpListener;
use std::path::Path;

use br_test_harness::SpawnedProcess;
use reqwest::StatusCode;

pub struct SubjectConfig {
    pub nats_url: String,
    pub command_stream: String,
    pub event_stream: String,
    pub service_key: String,
    pub scope_keys: String,
    pub label_key: String,
    pub description_key: String,
    pub platform_only: bool,
    pub wait_timeout: String,
    pub enabled: bool,
}

impl SubjectConfig {
    pub fn new(
        nats_url: &str,
        command_stream: &str,
        event_stream: &str,
        service_key: &str,
    ) -> Self {
        Self {
            nats_url: nats_url.to_string(),
            command_stream: command_stream.to_string(),
            event_stream: event_stream.to_string(),
            service_key: service_key.to_string(),
            scope_keys: String::new(),
            label_key: String::new(),
            description_key: String::new(),
            platform_only: false,
            wait_timeout: "500ms".to_string(),
            enabled: true,
        }
    }

    #[must_use]
    pub fn scope_keys(mut self, keys: &str) -> Self {
        self.scope_keys = keys.to_string();
        self
    }

    #[must_use]
    pub fn label_key(mut self, key: &str) -> Self {
        self.label_key = key.to_string();
        self
    }

    #[must_use]
    pub fn description_key(mut self, key: &str) -> Self {
        self.description_key = key.to_string();
        self
    }

    #[must_use]
    pub fn platform_only(mut self, platform_only: bool) -> Self {
        self.platform_only = platform_only;
        self
    }

    #[must_use]
    pub fn wait_timeout(mut self, wait_timeout: &str) -> Self {
        self.wait_timeout = wait_timeout.to_string();
        self
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
        let platform_only = config.platform_only.to_string();
        let enabled = config.enabled.to_string();
        let envs = [
            ("NATS_URL", config.nats_url.as_str()),
            ("HTTP_ADDR", addr.as_str()),
            ("COMMAND_STREAM_NAME", config.command_stream.as_str()),
            ("EVENT_STREAM_NAME", config.event_stream.as_str()),
            ("SERVICE_KEY", config.service_key.as_str()),
            ("SCOPE_KEYS", config.scope_keys.as_str()),
            ("LABEL_KEY", config.label_key.as_str()),
            ("DESCRIPTION_KEY", config.description_key.as_str()),
            ("PLATFORM_ONLY", platform_only.as_str()),
            ("WAIT_TIMEOUT", config.wait_timeout.as_str()),
            ("SCOPE_DECLARATION_ENABLED", enabled.as_str()),
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

    pub async fn not_ready(&self) -> bool {
        self.readyz_status().await == Some(StatusCode::SERVICE_UNAVAILABLE)
    }

    pub async fn readyz_body(&self) -> Option<String> {
        let resp = self
            .client
            .get(format!("{}/readyz", self.base_url))
            .send()
            .await
            .ok()?;
        resp.text().await.ok()
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
