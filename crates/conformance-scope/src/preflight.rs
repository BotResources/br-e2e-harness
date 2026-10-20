use std::time::Duration;

use crate::error::{ConformanceError, Result};
use crate::expected::ExpectedDeclaration;
use crate::prerequisite;
use crate::readyz::ReadyzProbe;
use crate::runner::AttachTarget;
use crate::scenario::{Scenario, require_selection};
use crate::spawn::SpawnTarget;
use crate::timeout::validated_timeout;

pub fn check_attach_input(
    target: &AttachTarget,
    expected: &ExpectedDeclaration,
    scenarios: &[Scenario],
    timeout: Duration,
) -> Result<()> {
    validated_timeout(timeout)?;
    require_selection(scenarios)?;
    expected.validated_service_key()?;
    ReadyzProbe::new(&target.readyz_url)?;
    Ok(())
}

pub fn check_spawn_input(
    target: &SpawnTarget,
    expected: &ExpectedDeclaration,
    scenarios: &[Scenario],
    timeout: Duration,
) -> Result<()> {
    validated_timeout(timeout)?;
    require_selection(scenarios)?;
    expected.validated_service_key()?;
    uniform_platform_only(expected)?;
    prerequisite::subject_binary(&target.binary)?;
    Ok(())
}

pub(crate) fn uniform_platform_only(expected: &ExpectedDeclaration) -> Result<bool> {
    let Some(first) = expected.scopes.first() else {
        return Ok(false);
    };
    if expected
        .scopes
        .iter()
        .all(|scope| scope.platform_only == first.platform_only)
    {
        return Ok(first.platform_only);
    }
    let flags = expected
        .scopes
        .iter()
        .map(|scope| format!("{}={}", scope.key, scope.platform_only))
        .collect::<Vec<_>>()
        .join(", ");
    Err(ConformanceError::InvalidInput(format!(
        "spawn mode drives the subject with one platform_only flag for every scope, so an \
         expectation with mixed flags cannot be satisfied by any subject ({flags}); use attach \
         mode for a mixed declaration"
    )))
}
