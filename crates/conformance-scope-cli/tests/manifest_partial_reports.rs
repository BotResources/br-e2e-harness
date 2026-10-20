#[path = "common/cli.rs"]
mod cli;
#[path = "common/fixture.rs"]
mod fixture;
#[path = "common/scratch.rs"]
mod scratch;
#[path = "common/stderr.rs"]
mod stderr;

use cli::cli;
use conformance_scope::locate_subject;
use fixture::{SCOPES, SERVICE_KEY};
use scratch::scratch;
use stderr::stderr_of;

const UNREACHABLE_BROKER: &str = "nats://127.0.0.1:1";

fn manifest_of_a_good_spawn_then_an_unreachable_attach(subject: &std::path::Path) -> String {
    let scopes: String = SCOPES
        .split(',')
        .map(|key| format!("      - key: {key}\n"))
        .collect();
    format!(
        "services:
  - service_key: {SERVICE_KEY}
    scenarios: s1
    scopes:
{scopes}    spawn:
      path: {}
  - service_key: second_service
    scenarios: s1
    attach:
      nats: {UNREACHABLE_BROKER}
      readyz: http://127.0.0.1:1/readyz
",
        subject.display()
    )
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server`, `fabric-nats` and the `scope-service` binary on PATH"]
async fn a_service_that_cannot_run_after_a_good_one_keeps_the_report_of_the_good_one() {
    let subject = locate_subject()
        .await
        .expect("locate the reference subject");
    let dir = scratch("manifest-partial");
    let manifest = dir.join("services.yaml");
    std::fs::write(
        &manifest,
        manifest_of_a_good_spawn_then_an_unreachable_attach(&subject),
    )
    .expect("write the manifest");
    let report_path = dir.join("report.json");
    let (manifest_arg, report_arg) = (
        manifest.to_string_lossy().into_owned(),
        report_path.to_string_lossy().into_owned(),
    );

    let assert = tokio::task::spawn_blocking(move || {
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
            .code(2)
    })
    .await
    .expect("cli task");

    let stderr = stderr_of(assert);
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&report_path).expect("the report file exists"))
            .expect("the file holds a JSON report");
    std::fs::remove_dir_all(&dir).ok();
    assert!(stderr.contains("second_service"), "{stderr}");
    assert!(stderr.contains(UNREACHABLE_BROKER), "{stderr}");
    assert_eq!(
        report["services"].as_array().map(Vec::len),
        Some(1),
        "{report}"
    );
    assert_eq!(
        report["services"][0]["service_key"],
        serde_json::json!(SERVICE_KEY),
        "{report}"
    );
    assert_eq!(
        report["services"][0]["conformant"],
        serde_json::json!(true),
        "{report}"
    );
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server`, `fabric-nats` and the `scope-service` binary on PATH"]
async fn the_partial_report_goes_to_stdout_without_an_output_file() {
    let subject = locate_subject()
        .await
        .expect("locate the reference subject");
    let dir = scratch("manifest-partial-stdout");
    let manifest = dir.join("services.yaml");
    std::fs::write(
        &manifest,
        manifest_of_a_good_spawn_then_an_unreachable_attach(&subject),
    )
    .expect("write the manifest");
    let manifest_arg = manifest.to_string_lossy().into_owned();

    let output = tokio::task::spawn_blocking(move || {
        cli()
            .args(["manifest", &manifest_arg, "--format", "json"])
            .assert()
            .code(2)
            .get_output()
            .clone()
    })
    .await
    .expect("cli task");

    std::fs::remove_dir_all(&dir).ok();
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout holds the partial JSON report");
    assert_eq!(
        report["services"].as_array().map(Vec::len),
        Some(1),
        "{report}"
    );
}
