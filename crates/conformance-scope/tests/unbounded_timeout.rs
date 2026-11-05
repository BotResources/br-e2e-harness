mod common;

use std::time::Duration;

use br_core_integration::{Actor, EventMetadata, IntegrationCommand, UserId};
use br_core_scope::{
    DeclareServiceScopes, ScopeDeclaration, ScopeKey, ScopeSpec, ServiceKey, ServiceManifest,
};
use br_scope_declaration_contract::declare_command_coords;
use br_test_harness::FabricTestNats;
use common::ReadyzStandIn;
use conformance_scope::checks::{CheckContext, run_scenario};
use conformance_scope::{
    AcceptorBehavior, CheckOutcome, DeclareCapture, ExpectedDeclaration, MAX_TIMEOUT, ReadyzProbe,
    Scenario,
};
use uuid::Uuid;

const SERVICE_KEY: &str = "notifier";

fn well_formed_declare() -> IntegrationCommand<DeclareServiceScopes> {
    let declaration = ScopeDeclaration::new(
        ServiceManifest::new(ServiceKey::new(SERVICE_KEY).unwrap(), "l", "d"),
        vec![ScopeSpec::new(
            ScopeKey::new("notifier:read").unwrap(),
            "l",
            "d",
            false,
        )],
    )
    .unwrap();
    IntegrationCommand::new(
        Uuid::now_v7(),
        "service_scope.declare",
        1,
        chrono::Utc::now(),
        EventMetadata::new(Actor::Human(UserId::from(Uuid::now_v7())), Uuid::now_v7()),
        DeclareServiceScopes::new(declaration),
    )
}

async fn run_with_timeout(
    scenario: Scenario,
    publish_declare: bool,
    timeout: Duration,
) -> CheckOutcome {
    let nats = FabricTestNats::start().await;
    let capture = DeclareCapture::start(&nats).await.expect("capture");
    if publish_declare {
        let coords = declare_command_coords().expect("declare coordinates");
        nats.fabric()
            .publish_command(&coords, &well_formed_declare())
            .await
            .expect("publish the declare");
    }
    let stand_in = ReadyzStandIn::start(None).await;
    let readyz = ReadyzProbe::new(stand_in.url.clone()).expect("readyz probe");
    let expected = ExpectedDeclaration::new(SERVICE_KEY, Vec::new());
    let key = ServiceKey::new(SERVICE_KEY).expect("service key");
    let behavior = AcceptorBehavior::Accept;
    let ctx = CheckContext {
        fabric: nats.fabric(),
        readyz: &readyz,
        capture: &capture,
        expected: &expected,
        service_key: &key,
        behavior: &behavior,
        timeout,
    };
    let outcome = run_scenario(scenario, &ctx).await;

    capture.stop().await;
    nats.shutdown().await;
    outcome
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_declare_wait_with_an_unbounded_context_timeout_is_clamped_instead_of_panicking() {
    for timeout in [Duration::MAX, MAX_TIMEOUT + Duration::from_nanos(1)] {
        let outcome = run_with_timeout(Scenario::DeclareWellFormed, true, timeout).await;

        assert!(outcome.is_pass(), "{timeout:?}: {outcome:?}");
    }
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_readyz_wait_with_an_unbounded_context_timeout_is_clamped_instead_of_panicking() {
    for timeout in [Duration::MAX, MAX_TIMEOUT + Duration::from_nanos(1)] {
        let outcome =
            run_with_timeout(Scenario::DisabledModeReadyWithoutDeclare, false, timeout).await;

        assert!(outcome.is_pass(), "{timeout:?}: {outcome:?}");
    }
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_confirmation_wait_with_an_unbounded_context_timeout_is_clamped_instead_of_panicking() {
    let outcome = run_with_timeout(
        Scenario::DuplicateConfirmationsTolerated,
        true,
        Duration::MAX,
    )
    .await;

    assert!(outcome.is_pass(), "{outcome:?}");
}
