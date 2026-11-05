#[path = "common/cli.rs"]
mod cli;
#[path = "common/fixture.rs"]
mod fixture;
#[path = "common/scratch.rs"]
mod scratch;
#[path = "common/stderr.rs"]
mod stderr;

use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin;
use br_test_harness::BareFabricNats;
use cli::cli;
use fixture::{SCOPES, SERVICE_KEY};
use scratch::scratch;
use stderr::stderr_of;

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn attach_to_a_broker_without_the_handshake_streams_exits_2_and_creates_none() {
    let bare = BareFabricNats::without_fixed_streams().await;
    let nats_url = bare.url();

    let stderr = tokio::task::spawn_blocking(move || {
        stderr_of(
            cli()
                .args([
                    "run",
                    "--nats",
                    &nats_url,
                    "--readyz",
                    "http://127.0.0.1:1/readyz",
                    "--service-key",
                    SERVICE_KEY,
                    "--scopes",
                    SCOPES,
                    "--timeout",
                    "1s",
                ])
                .assert()
                .code(2),
        )
    })
    .await
    .expect("cli task");

    assert!(stderr.contains("must already exist"), "{stderr}");
    assert!(bare.command_stream_absent().await, "{stderr}");
    assert!(bare.event_stream_absent().await, "{stderr}");
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn attach_to_a_broker_with_only_the_command_stream_exits_2_and_creates_no_event_stream() {
    let bare = BareFabricNats::with_only_command_stream().await;
    let nats_url = bare.url();

    let stderr = tokio::task::spawn_blocking(move || {
        stderr_of(
            cli()
                .args([
                    "run",
                    "--nats",
                    &nats_url,
                    "--readyz",
                    "http://127.0.0.1:1/readyz",
                    "--service-key",
                    SERVICE_KEY,
                    "--scopes",
                    SCOPES,
                    "--timeout",
                    "1s",
                ])
                .assert()
                .code(2),
        )
    })
    .await
    .expect("cli task");

    assert!(stderr.contains("must already exist"), "{stderr}");
    assert!(bare.event_stream_absent().await, "{stderr}");
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn spawn_from_a_released_layout_without_fabric_nats_exits_2_naming_it() {
    let released = scratch("released-layout");
    let executable = released.join("conformance-scope");
    std::fs::copy(cargo_bin("conformance-scope"), &executable).expect("copy the executable");
    let binary = cargo_bin("conformance-scope");

    let stderr = tokio::task::spawn_blocking(move || {
        stderr_of(
            Command::new(&executable)
                .args([
                    "run",
                    "--spawn",
                    &binary.to_string_lossy(),
                    "--service-key",
                    SERVICE_KEY,
                    "--scopes",
                    SCOPES,
                ])
                .assert()
                .code(2),
        )
    })
    .await
    .expect("cli task");

    std::fs::remove_dir_all(&released).ok();
    assert!(stderr.contains("fabric-nats"), "{stderr}");
    assert!(stderr.contains("attach mode"), "{stderr}");
}
