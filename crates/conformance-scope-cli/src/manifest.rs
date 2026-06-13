use std::path::PathBuf;

use conformance_scope::{
    AcceptorBehavior, AttachTarget, DEFAULT_STREAM_NAME, ExpectedDeclaration, PlatformOnly,
    SAMPLE_FALLBACK_SCOPE, SpawnTarget, attach_default, parse_scenarios, run_attach, run_spawn,
    spawn_default,
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
    #[serde(default)]
    pub stream: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SpawnSpec {
    pub path: PathBuf,
}

fn default_timeout() -> String {
    "10s".to_string()
}

pub fn load(raw: &str) -> Result<Manifest> {
    serde_yaml_ng::from_str(raw).map_err(|e| CliError::new(format!("invalid manifest: {e}")))
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
                stream_name: attach
                    .stream
                    .clone()
                    .unwrap_or_else(|| DEFAULT_STREAM_NAME.to_string()),
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
