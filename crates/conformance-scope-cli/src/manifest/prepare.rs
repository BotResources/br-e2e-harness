use std::collections::HashMap;
use std::time::Duration;

use conformance_scope::{
    AcceptorBehavior, AttachTarget, ExpectedDeclaration, ExpectedScope, SAMPLE_FALLBACK_SCOPE,
    Scenario, SpawnTarget, attach_default, check_attach_input, check_spawn_input, parse_scenarios,
    spawn_default,
};

use crate::duration;
use crate::error::{CliError, Result};

use super::schema::Manifest;
use super::schema::ServiceSpec;

pub(super) struct Prepared<'a> {
    pub(super) service_key: &'a str,
    pub(super) expected: ExpectedDeclaration,
    pub(super) behavior: AcceptorBehavior,
    pub(super) scenarios: Vec<Scenario>,
    pub(super) timeout: Duration,
    pub(super) target: Target,
}

pub(super) enum Target {
    Attach(AttachTarget),
    Spawn(SpawnTarget),
}

pub(super) fn prepare(manifest: &Manifest) -> Result<Vec<Prepared<'_>>> {
    if manifest.services.is_empty() {
        return Err(CliError::new(
            "manifest lists no services; there is nothing to check",
        ));
    }
    refuse_shared_brokers(&manifest.services)?;
    manifest
        .services
        .iter()
        .map(|spec| {
            prepare_service(spec)
                .map_err(|e| CliError::new(format!("service {:?}: {e}", spec.service_key)))
        })
        .collect()
}

fn refuse_shared_brokers(services: &[ServiceSpec]) -> Result<()> {
    let mut owners: HashMap<&str, &str> = HashMap::new();
    for spec in services {
        let Some(attach) = &spec.attach else { continue };
        let broker = attach.nats.trim_end_matches('/');
        if let Some(first) = owners.insert(broker, &spec.service_key) {
            return Err(CliError::new(format!(
                "services {first:?} and {:?} both attach to the broker {:?}; attach assumes a fresh handshake stream, so a broker serves one attach entry",
                spec.service_key, attach.nats
            )));
        }
    }
    Ok(())
}

fn prepare_service(spec: &ServiceSpec) -> Result<Prepared<'_>> {
    let expected = build_expected(spec);
    let timeout = duration::parse(&spec.timeout)?;
    let behavior = build_behavior(spec, &expected)?;
    let (target, scenarios) = match (&spec.attach, &spec.spawn) {
        (Some(_), Some(_)) => {
            return Err(CliError::new(
                "declares both `attach` and `spawn`; pick one",
            ));
        }
        (None, None) => {
            return Err(CliError::new("declares neither `attach` nor `spawn`"));
        }
        (Some(attach), None) => {
            let scenarios = resolve_scenarios(spec, attach_default())?;
            let target = AttachTarget {
                nats_url: attach.nats.clone(),
                readyz_url: attach.readyz.clone(),
            };
            check_attach_input(&target, &expected, &scenarios, timeout)?;
            (Target::Attach(target), scenarios)
        }
        (None, Some(spawn)) => {
            let scenarios = resolve_scenarios(spec, spawn_default())?;
            let target = SpawnTarget {
                binary: spawn.path.clone(),
            };
            check_spawn_input(&target, &expected, &scenarios, timeout)?;
            (Target::Spawn(target), scenarios)
        }
    };
    Ok(Prepared {
        service_key: &spec.service_key,
        expected,
        behavior,
        scenarios,
        timeout,
        target,
    })
}

fn resolve_scenarios(spec: &ServiceSpec, default: Vec<Scenario>) -> Result<Vec<Scenario>> {
    match &spec.scenarios {
        Some(raw) => Ok(parse_scenarios(raw)?),
        None => Ok(default),
    }
}

fn build_expected(spec: &ServiceSpec) -> ExpectedDeclaration {
    let scopes = spec
        .scopes
        .iter()
        .map(|scope| ExpectedScope {
            key: scope.key.clone(),
            platform_only: scope.platform_only,
        })
        .collect();
    ExpectedDeclaration::new(spec.service_key.clone(), scopes)
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
