#[path = "common/cli.rs"]
mod cli;
#[path = "common/scratch.rs"]
mod scratch;

use std::path::{Path, PathBuf};

use cli::cli;
use conformance_scope::locate_subject;
use scratch::scratch;

const SERVICE_KEY: &str = "notifier";
const SCOPES: &str = "notifier:read,notifier:admin";

async fn bare_subject_in_a_working_directory() -> (PathBuf, PathBuf) {
    let reference = locate_subject()
        .await
        .expect("locate the reference subject");
    let dir = scratch("bare-subject");
    std::fs::copy(&reference, dir.join("subject")).expect("copy the subject");
    (reference, dir)
}

fn report_of(stdout: Vec<u8>) -> serde_json::Value {
    let text = String::from_utf8(stdout).expect("utf8 report");
    serde_json::from_str(&text).expect("json report")
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server`, `fabric-nats` and the `scope-service` binary on PATH"]
async fn spawn_of_a_bare_file_name_runs_the_file_of_the_working_directory() {
    let (_, dir) = bare_subject_in_a_working_directory().await;
    let cwd = dir.clone();

    let stdout = tokio::task::spawn_blocking(move || {
        cli()
            .current_dir(&cwd)
            .args([
                "run",
                "--spawn",
                "subject",
                "--service-key",
                SERVICE_KEY,
                "--scopes",
                SCOPES,
                "--scenarios",
                "s1",
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

    std::fs::remove_dir_all(&dir).ok();
    let report = report_of(stdout);
    assert_eq!(report["conformant"], serde_json::json!(true), "{report}");
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server`, `fabric-nats` and the `scope-service` binary on PATH"]
async fn a_manifest_with_a_bare_file_name_after_a_good_service_keeps_every_report() {
    let (reference, dir) = bare_subject_in_a_working_directory().await;
    let manifest = dir.join("services.yaml");
    std::fs::write(&manifest, services(&reference)).expect("write the manifest");
    let cwd = dir.clone();
    let file = manifest.to_string_lossy().to_string();

    let stdout = tokio::task::spawn_blocking(move || {
        cli()
            .current_dir(&cwd)
            .args(["manifest", &file, "--format", "json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone()
    })
    .await
    .expect("cli task");

    std::fs::remove_dir_all(&dir).ok();
    let report = report_of(stdout);
    assert_eq!(report["conformant"], serde_json::json!(true), "{report}");
    assert_eq!(
        report["services"].as_array().map(Vec::len),
        Some(2),
        "{report}"
    );
}

fn services(absolute: &Path) -> String {
    format!(
        "services:
  - service_key: {SERVICE_KEY}
    scenarios: s1
    scopes:
      - key: notifier:read
      - key: notifier:admin
    spawn:
      path: {}
  - service_key: {SERVICE_KEY}
    scenarios: s1
    scopes:
      - key: notifier:read
      - key: notifier:admin
    spawn:
      path: subject
",
        absolute.display()
    )
}
