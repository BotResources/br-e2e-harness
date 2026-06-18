use std::time::Duration;

use br_test_harness::wait_until;
use conformance_identity::checks::{CheckContext, run_scenario};
use conformance_identity::{
    ConfirmationCapture, IdentityHarness, ReadyzProbe, Scenario, Subject, SubjectConfig, provision,
};
use uuid::Uuid;

const SHORT: Duration = Duration::from_secs(10);

async fn provision_scope_declaration(harness: &IdentityHarness) {
    provision(&harness.nats_url(), "scope_declaration.toml")
        .await
        .expect("fabric-nats provision");
}

struct Fixture {
    harness: IdentityHarness,
    capture: ConfirmationCapture,
    subject: Subject,
    namespace: String,
}

impl Fixture {
    async fn start() -> Self {
        let harness = IdentityHarness::start().await.expect("harness");
        provision_scope_declaration(&harness).await;
        let capture = harness.capture_confirmations().await.expect("capture");
        let config = SubjectConfig::new(&harness.nats_url());
        let subject = Subject::spawn(harness.binary(), &config);
        let readyz = ReadyzProbe::new(format!("{}/readyz", subject.base_url())).expect("readyz");
        let ready = wait_until(SHORT, || async { readyz.is_ready().await }).await;
        assert!(
            ready,
            "acceptor must reach /readyz=200; logs:\n{}",
            subject.logs()
        );
        Self {
            harness,
            capture,
            subject,
            namespace: Uuid::now_v7().simple().to_string(),
        }
    }

    async fn run(&self, scenario: Scenario) -> conformance_identity::CheckOutcome {
        let declarer = self.harness.declarer();
        let ctx = CheckContext {
            declarer: &declarer,
            capture: &self.capture,
            namespace: &self.namespace,
            timeout: SHORT,
        };
        run_scenario(scenario, &ctx).await
    }

    async fn shutdown(self) {
        self.subject.shutdown().await;
        self.capture.stop().await;
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
async fn a1_clean_declaration_is_accepted() {
    assert_scenario(Scenario::CleanDeclarationAccepted).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn a2_owned_scope_reclaim_after_prior_accept_is_rejected() {
    assert_scenario(Scenario::OwnedScopeReclaimAfterPriorAccept).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn a3_intra_declaration_duplicate_is_rejected() {
    assert_scenario(Scenario::IntraDeclarationDuplicateRejected).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn a4_prefix_mismatch_is_rejected() {
    assert_scenario(Scenario::PrefixMismatchRejected).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn a5_invalid_scope_key_is_rejected() {
    assert_scenario(Scenario::InvalidScopeKeyRejected).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn a6_idempotent_redeclare_is_accepted() {
    assert_scenario(Scenario::IdempotentRedeclareAccepted).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn a7_malformed_scope_key_is_rejected() {
    assert_scenario(Scenario::MalformedScopeKeyRejected).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn full_battery_is_conformant_via_run_spawn() {
    use conformance_identity::{SpawnTarget, build_subject, run_spawn, spawn_default};
    let binary = build_subject().await.expect("build subject");
    let report = run_spawn(&SpawnTarget { binary }, &spawn_default(), SHORT)
        .await
        .expect("run_spawn");
    assert!(
        report.is_conformant(),
        "the full A1..A7 battery must be conformant: {} passed, {} failed, {} skipped\n{:#?}",
        report.passed(),
        report.failed(),
        report.skipped(),
        report.outcomes,
    );
    assert_eq!(report.passed(), 7, "all seven checks must pass");
}
