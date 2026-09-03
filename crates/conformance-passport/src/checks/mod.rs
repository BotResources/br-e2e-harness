mod anonymous;
mod fail_closed;
mod kv_error;
mod resolved;

use crate::endpoint::PassportEndpoint;
use crate::error::Result;
use crate::harness::PassportHarness;
use crate::outcome::CheckOutcome;
use crate::scenario::Scenario;
use crate::seal::{SealedSeed, SealedSeeder};

use anonymous::run_anonymous_scenario;
use fail_closed::{run_tampered_envelope, run_unreadable_envelope, run_wrong_seal_key};
use kv_error::run_kv_error;
use resolved::{run_distinct_tokens, run_valid_bearer};

pub struct CheckContext<'a> {
    pub harness: &'a PassportHarness,
    pub seeder: &'a SealedSeeder,
    pub endpoint: &'a PassportEndpoint,
    pub namespace: &'a str,
}

impl CheckContext<'_> {
    pub async fn seed(&self, label: &str) -> Result<SealedSeed> {
        self.seeder.seed(self.harness, self.namespace, label).await
    }
}

pub async fn run_scenario(scenario: Scenario, ctx: &CheckContext<'_>) -> CheckOutcome {
    match scenario {
        Scenario::ValidBearerResolvesToPassport => run_valid_bearer(ctx).await,
        Scenario::DistinctTokensDistinctPassports => run_distinct_tokens(ctx).await,
        Scenario::RevokedBearerIsAnonymous
        | Scenario::UnknownBearerIsAnonymous
        | Scenario::NoCredentialIsAnonymous => run_anonymous_scenario(scenario, ctx).await,
        Scenario::WrongSealKeyFailsClosed => run_wrong_seal_key(ctx).await,
        Scenario::TamperedEnvelopeFailsClosed => run_tampered_envelope(ctx).await,
        Scenario::UnreadableEnvelopeFailsClosed => run_unreadable_envelope(ctx).await,
        Scenario::KvErrorFailsLoud => run_kv_error(ctx).await,
    }
}
