use crate::endpoint::Resolution;
use crate::error::Result;
use crate::outcome::CheckOutcome;
use crate::scenario::Scenario;
use crate::seed::unknown_bearer;

use super::CheckContext;

pub async fn run_anonymous_scenario(scenario: Scenario, ctx: &CheckContext<'_>) -> CheckOutcome {
    let id = scenario.check_id();
    let expected = "anonymous(200, no X-Passport)";
    let resolution = match resolve(scenario, ctx).await {
        Ok(resolution) => resolution,
        Err(e) => {
            return CheckOutcome::fail(id, expected, "the endpoint call failed", format!("{e}"));
        }
    };

    match resolution {
        Resolution::Anonymous => CheckOutcome::pass(id, expected, resolution.label()),
        Resolution::Resolved(_) => CheckOutcome::fail(
            id,
            expected,
            resolution.label(),
            "the endpoint returned an X-Passport for a credential that must resolve to anonymous",
        ),
    }
}

async fn resolve(scenario: Scenario, ctx: &CheckContext<'_>) -> Result<Resolution> {
    match scenario {
        Scenario::RevokedBearerIsAnonymous => {
            let token = ctx.seeder.seed(ctx.namespace, "revoked").await?;
            ctx.seeder.revoke(&token).await?;
            ctx.endpoint.resolve_bearer(&token.raw).await
        }
        Scenario::UnknownBearerIsAnonymous => ctx.endpoint.resolve_bearer(&unknown_bearer()).await,
        Scenario::NoCredentialIsAnonymous => ctx.endpoint.resolve_anonymous().await,
        other => unreachable!("run_anonymous_scenario called with non-anonymous {other:?}"),
    }
}
