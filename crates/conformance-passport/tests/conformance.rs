use std::time::Duration;

use br_test_harness::wait_until;
use conformance_passport::checks::{CheckContext, run_scenario};
use conformance_passport::{
    PassportEndpoint, PassportHarness, ReadyzProbe, Scenario, SealedSeeder, Subject, SubjectConfig,
};

const SHORT: Duration = Duration::from_secs(10);

struct Fixture {
    harness: PassportHarness,
    seeder: SealedSeeder,
    endpoint: PassportEndpoint,
    subject: Subject,
}

impl Fixture {
    async fn start() -> Self {
        let harness = PassportHarness::start().await.expect("harness");
        let seeder = harness.seeder();
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
        }
    }

    async fn run(&self, scenario: Scenario) -> conformance_passport::CheckOutcome {
        let ctx = CheckContext {
            harness: &self.harness,
            seeder: &self.seeder,
            endpoint: &self.endpoint,
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
#[ignore = "real-infra: needs `nats-server` + `identity-passport` on PATH"]
async fn p1_valid_bearer_resolves_to_passport() {
    assert_scenario(Scenario::ValidBearerResolvesToPassport).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `identity-passport` on PATH"]
async fn p2_revoked_bearer_is_anonymous() {
    assert_scenario(Scenario::RevokedBearerIsAnonymous).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `identity-passport` on PATH"]
async fn p3_unknown_bearer_is_anonymous() {
    assert_scenario(Scenario::UnknownBearerIsAnonymous).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `identity-passport` on PATH"]
async fn p4_no_credential_is_anonymous() {
    assert_scenario(Scenario::NoCredentialIsAnonymous).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `identity-passport` on PATH"]
async fn p5_distinct_tokens_distinct_passports() {
    assert_scenario(Scenario::DistinctTokensDistinctPassports).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `identity-passport` on PATH"]
async fn p6_wrong_seal_key_fails_closed() {
    assert_scenario(Scenario::WrongSealKeyFailsClosed).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `identity-passport` on PATH"]
async fn p7_tampered_envelope_fails_closed() {
    assert_scenario(Scenario::TamperedEnvelopeFailsClosed).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `identity-passport` on PATH"]
async fn p10_tampered_nonce_fails_closed() {
    assert_scenario(Scenario::TamperedNonceFailsClosed).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `identity-passport` on PATH"]
async fn p9_unreadable_envelope_fails_closed() {
    assert_scenario(Scenario::UnreadableEnvelopeFailsClosed).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `identity-passport` on PATH"]
async fn p8_kv_error_fails_loud() {
    assert_scenario(Scenario::KvErrorFailsLoud).await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `identity-passport` on PATH"]
async fn full_battery_is_conformant_via_run_spawn() {
    use conformance_passport::{ALL, SpawnTarget, locate_subject, run_spawn};
    let binary = locate_subject()
        .await
        .expect("locate the identity-passport binary");
    let report = run_spawn(&SpawnTarget { binary }, &ALL, SHORT)
        .await
        .expect("run_spawn");
    assert!(
        report.is_conformant(),
        "the full battery must be conformant: {} passed, {} failed, {} skipped\n{:#?}",
        report.passed(),
        report.failed(),
        report.skipped(),
        report.outcomes,
    );
    assert_eq!(report.passed(), ALL.len(), "every spawn check must pass");
}

fn non_loopback_address() -> std::net::IpAddr {
    let probe = std::net::UdpSocket::bind("0.0.0.0:0").expect("bind a probe socket");
    probe
        .connect("192.0.2.1:9")
        .expect("the host needs a route to find its non-loopback address");
    let address = probe.local_addr().expect("probe address").ip();
    assert!(
        !address.is_loopback(),
        "the host has no non-loopback address to probe: {address}"
    );
    address
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `identity-passport` on PATH"]
async fn the_subject_does_not_answer_on_a_non_loopback_address() {
    let fixture = Fixture::start().await;
    let port: u16 = fixture
        .subject
        .base_url()
        .rsplit(':')
        .next()
        .expect("port")
        .parse()
        .expect("numeric port");
    let external = non_loopback_address();
    let reached = tokio::time::timeout(
        Duration::from_secs(3),
        tokio::net::TcpStream::connect((external, port)),
    )
    .await;
    let logs = fixture.subject.logs();
    fixture.shutdown().await;
    assert!(
        !matches!(reached, Ok(Ok(_))),
        "the subject answered on {external}:{port}, it must listen on 127.0.0.1 only; logs:\n{logs}"
    );
}
