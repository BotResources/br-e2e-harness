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
        &["a:read".to_string(), "b:read".to_string()],
        &PlatformOnly::PerScope(vec![("a:read".to_string(), true)]),
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
    assert!(detail.contains("a:read=true"), "{detail}");
    assert!(detail.contains("b:read=false"), "{detail}");
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
            assert!(detail.contains("INTEGRATION_CMD"), "{detail}");
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
async fn attach_to_a_broker_with_only_the_command_stream_is_a_typed_error_naming_the_event_stream()
{
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
            assert!(detail.contains("INTEGRATION_EVT"), "{detail}");
            assert!(detail.contains("must already exist"), "{detail}");
        }
        other => panic!("expected a typed Jetstream error: {other:?}"),
    }
    assert!(
        bare.event_stream_absent().await,
        "attach mode must not provision the event stream"
    );
}
