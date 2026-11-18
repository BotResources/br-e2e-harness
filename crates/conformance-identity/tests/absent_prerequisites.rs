use std::fmt::Debug;
use std::time::Duration;

use conformance_identity::{
    AttachTarget, ConformanceError, IdentityHarness, Result, Scenario, SpawnTarget, run_attach,
    run_spawn,
};

const SHORT: Duration = Duration::from_secs(2);

fn free_loopback_url() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve a port");
    let port = listener.local_addr().expect("local address").port();
    format!("127.0.0.1:{port}")
}

fn assert_invalid_input<T: Debug>(what: &str, result: Result<T>) {
    match result {
        Err(ConformanceError::InvalidInput(_)) => {}
        other => panic!("{what}: expected a typed input error: {other:?}"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn selecting_no_scenario_is_a_typed_error_not_a_green_report() {
    let spawn = run_spawn(
        &SpawnTarget {
            binary: std::env::current_exe().expect("test executable"),
        },
        &[],
        SHORT,
    )
    .await;
    assert_invalid_input("run_spawn", spawn);

    let attach = run_attach(
        &AttachTarget {
            nats_url: format!("nats://{}", free_loopback_url()),
            readyz_url: format!("http://{}/readyz", free_loopback_url()),
        },
        &[],
        SHORT,
    )
    .await;
    assert_invalid_input("run_attach", attach);
}

#[tokio::test(flavor = "current_thread")]
async fn a_missing_acceptor_binary_is_a_typed_error() {
    let absent = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("absent-{}", uuid::Uuid::now_v7().simple()));
    assert_invalid_input(
        "start_with_binary",
        IdentityHarness::start_with_binary(absent.clone())
            .await
            .map(|_| ()),
    );
    assert_invalid_input(
        "run_spawn",
        run_spawn(
            &SpawnTarget { binary: absent },
            &[Scenario::CleanDeclarationAccepted],
            SHORT,
        )
        .await,
    );
}

#[tokio::test(flavor = "current_thread")]
async fn an_unreachable_nats_is_a_typed_error_not_a_panic() {
    let result = run_attach(
        &AttachTarget {
            nats_url: format!("nats://{}", free_loopback_url()),
            readyz_url: format!("http://{}/readyz", free_loopback_url()),
        },
        &[Scenario::CleanDeclarationAccepted],
        SHORT,
    )
    .await;
    match result {
        Err(ConformanceError::Jetstream(detail)) => {
            assert!(
                detail.contains("NATS"),
                "the error must name NATS: {detail}"
            );
        }
        other => panic!("an unreachable NATS must be a typed error: {other:?}"),
    }
}
