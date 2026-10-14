use std::path::PathBuf;

use conformance_scope::{
    AcceptorBehavior, AttachTarget, ExpectedDeclaration, PlatformOnly, SAMPLE_FALLBACK_SCOPE,
    SpawnTarget, attach_default, parse_scenarios, run_attach, run_spawn, spawn_default,
};
use serde::Deserialize;

use crate::duration;
use crate::error::{CliError, Result};
use crate::report::{AggregateReport, ServiceReport};

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub services: Vec<ServiceSpec>,
}

#[derive(Debug, Deserialize)]
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
pub struct ScopeSpec {
    pub key: String,
    #[serde(default)]
    pub platform_only: bool,
}

#[derive(Debug, Deserialize)]
pub struct AttachSpec {
    pub nats: String,
    pub readyz: String,
}

#[derive(Debug, Deserialize)]
pub struct SpawnSpec {
    pub path: PathBuf,
}

fn default_timeout() -> String {
    "10s".to_string()
}

pub fn load(raw: &str) -> Result<Manifest> {
    serde_saphyr::from_str(raw).map_err(|e| CliError::new(format!("invalid manifest: {e}")))
}

pub async fn run(manifest: &Manifest) -> Result<AggregateReport> {
    let mut services = Vec::with_capacity(manifest.services.len());
    for spec in &manifest.services {
        services.push(run_service(spec).await?);
    }
    Ok(AggregateReport::from_services(services))
}

async fn run_service(spec: &ServiceSpec) -> Result<ServiceReport> {
    let platform_only = build_platform_only(spec);
    let scope_keys: Vec<String> = spec.scopes.iter().map(|s| s.key.clone()).collect();
    let expected = ExpectedDeclaration::from_parts(&spec.service_key, &scope_keys, &platform_only);
    let timeout = duration::parse(&spec.timeout)?;
    let behavior = build_behavior(spec, &expected)?;

    let report = match (&spec.attach, &spec.spawn) {
        (Some(_), Some(_)) => {
            return Err(CliError::new(format!(
                "service {:?} declares both `attach` and `spawn`; pick one",
                spec.service_key
            )));
        }
        (None, None) => {
            return Err(CliError::new(format!(
                "service {:?} declares neither `attach` nor `spawn`",
                spec.service_key
            )));
        }
        (Some(attach), None) => {
            let scenarios = match &spec.scenarios {
                Some(raw) => parse_scenarios(raw)?,
                None => attach_default(),
            };
            let target = AttachTarget {
                nats_url: attach.nats.clone(),
                readyz_url: attach.readyz.clone(),
            };
            run_attach(&target, &expected, &behavior, &scenarios, timeout).await?
        }
        (None, Some(spawn)) => {
            let scenarios = match &spec.scenarios {
                Some(raw) => parse_scenarios(raw)?,
                None => spawn_default(),
            };
            let target = SpawnTarget {
                binary: spawn.path.clone(),
            };
            run_spawn(&target, &expected, &behavior, &scenarios, timeout).await?
        }
    };

    Ok(ServiceReport::from_report(&spec.service_key, &report))
}

fn build_platform_only(spec: &ServiceSpec) -> PlatformOnly {
    let entries: Vec<(String, bool)> = spec
        .scopes
        .iter()
        .map(|s| (s.key.clone(), s.platform_only))
        .collect();
    PlatformOnly::PerScope(entries)
}

fn build_behavior(spec: &ServiceSpec, expected: &ExpectedDeclaration) -> Result<AcceptorBehavior> {
    match &spec.reject {
        Some(reason_code) => {
            let sample = expected
                .scopes
                .first()
                .map(|s| s.key.as_str())
                .unwrap_or(SAMPLE_FALLBACK_SCOPE);
            Ok(AcceptorBehavior::reject(Some(reason_code), sample)?)
        }
        None => Ok(AcceptorBehavior::Accept),
    }
}

#[cfg(test)]
mod tests {
    use super::load;

    const SAMPLE: &str = "services:
  - service_key: example-service
    scopes:
      - key: example:read
        platform_only: false
      - key: example:admin
        platform_only: true
    attach:
      nats: nats://127.0.0.1:4222
      readyz: http://127.0.0.1:8080/readyz
  - service_key: other-service
    scopes:
      - key: other:read
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
        assert_eq!(first.service_key, "example-service");
        assert_eq!(first.scopes.len(), 2);
        assert_eq!(first.scopes[0].key, "example:read");
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
