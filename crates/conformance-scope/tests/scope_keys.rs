#[path = "common/absent_binary.rs"]
mod absent_binary;
#[path = "common/invalid_input.rs"]
mod invalid_input;
#[path = "common/short_timeout.rs"]
mod short_timeout;

use absent_binary::absent_binary;
use conformance_scope::{
    AcceptorBehavior, AttachTarget, ConformanceError, ExpectedDeclaration, ExpectedScope,
    PlatformOnly, Scenario, run_attach, run_spawn,
};
use invalid_input::invalid_input;
use short_timeout::TIMEOUT;

fn expecting(service_key: &str, scope_keys: &[&str]) -> ExpectedDeclaration {
    ExpectedDeclaration::new(
        service_key,
        scope_keys
            .iter()
            .map(|key| ExpectedScope {
                key: (*key).to_string(),
                platform_only: false,
            })
            .collect(),
    )
}

const UNHOLDABLE: [(&[&str], &str); 4] = [
    (&["My-Service:Read", "my_service:admin"], "My-Service:Read"),
    (
        &["my_service:read", "other_service:read"],
        "other_service:read",
    ),
    (
        &["my_service:read", "my_service:read", "my_service:admin"],
        "my_service:read",
    ),
    (&["my_service:read:write"], "my_service:read:write"),
];

#[test]
fn validate_refuses_scope_keys_that_no_declaration_can_hold_naming_the_key() {
    for (scopes, culprit) in UNHOLDABLE {
        match expecting("my_service", scopes).validate() {
            Err(ConformanceError::InvalidInput(detail)) => {
                assert!(detail.contains(culprit), "{scopes:?}: {detail}");
            }
            other => panic!("{scopes:?}: expected InvalidInput: {other:?}"),
        }
    }
}

#[test]
fn validate_accepts_owned_distinct_scope_keys_and_an_empty_list() {
    assert!(
        expecting("my_service", &["my_service:read", "my_service:admin"])
            .validate()
            .is_ok()
    );
    assert!(expecting("my_service", &[]).validate().is_ok());
}

#[test]
fn validate_refuses_an_invalid_service_key_naming_it() {
    match expecting("My Service", &[]).validate() {
        Err(ConformanceError::InvalidInput(detail)) => {
            assert!(detail.contains("My Service"), "{detail}");
        }
        other => panic!("expected InvalidInput: {other:?}"),
    }
}

#[tokio::test]
async fn attach_refuses_unholdable_scope_keys_before_any_io() {
    let target = AttachTarget {
        nats_url: "nats://127.0.0.1:1".to_string(),
        readyz_url: "http://127.0.0.1:1/readyz".to_string(),
    };
    for (scopes, culprit) in UNHOLDABLE {
        let detail = invalid_input(
            run_attach(
                &target,
                &expecting("my_service", scopes),
                &AcceptorBehavior::Accept,
                &[Scenario::DeclarationContent],
                TIMEOUT,
            )
            .await,
        );
        assert!(detail.contains(culprit), "{scopes:?}: {detail}");
    }
}

#[tokio::test]
async fn spawn_refuses_unholdable_scope_keys_before_looking_at_the_binary() {
    for (scopes, culprit) in UNHOLDABLE {
        let detail = invalid_input(
            run_spawn(
                &absent_binary(),
                &expecting("my_service", scopes),
                &AcceptorBehavior::Accept,
                &[Scenario::DeclarationContent],
                TIMEOUT,
            )
            .await,
        );
        assert!(detail.contains(culprit), "{scopes:?}: {detail}");
        assert!(!detail.contains("/nonexistent"), "{detail}");
    }
}

fn scope_list(keys: &[&str]) -> Vec<String> {
    keys.iter().map(|key| (*key).to_string()).collect()
}

#[test]
fn try_from_parts_refuses_a_per_scope_key_that_names_no_expected_scope() {
    let platform_only = PlatformOnly::PerScope(vec![("my_service:admn".to_string(), true)]);
    match ExpectedDeclaration::try_from_parts(
        "my_service",
        &scope_list(&["my_service:read", "my_service:admin"]),
        &platform_only,
    ) {
        Err(ConformanceError::InvalidInput(detail)) => {
            assert!(detail.contains("my_service:admn"), "{detail}");
        }
        other => panic!("expected InvalidInput: {other:?}"),
    }
}

#[test]
fn try_from_parts_refuses_a_per_scope_key_that_appears_twice() {
    let platform_only = PlatformOnly::PerScope(vec![
        ("my_service:admin".to_string(), true),
        ("my_service:admin".to_string(), false),
    ]);
    match ExpectedDeclaration::try_from_parts(
        "my_service",
        &scope_list(&["my_service:read", "my_service:admin"]),
        &platform_only,
    ) {
        Err(ConformanceError::InvalidInput(detail)) => {
            assert!(detail.contains("my_service:admin"), "{detail}");
        }
        other => panic!("expected InvalidInput: {other:?}"),
    }
}

#[test]
fn try_from_parts_agrees_with_from_parts_on_valid_input() {
    let scopes = scope_list(&["my_service:read", "my_service:admin"]);
    for platform_only in [
        PlatformOnly::All(true),
        PlatformOnly::All(false),
        PlatformOnly::PerScope(vec![("my_service:admin".to_string(), true)]),
        PlatformOnly::PerScope(Vec::new()),
    ] {
        assert_eq!(
            ExpectedDeclaration::try_from_parts("my_service", &scopes, &platform_only)
                .expect("valid parts"),
            ExpectedDeclaration::from_parts("my_service", &scopes, &platform_only)
        );
    }
}
