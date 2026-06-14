use std::net::TcpListener;
use std::path::Path;

use br_test_harness::SpawnedProcess;

use crate::seed::BEARER_BUCKET;

pub struct SubjectConfig {
    pub nats_url: String,
    pub bearer_bucket: String,
}

impl SubjectConfig {
    pub fn new(nats_url: &str) -> Self {
        Self {
            nats_url: nats_url.to_string(),
            bearer_bucket: BEARER_BUCKET.to_string(),
        }
    }
}

pub struct Subject {
    process: SpawnedProcess,
    base_url: String,
}

impl Subject {
    pub fn spawn(binary: &Path, config: &SubjectConfig) -> Self {
        let addr = free_loopback_addr();
        let base_url = format!("http://{addr}");
        let envs = [
            ("NATS_URL", config.nats_url.as_str()),
            ("HTTP_ADDR", addr.as_str()),
            ("BEARER_BUCKET", config.bearer_bucket.as_str()),
        ];

        let process = SpawnedProcess::spawn(&binary.to_string_lossy(), &[], &envs);

        Self { process, base_url }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn logs(&self) -> String {
        self.process.logs()
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
