use std::fmt::Debug;
use std::time::Duration;

use conformance_identity::{
    ConformanceError, IdentityHarness, Result, Scenario, SpawnTarget, run_spawn,
};

fn assert_names_the_missing_server<T: Debug>(what: &str, result: Result<T>) {
    match result {
        Err(ConformanceError::Jetstream(detail)) => assert!(
            detail.contains("nats-server"),
            "{what}: the error must name the missing prerequisite: {detail}"
        ),
        other => panic!("{what}: a missing nats-server must be a typed error: {other:?}"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn spawn_returns_a_typed_error_when_nats_server_is_not_on_path() {
    let empty = std::env::temp_dir().join(format!("no-binaries-{}", uuid::Uuid::now_v7().simple()));
    std::fs::create_dir_all(&empty).expect("create an empty PATH directory");
    // SAFETY: this file holds a single test, so no other thread of this process reads or writes the environment while it runs.
    unsafe {
        std::env::set_var("PATH", &empty);
    }
    let binary = std::env::current_exe().expect("test executable");

    assert_names_the_missing_server(
        "start_with_binary",
        IdentityHarness::start_with_binary(binary.clone())
            .await
            .map(|_| ()),
    );
    assert_names_the_missing_server(
        "run_spawn",
        run_spawn(
            &SpawnTarget { binary },
            &[Scenario::CleanDeclarationAccepted],
            Duration::from_secs(2),
        )
        .await,
    );

    std::fs::remove_dir_all(&empty).ok();
}
