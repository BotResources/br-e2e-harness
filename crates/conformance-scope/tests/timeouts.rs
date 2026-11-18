#[path = "common/absent_binary.rs"]
mod absent_binary;
#[path = "common/invalid_input.rs"]
mod invalid_input;
#[path = "common/uniform.rs"]
mod uniform;

use std::time::Duration;

use absent_binary::absent_binary;
use conformance_scope::{
    AcceptorBehavior, AttachTarget, ConformanceError, MAX_TIMEOUT, Scenario, run_attach, run_spawn,
};
use invalid_input::invalid_input;
use uniform::uniform;

fn unservable_timeouts() -> [Duration; 5] {
    [
        Duration::ZERO,
        MAX_TIMEOUT + Duration::from_nanos(1),
        Duration::from_secs(u64::MAX),
        Duration::MAX,
        Duration::from_secs(MAX_TIMEOUT.as_secs() * 365 * 1000),
    ]
}

#[tokio::test]
async fn attach_refuses_a_timeout_no_service_can_satisfy_before_any_io() {
    let target = AttachTarget {
        nats_url: "nats://127.0.0.1:1".to_string(),
        readyz_url: "http://127.0.0.1:1/readyz".to_string(),
    };
    for timeout in unservable_timeouts() {
        let detail = invalid_input(
            run_attach(
                &target,
                &uniform(),
                &AcceptorBehavior::Accept,
                &[Scenario::DeclareWellFormed],
                timeout,
            )
            .await,
        );
        assert!(detail.contains(&format!("{timeout:?}")), "{detail}");
        assert!(!detail.contains("127.0.0.1"), "{detail}");
    }
}

#[tokio::test]
async fn spawn_refuses_a_timeout_no_service_can_satisfy_before_looking_at_the_binary() {
    for timeout in unservable_timeouts() {
        let detail = invalid_input(
            run_spawn(
                &absent_binary(),
                &uniform(),
                &AcceptorBehavior::Accept,
                &[Scenario::DeclareWellFormed],
                timeout,
            )
            .await,
        );
        assert!(detail.contains(&format!("{timeout:?}")), "{detail}");
        assert!(!detail.contains("/nonexistent"), "{detail}");
    }
}

#[tokio::test]
async fn attach_and_spawn_accept_the_cap_itself_as_a_timeout() {
    let target = AttachTarget {
        nats_url: "nats://127.0.0.1:1".to_string(),
        readyz_url: "http://127.0.0.1:1/readyz".to_string(),
    };
    let attach = run_attach(
        &target,
        &uniform(),
        &AcceptorBehavior::Accept,
        &[Scenario::DeclareWellFormed],
        MAX_TIMEOUT,
    )
    .await;
    assert!(
        matches!(attach, Err(ConformanceError::Jetstream(_))),
        "{attach:?}"
    );
    let spawn = run_spawn(
        &absent_binary(),
        &uniform(),
        &AcceptorBehavior::Accept,
        &[Scenario::DeclareWellFormed],
        MAX_TIMEOUT,
    )
    .await;
    let detail = invalid_input(spawn);
    assert!(detail.contains("/nonexistent/scope-service"), "{detail}");
}
