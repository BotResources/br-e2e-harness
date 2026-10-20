#[path = "common/absent_binary.rs"]
mod absent_binary;
#[path = "common/invalid_input.rs"]
mod invalid_input;
#[path = "common/mixed.rs"]
mod mixed;
#[path = "common/short_timeout.rs"]
mod short_timeout;
#[path = "common/uniform.rs"]
mod uniform;

use absent_binary::absent_binary;
use conformance_scope::{AcceptorBehavior, AttachTarget, Scenario, run_attach, run_spawn};
use invalid_input::invalid_input;
use mixed::mixed;
use short_timeout::TIMEOUT;
use uniform::uniform;

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
