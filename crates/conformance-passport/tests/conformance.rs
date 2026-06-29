use std::time::Duration;

use br_test_harness::wait_until;
use conformance_passport::checks::{CheckContext, run_scenario};
use conformance_passport::{
    PassportEndpoint, PassportHarness, ReadyzProbe, Scenario, SealedSeeder, Subject, SubjectConfig,
};
use uuid::Uuid;

const SHORT: Duration = Duration::from_secs(10);

struct Fixture {
    harness: PassportHarness,
    seeder: SealedSeeder,
    endpoint: PassportEndpoint,
    subject: Subject,
    namespace: String,
}

impl Fixture {
    async fn start() -> Self {
        let harness = PassportHarness::start().await.expect("harness");
        let seeder = harness.seeder().await.expect("sealed seeder");
        let config = SubjectConfig::new(&harness.nats_url());
        let subject = Subject::spawn(harness.binary(), &config);
        let readyz = ReadyzProbe::new(format!("{}/readyz", subject.base_url())).expect("readyz");
        let ready = wait_until(SHORT, || async { readyz.is_ready().await }).await;
        assert!(
            ready,
            "passport subject must reach /readyz=200; logs:\n{}",
            subject.logs()
        );
        let endpoint = PassportEndpoint::new(subject.base_url()).expect("endpoint");
        Self {
            harness,
            seeder,
            endpoint,
            subject,
            namespace: Uuid::now_v7().simple().to_string(),
        }
    }

    async fn run(&self, scenario: Scenario) -> conformance_passport::CheckOutcome {
        let ctx = CheckContext {
            harness: &self.harness,
            seeder: &self.seeder,
            endpoint: &self.endpoint,
            namespace: &self.namespace,
        };
        run_scenario(scenario, &ctx).await
    }

    async fn shutdown(self) {
        self.subject.shutdown().await;
        self.harness.shutdown().await;
    }
}

async fn assert_scenario(scenario: Scenario) {
    let fixture = Fixture::start().await;
    let outcome = fixture.run(scenario).await;
    let logs = fixture.subject.logs();
    let pass = outcome.is_pass();
    fixture.shutdown().await;
    assert!(
        pass,
        "{} must pass: {outcome:?}; logs:\n{logs}",
        scenario.code()
    );
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn p1_valid_bearer_resolves_to_passport() {
    assert_scenario(Scenario::ValidBearerResolvesToPassport).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn p2_revoked_bearer_is_anonymous() {
    assert_scenario(Scenario::RevokedBearerIsAnonymous).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn p3_unknown_bearer_is_anonymous() {
    assert_scenario(Scenario::UnknownBearerIsAnonymous).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn p4_no_credential_is_anonymous() {
    assert_scenario(Scenario::NoCredentialIsAnonymous).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn p5_distinct_tokens_distinct_passports() {
    assert_scenario(Scenario::DistinctTokensDistinctPassports).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn p6_wrong_seal_key_fails_closed() {
    assert_scenario(Scenario::WrongSealKeyFailsClosed).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn p7_tampered_envelope_fails_closed() {
    assert_scenario(Scenario::TamperedEnvelopeFailsClosed).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn p8_kv_error_fails_loud() {
    assert_scenario(Scenario::KvErrorFailsLoud).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn full_battery_is_conformant_via_run_spawn() {
    use conformance_passport::{ALL, SpawnTarget, build_subject, run_spawn};
    let binary = build_subject().await.expect("build subject");
    let report = run_spawn(&SpawnTarget { binary }, &ALL, SHORT)
        .await
        .expect("run_spawn");
    assert!(
        report.is_conformant(),
        "the full P1..P8 battery must be conformant: {} passed, {} failed, {} skipped\n{:#?}",
        report.passed(),
        report.failed(),
        report.skipped(),
        report.outcomes,
    );
    assert_eq!(report.passed(), 8, "all eight checks must pass");
}
