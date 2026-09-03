use std::net::TcpListener;
use std::path::Path;

use br_test_harness::SpawnedProcess;

use crate::vectors::seal_key_b64;

pub struct SubjectConfig {
    pub nats_url: String,
    pub seal_key_b64: String,
}

impl SubjectConfig {
    pub fn new(nats_url: &str) -> Self {
        Self {
            nats_url: nats_url.to_string(),
            seal_key_b64: seal_key_b64(),
        }
    }
}

pub struct Subject {
    process: SpawnedProcess,
    base_url: String,
}

impl Subject {
    pub fn spawn(binary: &Path, config: &SubjectConfig) -> Self {
        let port = free_loopback_port();
        let base_url = format!("http://127.0.0.1:{port}");
        let port = port.to_string();
        let envs = [
            ("NATS_URL", config.nats_url.as_str()),
            ("PORT", port.as_str()),
            ("BEARER_SEAL_KEY", config.seal_key_b64.as_str()),
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

fn free_loopback_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to reserve a loopback port");
    listener
        .local_addr()
        .expect("reserved listener has no address")
        .port()
}
