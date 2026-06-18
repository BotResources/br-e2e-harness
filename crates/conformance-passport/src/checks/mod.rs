mod anonymous;
mod resolved;

use crate::endpoint::PassportEndpoint;
use crate::outcome::CheckOutcome;
use crate::scenario::Scenario;
use br_test_harness::BearerSeeder;

use anonymous::run_anonymous_scenario;
use resolved::{run_distinct_tokens, run_valid_bearer};

pub struct CheckContext<'a> {
    pub seeder: &'a BearerSeeder,
    pub endpoint: &'a PassportEndpoint,
    pub namespace: &'a str,
}

pub async fn run_scenario(scenario: Scenario, ctx: &CheckContext<'_>) -> CheckOutcome {
    match scenario {
        Scenario::ValidBearerResolvesToPassport => run_valid_bearer(ctx).await,
        Scenario::DistinctTokensDistinctPassports => run_distinct_tokens(ctx).await,
        Scenario::RevokedBearerIsAnonymous
        | Scenario::UnknownBearerIsAnonymous
        | Scenario::NoCredentialIsAnonymous => run_anonymous_scenario(scenario, ctx).await,
    }
}
