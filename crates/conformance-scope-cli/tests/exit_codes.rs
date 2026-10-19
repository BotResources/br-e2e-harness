use std::path::{Path, PathBuf};

use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin;
use br_test_harness::BareFabricNats;
use conformance_scope::{ScopeHarness, Subject, SubjectConfig};

const SERVICE_KEY: &str = "notifier";
const SCOPES: &str = "notifier:read,notifier:admin";

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("{name}-{}", uuid::Uuid::now_v7().simple()));
    std::fs::create_dir_all(&dir).expect("create scratch directory");
    dir
}

fn cli() -> Command {
    Command::cargo_bin("conformance-scope").expect("cargo bin")
}

fn stderr_of(assert: assert_cmd::assert::Assert) -> String {
    String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr")
}

#[test]
fn attach_to_an_unreachable_broker_exits_2_naming_the_stream() {
    let assert = cli()
        .args([
            "run",
            "--nats",
            "nats://127.0.0.1:1",
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
        .code(2);
    let stderr = stderr_of(assert);
    assert!(stderr.contains("INTEGRATION_CMD"), "{stderr}");
}

#[test]
fn attach_without_nats_or_readyz_exits_2_naming_the_flag() {
    let missing_nats = stderr_of(
        cli()
            .args([
                "run",
                "--readyz",
                "http://127.0.0.1:1/readyz",
                "--service-key",
                SERVICE_KEY,
            ])
            .assert()
            .code(2),
    );
    assert!(missing_nats.contains("--nats"), "{missing_nats}");

    let missing_readyz = stderr_of(
        cli()
            .args([
                "run",
                "--nats",
                "nats://127.0.0.1:1",
                "--service-key",
                SERVICE_KEY,
            ])
            .assert()
            .code(2),
    );
    assert!(missing_readyz.contains("--readyz"), "{missing_readyz}");
}

#[test]
fn an_unknown_scenario_exits_2() {
    let stderr = stderr_of(
        cli()
            .args([
                "run",
                "--nats",
                "nats://127.0.0.1:1",
                "--readyz",
                "http://127.0.0.1:1/readyz",
                "--service-key",
                SERVICE_KEY,
                "--scenarios",
                "s9",
            ])
            .assert()
            .code(2),
    );
    assert!(stderr.contains("s9"), "{stderr}");
}

#[test]
fn spawn_of_a_binary_that_is_not_a_file_exits_2_naming_it() {
    let stderr = stderr_of(
        cli()
            .args([
                "run",
                "--spawn",
                "/nonexistent/service-binary",
                "--service-key",
                SERVICE_KEY,
                "--scopes",
                SCOPES,
            ])
            .assert()
            .code(2),
    );
    assert!(stderr.contains("/nonexistent/service-binary"), "{stderr}");
}

#[test]
fn spawn_without_nats_server_on_path_exits_2_naming_it() {
    let empty_path = scratch("empty-path");
    let stderr = stderr_of(
        cli()
            .env("PATH", &empty_path)
            .args([
                "run",
                "--spawn",
                &cargo_bin("conformance-scope").to_string_lossy(),
                "--service-key",
                SERVICE_KEY,
                "--scopes",
                SCOPES,
            ])
            .assert()
            .code(2),
    );
    std::fs::remove_dir_all(&empty_path).ok();
    assert!(stderr.contains("nats-server"), "{stderr}");
}

#[test]
fn a_manifest_that_cannot_be_read_or_parsed_exits_2() {
    let dir = scratch("manifest-refused");
    let absent = dir.join("absent.yaml");
    let unreadable = stderr_of(
        cli()
            .args(["manifest", &absent.to_string_lossy()])
            .assert()
            .code(2),
    );
    assert!(unreadable.contains("absent.yaml"), "{unreadable}");

    let malformed = dir.join("malformed.yaml");
    std::fs::write(&malformed, "services: [not, a, service]").expect("write manifest");
    let parsed = stderr_of(
        cli()
            .args(["manifest", &malformed.to_string_lossy()])
            .assert()
            .code(2),
    );
    assert!(parsed.contains("invalid manifest"), "{parsed}");
    std::fs::remove_dir_all(&dir).ok();
}

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

struct AttachedService {
    harness: ScopeHarness,
    subject: Subject,
    nats_url: String,
    readyz_url: String,
}

impl AttachedService {
    async fn boot() -> Self {
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
        Self {
            harness,
            subject,
            nats_url,
            readyz_url,
        }
    }

    async fn shutdown(self) {
        self.subject.shutdown().await;
        self.harness.shutdown().await;
    }
}

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
