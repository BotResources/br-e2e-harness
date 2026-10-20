#[path = "common/attached.rs"]
mod attached;
#[path = "common/cli.rs"]
mod cli;
#[path = "common/fixture.rs"]
mod fixture;

use attached::AttachedService;
use cli::cli;
use fixture::{SCOPES, SERVICE_KEY};

async fn attach_scenario(
    service: &AttachedService,
    scenario: &str,
    timeout: &str,
) -> std::process::Output {
    let args: Vec<String> = [
        "run",
        "--nats",
        &service.nats_url,
        "--readyz",
        &service.readyz_url,
        "--service-key",
        SERVICE_KEY,
        "--scopes",
        SCOPES,
        "--scenarios",
        scenario,
        "--timeout",
        timeout,
    ]
    .iter()
    .map(|arg| (*arg).to_string())
    .collect();
    tokio::task::spawn_blocking(move || {
        cli()
            .args(&args)
            .output()
            .expect("run the conformance-scope executable")
    })
    .await
    .expect("cli task")
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + the `scope-service` binary on PATH"]
async fn attach_s3_is_conformant_for_a_service_that_republishes_within_the_timeout() {
    let service = AttachedService::boot().await;
    let output = attach_scenario(&service, "s3", "10s").await;
    service.shutdown().await;
    assert_eq!(output.status.code(), Some(0), "{output:?}");
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + the `scope-service` binary on PATH"]
async fn attach_s6_is_conformant_for_a_service_booted_with_declaration_disabled() {
    let service = AttachedService::boot_with(|config| config.enabled(false)).await;
    let output = attach_scenario(&service, "s6", "10s").await;
    service.shutdown().await;
    assert_eq!(output.status.code(), Some(0), "{output:?}");
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + the `scope-service` binary on PATH"]
async fn attach_s6_against_a_service_booted_with_declaration_enabled_names_the_prerequisite() {
    let service = AttachedService::boot().await;
    let output = attach_scenario(&service, "s6", "2s").await;
    service.shutdown().await;
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[FAIL] s6"), "{stdout}");
    assert!(
        stdout.contains("booted with scope declaration disabled"),
        "{stdout}"
    );
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + the `scope-service` binary on PATH"]
async fn attach_s3_against_a_service_that_waits_longer_than_the_timeout_names_the_prerequisite() {
    let service = AttachedService::boot_with(|config| config.wait_timeout("30s")).await;
    let output = attach_scenario(&service, "s3", "2s").await;
    service.shutdown().await;
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[FAIL] s3"), "{stdout}");
    assert!(stdout.contains("--timeout above"), "{stdout}");
}
