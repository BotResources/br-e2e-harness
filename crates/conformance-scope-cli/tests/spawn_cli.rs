use assert_cmd::Command;
use conformance_scope::{ScopeHarness, Subject, SubjectConfig, build_subject};

const SERVICE_KEY: &str = "notifier";
const SCOPES: &str = "notifier:read,notifier:admin";

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn spawn_run_is_conformant_against_the_reference_subject() {
    let binary = build_subject().await.expect("build the reference subject");

    let assert = tokio::task::spawn_blocking(move || {
        Command::cargo_bin("conformance-scope")
            .expect("cargo bin")
            .args([
                "run",
                "--spawn",
                &binary.to_string_lossy(),
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

    let stdout = String::from_utf8(assert).expect("utf8 report");
    let report: serde_json::Value = serde_json::from_str(&stdout).expect("json report");
    assert_eq!(report["conformant"], serde_json::json!(true), "{stdout}");
    assert_eq!(report["failed"], serde_json::json!(0), "{stdout}");
    assert_eq!(report["skipped"], serde_json::json!(0), "{stdout}");

    let outcomes = report["services"][0]["outcomes"]
        .as_array()
        .expect("outcomes array");
    let s4 = outcomes
        .iter()
        .find(|o| o["check"] == serde_json::json!("s4"))
        .expect("s4 must be in the spawn battery");
    assert_eq!(
        s4["status"],
        serde_json::json!("pass"),
        "s4 must run as a real rejection under the default --accept, not be skipped: {stdout}"
    );
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn attach_run_flags_wrong_scopes_with_a_diff() {
    let harness = ScopeHarness::start().await.expect("harness");
    let nats_url = harness.nats_url();
    let config = SubjectConfig::new(
        &nats_url,
        harness.stream_name(),
        harness.event_stream_name(),
        SERVICE_KEY,
    )
    .scope_keys(SCOPES)
    .label_key("label.notifier")
    .description_key("desc.notifier")
    .wait_timeout("500ms");
    let subject = Subject::spawn(harness.binary(), &config);
    let readyz_url = format!("{}/readyz", subject.base_url());

    let output = tokio::task::spawn_blocking(move || {
        let assert = Command::cargo_bin("conformance-scope")
            .expect("cargo bin")
            .args([
                "run",
                "--attach",
                "--nats",
                &nats_url,
                "--readyz",
                &readyz_url,
                "--service-key",
                SERVICE_KEY,
                "--scopes",
                "notifier:read",
                "--scenarios",
                "declaration-content",
                "--format",
                "human",
            ])
            .assert()
            .failure();
        assert.get_output().stdout.clone()
    })
    .await
    .expect("cli task");

    subject.shutdown().await;
    harness.shutdown().await;

    let stdout = String::from_utf8(output).expect("utf8 report");
    assert!(
        stdout.contains("[FAIL] declaration-content"),
        "the report must mark declaration-content as failed:\n{stdout}"
    );
    assert!(
        stdout.contains("scope set mismatch"),
        "the failure must read as an expected-vs-observed diff:\n{stdout}"
    );
    assert!(
        stdout.contains("NON-CONFORMANT"),
        "the verdict must be NON-CONFORMANT:\n{stdout}"
    );
}
