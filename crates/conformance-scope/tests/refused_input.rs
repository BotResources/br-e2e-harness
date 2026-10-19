use std::path::PathBuf;
use std::time::Duration;

use conformance_scope::{
    AcceptorBehavior, AttachTarget, ConformanceError, ExpectedDeclaration, ExpectedScope,
    PlatformOnly, Scenario, SpawnTarget, run_attach, run_spawn,
};

const TIMEOUT: Duration = Duration::from_secs(1);

fn invalid_input(result: Result<conformance_scope::ConformanceReport, ConformanceError>) -> String {
    match result {
        Err(ConformanceError::InvalidInput(detail)) => detail,
        other => panic!("expected InvalidInput: {other:?}"),
    }
}

fn mixed() -> ExpectedDeclaration {
    ExpectedDeclaration::from_parts(
        "svc",
        &["svc:a".to_string(), "svc:b".to_string()],
        &PlatformOnly::PerScope(vec![("svc:a".to_string(), true)]),
    )
}

fn uniform() -> ExpectedDeclaration {
    ExpectedDeclaration::new(
        "svc",
        vec![ExpectedScope {
            key: "svc:read".to_string(),
            platform_only: false,
        }],
    )
}

fn absent_binary() -> SpawnTarget {
    SpawnTarget {
        binary: PathBuf::from("/nonexistent/scope-service"),
    }
}

#[tokio::test]
async fn spawn_refuses_an_expectation_whose_scopes_carry_different_platform_only_flags() {
    let detail = invalid_input(
        run_spawn(
            &absent_binary(),
            &mixed(),
            &AcceptorBehavior::Accept,
            &[Scenario::DeclarationContent],
            TIMEOUT,
        )
        .await,
    );
    assert!(detail.contains("svc:a=true"), "{detail}");
    assert!(detail.contains("svc:b=false"), "{detail}");
}

#[tokio::test]
async fn spawn_refuses_an_empty_scenario_selection() {
    let detail = invalid_input(
        run_spawn(
            &absent_binary(),
            &uniform(),
            &AcceptorBehavior::Accept,
            &[],
            TIMEOUT,
        )
        .await,
    );
    assert!(detail.contains("no scenarios"), "{detail}");
}

#[tokio::test]
async fn attach_refuses_an_empty_scenario_selection() {
    let target = AttachTarget {
        nats_url: "nats://127.0.0.1:1".to_string(),
        readyz_url: "http://127.0.0.1:1/readyz".to_string(),
    };
    let detail = invalid_input(
        run_attach(&target, &uniform(), &AcceptorBehavior::Accept, &[], TIMEOUT).await,
    );
    assert!(detail.contains("no scenarios"), "{detail}");
}

#[tokio::test]
async fn spawn_reports_a_binary_that_is_not_a_file_as_a_typed_input_error() {
    let detail = invalid_input(
        run_spawn(
            &absent_binary(),
            &uniform(),
            &AcceptorBehavior::Accept,
            &[Scenario::DeclareWellFormed],
            TIMEOUT,
        )
        .await,
    );
    assert!(detail.contains("/nonexistent/scope-service"), "{detail}");
}

#[tokio::test]
async fn attach_to_an_unreachable_broker_is_a_typed_error_not_a_panic() {
    let target = AttachTarget {
        nats_url: "nats://127.0.0.1:1".to_string(),
        readyz_url: "http://127.0.0.1:1/readyz".to_string(),
    };
    let started = std::time::Instant::now();
    let result = run_attach(
        &target,
        &uniform(),
        &AcceptorBehavior::Accept,
        &[Scenario::DeclareWellFormed],
        TIMEOUT,
    )
    .await;
    match result {
        Err(ConformanceError::Jetstream(detail)) => {
            assert!(detail.contains("nats://127.0.0.1:1"), "{detail}");
        }
        other => panic!("expected a typed Jetstream error: {other:?}"),
    }
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "{:?}",
        started.elapsed()
    );
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn attach_to_a_broker_without_the_handshake_stream_fails_loud_and_provisions_nothing() {
    let bare = br_test_harness::BareFabricNats::without_fixed_streams().await;
    let target = AttachTarget {
        nats_url: bare.url(),
        readyz_url: "http://127.0.0.1:1/readyz".to_string(),
    };
    let result = run_attach(
        &target,
        &uniform(),
        &AcceptorBehavior::Accept,
        &[Scenario::DeclareWellFormed],
        TIMEOUT,
    )
    .await;
    match result {
        Err(ConformanceError::Jetstream(detail)) => {
            assert!(detail.contains("must already exist"), "{detail}");
        }
        other => panic!("expected a typed Jetstream error: {other:?}"),
    }
    assert!(
        bare.command_stream_absent().await,
        "attach mode must not provision the command stream"
    );
    assert!(
        bare.event_stream_absent().await,
        "attach mode must not provision the event stream"
    );
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn attach_to_a_broker_with_only_the_command_stream_is_refused_and_creates_no_event_stream() {
    let bare = br_test_harness::BareFabricNats::with_only_command_stream().await;
    let target = AttachTarget {
        nats_url: bare.url(),
        readyz_url: "http://127.0.0.1:1/readyz".to_string(),
    };
    let result = run_attach(
        &target,
        &uniform(),
        &AcceptorBehavior::Accept,
        &[Scenario::DeclareWellFormed],
        TIMEOUT,
    )
    .await;
    match result {
        Err(ConformanceError::Jetstream(detail)) => {
            assert!(detail.contains("must already exist"), "{detail}");
        }
        other => panic!("expected a typed Jetstream error: {other:?}"),
    }
    assert!(
        bare.event_stream_absent().await,
        "attach mode must not provision the event stream"
    );
}

#[test]
fn a_readyz_url_that_is_not_an_http_url_is_invalid_input_naming_it() {
    for raw in [
        "127.0.0.1:8080/readyz",
        "not-a-url",
        "ftp://127.0.0.1/readyz",
        "http://",
    ] {
        match conformance_scope::ReadyzProbe::new(raw) {
            Err(ConformanceError::InvalidInput(detail)) => {
                assert!(detail.contains(raw), "{detail}");
            }
            Err(other) => panic!("`{raw}`: expected InvalidInput: {other:?}"),
            Ok(_) => panic!("`{raw}` must be refused"),
        }
    }
}

#[test]
fn a_readyz_url_with_an_http_or_https_host_is_accepted() {
    for raw in ["http://127.0.0.1:8080/readyz", "https://svc.example/readyz"] {
        assert!(conformance_scope::ReadyzProbe::new(raw).is_ok(), "{raw}");
    }
}

#[tokio::test]
async fn attach_with_a_scheme_less_readyz_url_is_invalid_input_before_touching_the_broker() {
    let target = AttachTarget {
        nats_url: "nats://127.0.0.1:1".to_string(),
        readyz_url: "127.0.0.1:8080/readyz".to_string(),
    };
    let detail = invalid_input(
        run_attach(
            &target,
            &uniform(),
            &AcceptorBehavior::Accept,
            &[Scenario::DeclareWellFormed],
            TIMEOUT,
        )
        .await,
    );
    assert!(detail.contains("127.0.0.1:8080/readyz"), "{detail}");
}

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
