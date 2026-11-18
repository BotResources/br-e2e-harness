use std::path::PathBuf;

use serde::Deserialize;

use crate::error::{CliError, Result};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub services: Vec<ServiceSpec>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceSpec {
    pub service_key: String,
    #[serde(default)]
    pub scopes: Vec<ScopeSpec>,
    #[serde(default)]
    pub reject: Option<String>,
    #[serde(default)]
    pub scenarios: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout: String,
    #[serde(default)]
    pub attach: Option<AttachSpec>,
    #[serde(default)]
    pub spawn: Option<SpawnSpec>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeSpec {
    pub key: String,
    #[serde(default)]
    pub platform_only: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachSpec {
    pub nats: String,
    pub readyz: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnSpec {
    pub path: PathBuf,
}

fn default_timeout() -> String {
    "10s".to_string()
}

pub fn load(raw: &str) -> Result<Manifest> {
    serde_saphyr::from_str(raw).map_err(|e| CliError::new(format!("invalid manifest: {e}")))
}

#[cfg(test)]
mod tests {
    use super::load;

    const SAMPLE: &str = "services:
  - service_key: example_service
    scopes:
      - key: example_service:read
        platform_only: false
      - key: example_service:admin
        platform_only: true
    attach:
      nats: nats://127.0.0.1:4222
      readyz: http://127.0.0.1:8080/readyz
  - service_key: other_service
    scopes:
      - key: other_service:read
    scenarios: s1,s2
    reject: scope_owned_by_another_service
    timeout: 30s
    spawn:
      path: ./other-subject
";

    #[test]
    fn given_the_documented_manifest_when_loaded_then_every_field_is_read() {
        let manifest = load(SAMPLE).expect("the documented manifest loads");

        assert_eq!(manifest.services.len(), 2);
        let first = &manifest.services[0];
        assert_eq!(first.service_key, "example_service");
        assert_eq!(first.scopes.len(), 2);
        assert_eq!(first.scopes[0].key, "example_service:read");
        assert!(!first.scopes[0].platform_only);
        assert!(first.scopes[1].platform_only);
        assert_eq!(first.timeout, "10s");
        let attach = first.attach.as_ref().expect("attach is read");
        assert_eq!(attach.nats, "nats://127.0.0.1:4222");
        assert_eq!(attach.readyz, "http://127.0.0.1:8080/readyz");
        assert!(first.spawn.is_none());

        let second = &manifest.services[1];
        assert_eq!(second.scenarios.as_deref(), Some("s1,s2"));
        assert_eq!(
            second.reject.as_deref(),
            Some("scope_owned_by_another_service")
        );
        assert_eq!(second.timeout, "30s");
        assert_eq!(
            second.spawn.as_ref().expect("spawn is read").path,
            std::path::PathBuf::from("./other-subject")
        );
        assert!(second.attach.is_none());
    }

    #[test]
    fn given_an_unknown_field_when_loaded_then_it_is_refused_naming_the_field() {
        let typo_on_scope = "services:
  - service_key: my_service
    scopes:
      - key: my_service:admin
        platform_onyl: true
    attach:
      nats: nats://127.0.0.1:4222
      readyz: http://127.0.0.1:8080/readyz
";
        let typo_on_service = "services:
  - service_key: my_service
    scenarioz: s1
    attach:
      nats: nats://127.0.0.1:4222
      readyz: http://127.0.0.1:8080/readyz
";
        let typo_on_attach = "services:
  - service_key: my_service
    attach:
      nats: nats://127.0.0.1:4222
      readyz: http://127.0.0.1:8080/readyz
      timeout: 5s
";
        let typo_on_spawn = "services:
  - service_key: my_service
    spawn:
      path: ./subject
      args: []
";
        let typo_on_root = "services: []\nextra: true\n";

        for (raw, field) in [
            (typo_on_scope, "platform_onyl"),
            (typo_on_service, "scenarioz"),
            (typo_on_attach, "timeout"),
            (typo_on_spawn, "args"),
            (typo_on_root, "extra"),
        ] {
            let error = load(raw).expect_err(field);
            let message = error.to_string();
            assert!(message.contains("invalid manifest"), "{message}");
            assert!(message.contains(field), "{field}: {message}");
        }
    }

    #[test]
    fn given_a_manifest_without_services_when_loaded_then_it_is_refused() {
        let error = load("scopes: []").expect_err("services is required");

        assert!(error.to_string().contains("invalid manifest"));
    }

    #[test]
    fn given_malformed_yaml_when_loaded_then_it_is_refused() {
        let error = load("services: [unclosed").expect_err("malformed yaml");

        assert!(error.to_string().contains("invalid manifest"));
    }
}
