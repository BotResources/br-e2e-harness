mod common;

use std::time::Duration;

use br_core_integration::{Actor, EventMetadata, IntegrationCommand, UserId};
use br_core_scope::{
    DeclareServiceScopes, ScopeDeclaration, ScopeKey, ScopeSpec, ServiceKey, ServiceManifest,
};
use br_scope_declaration_contract::declare_command_coords;
use br_test_harness::FabricTestNats;
use common::ReadyzStandIn;
use conformance_scope::{
    AcceptorBehavior, AttachTarget, CheckStatus, ExpectedDeclaration, ExpectedScope, Scenario,
    run_attach,
};
use uuid::Uuid;

const SERVICE_KEY: &str = "notifier";

fn expected() -> ExpectedDeclaration {
    ExpectedDeclaration::new(
        SERVICE_KEY,
        vec![ExpectedScope {
            key: "notifier:read".to_string(),
            platform_only: false,
        }],
    )
}

fn declare_with(correlation_id: Uuid) -> IntegrationCommand<DeclareServiceScopes> {
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
        EventMetadata::new(Actor::Human(UserId::from(Uuid::now_v7())), correlation_id),
        DeclareServiceScopes::new(declaration),
    )
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_service_that_mints_a_new_correlation_id_on_a_republish_is_not_blamed_on_a_stale_stream_alone()
 {
    let nats = FabricTestNats::start().await;
    let coords = declare_command_coords().expect("declare coordinates");
    for _ in 0..2 {
        nats.fabric()
            .publish_command(&coords, &declare_with(Uuid::now_v7()))
            .await
            .expect("publish a declare");
    }
    let stand_in = ReadyzStandIn::start(Some(Duration::ZERO)).await;
    let target = AttachTarget {
        nats_url: nats.url(),
        readyz_url: stand_in.url.clone(),
    };

    let report = run_attach(
        &target,
        &expected(),
        &AcceptorBehavior::Accept,
        &[
            Scenario::RepublishesSameCorrelationId,
            Scenario::ReadinessGated,
        ],
        Duration::from_secs(3),
    )
    .await
    .expect("attach run");

    nats.shutdown().await;
    assert_eq!(report.outcomes.len(), 2, "{report:?}");
    for outcome in &report.outcomes {
        assert_eq!(outcome.status, CheckStatus::Fail, "{report:?}");
        let detail = outcome.detail.clone().unwrap_or_default();
        assert!(
            detail.contains("2 distinct declares were captured"),
            "{detail}"
        );
        assert!(
            detail.contains("use a fresh INTEGRATION_CMD stream"),
            "{detail}"
        );
        assert!(
            detail.contains("minted a new correlation_id on a re-publish"),
            "{detail}"
        );
    }
    let s3 = &report.outcomes[0];
    assert!(
        s3.observed.contains("correlation_ids diverged"),
        "{report:?}"
    );
}
