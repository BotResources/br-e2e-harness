mod anonymous;
mod fail_closed;
mod kv_error;
mod resolved;

use crate::endpoint::PassportEndpoint;
use crate::harness::PassportHarness;
use crate::outcome::CheckOutcome;
use crate::scenario::Scenario;
use crate::seal::{SealedSeed, SealedSeeder};
use crate::vectors::Vector;

use anonymous::run_anonymous_scenario;
use fail_closed::{
    run_tampered_ciphertext, run_tampered_nonce, run_unreadable_envelope, run_wrong_seal_key,
};
use kv_error::run_kv_error;
use resolved::{run_distinct_tokens, run_valid_bearer};

pub struct CheckContext<'a> {
    pub harness: &'a PassportHarness,
    pub seeder: &'a SealedSeeder,
    pub endpoint: &'a PassportEndpoint,
}

impl CheckContext<'_> {
    pub async fn seed(&self, vector: Vector) -> SealedSeed {
        self.seeder.seed(self.harness, vector).await
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
        Scenario::TamperedEnvelopeFailsClosed => run_tampered_ciphertext(ctx).await,
        Scenario::TamperedNonceFailsClosed => run_tampered_nonce(ctx).await,
        Scenario::UnreadableEnvelopeFailsClosed => run_unreadable_envelope(ctx).await,
        Scenario::KvErrorFailsLoud => run_kv_error(ctx).await,
    }
}
