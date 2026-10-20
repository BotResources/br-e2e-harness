#[path = "common/attached.rs"]
mod attached;
#[path = "common/cli.rs"]
mod cli;
#[path = "common/fixture.rs"]
mod fixture;
#[path = "common/scratch.rs"]
mod scratch;

use attached::AttachedService;
use cli::cli;
use fixture::{SCOPES, SERVICE_KEY};
use scratch::scratch;

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + the `scope-service` binary on PATH"]
async fn attach_run_against_a_conformant_service_exits_0_with_a_passing_json_report() {
    let service = AttachedService::boot().await;
    let (nats_url, readyz_url) = (service.nats_url.clone(), service.readyz_url.clone());

    let stdout = tokio::task::spawn_blocking(move || {
        cli()
            .args([
                "run",
                "--nats",
                &nats_url,
                "--readyz",
                &readyz_url,
                "--service-key",
                SERVICE_KEY,
                "--scopes",
                SCOPES,
                "--format",
                "json",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone()
    })
    .await
    .expect("cli task");
    service.shutdown().await;

    let report: serde_json::Value =
        serde_json::from_slice(&stdout).expect("the stdout is a JSON report");
    assert_eq!(report["conformant"], serde_json::json!(true), "{report}");
    assert_eq!(report["failed"], serde_json::json!(0), "{report}");
    assert_eq!(report["skipped"], serde_json::json!(0), "{report}");
    let passed: Vec<&str> = report["services"][0]["outcomes"]
        .as_array()
        .expect("outcomes")
        .iter()
        .filter(|o| o["status"] == serde_json::json!("pass"))
        .map(|o| o["check"].as_str().expect("check id"))
        .collect();
    for expected in ["s1", "s2", "declaration-content"] {
        assert!(passed.contains(&expected), "{expected} missing: {report}");
    }
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + the `scope-service` binary on PATH"]
async fn attach_run_is_conformant_whatever_the_order_of_the_scopes_flag() {
    let service = AttachedService::boot().await;
    let (nats_url, readyz_url) = (service.nats_url.clone(), service.readyz_url.clone());

    tokio::task::spawn_blocking(move || {
        cli()
            .args([
                "run",
                "--nats",
                &nats_url,
                "--readyz",
                &readyz_url,
                "--service-key",
                SERVICE_KEY,
                "--scopes",
                "notifier:admin,notifier:read",
            ])
            .assert()
            .success();
    })
    .await
    .expect("cli task");
    service.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + the `scope-service` binary on PATH"]
async fn manifest_run_aggregates_an_attached_service_into_the_output_file() {
    let service = AttachedService::boot().await;
    let dir = scratch("manifest-attach");
    let manifest = dir.join("services.yaml");
    std::fs::write(
        &manifest,
        format!(
            "services:\n  - service_key: {SERVICE_KEY}\n    scopes:\n      - key: notifier:read\n      - key: notifier:admin\n    attach:\n      nats: {}\n      readyz: {}\n",
            service.nats_url, service.readyz_url
        ),
    )
    .expect("write manifest");
    let report_path = dir.join("report.json");
    let (manifest_arg, report_arg) = (
        manifest.to_string_lossy().into_owned(),
        report_path.to_string_lossy().into_owned(),
    );

    let stdout = tokio::task::spawn_blocking(move || {
        cli()
            .args([
                "manifest",
                &manifest_arg,
                "--format",
                "json",
                "--output",
                &report_arg,
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone()
    })
    .await
    .expect("cli task");
    service.shutdown().await;

    assert!(stdout.is_empty(), "the report goes to the file, not stdout");
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&report_path).expect("report file"))
            .expect("the file holds a JSON report");
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(report["conformant"], serde_json::json!(true), "{report}");
    assert_eq!(
        report["services"][0]["service_key"],
        serde_json::json!(SERVICE_KEY),
        "{report}"
    );
}
