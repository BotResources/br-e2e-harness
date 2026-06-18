use conformance_scope::{
    AcceptorBehavior, AttachTarget, ExpectedDeclaration, SAMPLE_FALLBACK_SCOPE, Scenario,
    SpawnTarget, attach_default, parse_platform_only, parse_scenarios, parse_scope_keys,
    run_attach, run_spawn, spawn_default,
};

use crate::cli::RunArgs;
use crate::duration;
use crate::error::{CliError, Result};
use crate::report::ServiceReport;

pub async fn run_single(args: &RunArgs) -> Result<ServiceReport> {
    let expected = build_expected(args)?;
    let timeout = duration::parse(&args.timeout)?;
    let behavior = build_behavior(args, &expected)?;

    let report = if let Some(binary) = &args.spawn {
        let scenarios = resolve_scenarios(args, &spawn_default())?;
        let target = SpawnTarget {
            binary: binary.clone(),
        };
        run_spawn(&target, &expected, &behavior, &scenarios, timeout).await?
    } else {
        let scenarios = resolve_scenarios(args, &attach_default())?;
        let target = attach_target(args)?;
        run_attach(&target, &expected, &behavior, &scenarios, timeout).await?
    };

    Ok(ServiceReport::from_report(&expected.service_key, &report))
}

fn build_expected(args: &RunArgs) -> Result<ExpectedDeclaration> {
    let scope_keys = parse_scope_keys(&args.scopes);
    let platform_only = parse_platform_only(&args.platform_only)?;
    Ok(ExpectedDeclaration::from_parts(
        &args.service_key,
        &scope_keys,
        &platform_only,
    ))
}

fn build_behavior(args: &RunArgs, expected: &ExpectedDeclaration) -> Result<AcceptorBehavior> {
    match &args.reject {
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

fn resolve_scenarios(args: &RunArgs, default: &[Scenario]) -> Result<Vec<Scenario>> {
    match &args.scenarios {
        Some(raw) => Ok(parse_scenarios(raw)?),
        None => Ok(default.to_vec()),
    }
}

fn attach_target(args: &RunArgs) -> Result<AttachTarget> {
    let nats = args
        .nats
        .clone()
        .ok_or_else(|| CliError::new("--nats <URL> is required in attach mode"))?;
    let readyz = args
        .readyz
        .clone()
        .ok_or_else(|| CliError::new("--readyz <URL> is required in attach mode"))?;
    Ok(AttachTarget {
        nats_url: nats,
        readyz_url: readyz,
    })
}
