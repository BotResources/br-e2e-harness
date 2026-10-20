use assert_cmd::Command;
use br_test_harness::{BareFabricNats, FabricTestNats};

mod common;

use common::{Scratch, strings};

const BIN: &str = "conformance-nats-fabric";

fn run(args: Vec<String>) -> (Option<i32>, serde_json::Value, String) {
    let output = Command::cargo_bin(BIN)
        .expect("cargo bin")
        .args(&args)
        .output()
        .expect("run the cli");
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let report = serde_json::from_str(&stdout).unwrap_or(serde_json::Value::Null);
    (
        output.status.code(),
        report,
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

async fn run_blocking(args: Vec<String>) -> (Option<i32>, serde_json::Value, String) {
    tokio::task::spawn_blocking(move || run(args))
        .await
        .expect("cli task")
}

fn status_of(report: &serde_json::Value, check: &str) -> String {
    report["outcomes"]
        .as_array()
        .expect("outcomes")
        .iter()
        .find(|o| o["check"] == check)
        .unwrap_or_else(|| panic!("{check} missing from {report}"))["status"]
        .as_str()
        .expect("status")
        .to_string()
}

#[tokio::test]
#[ignore = "requires `nats-server` and the prebuilt `nats-fabric` anchor on PATH"]
async fn spawn_run_is_conformant_on_every_check() {
    let (code, report, stderr) = run_blocking(strings(&["run", "--format", "json"])).await;
    assert_eq!(code, Some(0), "{report}\n{stderr}");
    assert_eq!(report["mode"], "spawn");
    assert_eq!(report["conformant"], true, "{report}");
    assert_eq!(report["passed"], 12, "{report}");
    assert_eq!(report["failed"], 0, "{report}");
    assert_eq!(report["skipped"], 0, "{report}");
}

#[tokio::test]
#[ignore = "requires `nats-server` and the prebuilt `nats-fabric` anchor on PATH"]
async fn attach_run_passes_on_a_provisioned_server_without_the_spawn_only_checks_unless_selected() {
    let harness = FabricTestNats::start()
        .await
        .with_published_language()
        .await;
    let url = harness.url();

    let (code, report, stderr) =
        run_blocking(strings(&["run", "--nats", &url, "--format", "json"])).await;
    assert_eq!(code, Some(0), "{report}\n{stderr}");
    assert_eq!(report["mode"], "attach");
    assert_eq!(report["passed"], 10, "{report}");
    assert_eq!(report["skipped"], 0, "{report}");

    let (code, report, stderr) = run_blocking(strings(&[
        "run",
        "--nats",
        &url,
        "--checks",
        "missing-stream-fails-coverage-probe,dead-grammar-fails-loud",
        "--format",
        "json",
    ]))
    .await;
    assert_eq!(code, Some(0), "{report}\n{stderr}");
    assert_eq!(
        status_of(&report, "missing-stream-fails-coverage-probe"),
        "skipped"
    );
    assert_eq!(status_of(&report, "dead-grammar-fails-loud"), "pass");
    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "requires `nats-server` and the prebuilt `nats-fabric` anchor on PATH"]
async fn attach_run_to_a_server_without_the_fixed_streams_is_non_conformant_and_creates_nothing() {
    let bare = BareFabricNats::without_fixed_streams().await;
    let url = bare.url();

    let output = tokio::task::spawn_blocking(move || {
        Command::cargo_bin(BIN)
            .expect("cargo bin")
            .args(["run", "--nats", &url, "--checks", "dead-grammar-fails-loud"])
            .assert()
            .code(1)
            .get_output()
            .stdout
            .clone()
    })
    .await
    .expect("cli task");

    let stdout = String::from_utf8(output).expect("utf8 report");
    assert!(
        stdout.contains("[FAIL] dead-grammar-fails-loud"),
        "{stdout}"
    );
    assert!(stdout.contains("must exist"), "{stdout}");
    assert!(stdout.contains("NON-CONFORMANT"), "{stdout}");
    assert!(bare.command_stream_absent().await);
    assert!(bare.event_stream_absent().await);
    bare.shutdown().await;
}

#[test]
fn a_missing_anchor_is_a_typed_error_naming_it_and_exit_code_two() {
    let scratch = Scratch::new("nf-cli-empty");
    let empty = &scratch.0;

    let assert = Command::cargo_bin(BIN)
        .expect("cargo bin")
        .env_clear()
        .env("PATH", empty)
        .env("XDG_CACHE_HOME", empty)
        .env("HTTPS_PROXY", "http://127.0.0.1:1")
        .env("https_proxy", "http://127.0.0.1:1")
        .args(["run", "--checks", "anchor-subjects-match-lib"])
        .assert()
        .code(2);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();

    assert!(
        stderr.contains("the `nats-fabric` binary is not available"),
        "{stderr}"
    );
}

#[test]
fn an_unknown_check_id_is_a_usage_error_naming_it() {
    let assert = Command::cargo_bin(BIN)
        .expect("cargo bin")
        .args(["run", "--checks", "no-such-check"])
        .assert()
        .code(2);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(stderr.contains("unknown check `no-such-check`"), "{stderr}");
}

#[test]
fn list_names_every_check_and_marks_the_spawn_only_ones() {
    let assert = Command::cargo_bin(BIN)
        .expect("cargo bin")
        .arg("list")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert_eq!(stdout.matches("[spawn").count(), 12, "{stdout}");
    assert!(
        stdout.contains("missing-stream-fails-durable-bind  [spawn only]"),
        "{stdout}"
    );
    assert!(
        stdout.contains("pl-decode-fails-closed  [spawn, attach]"),
        "{stdout}"
    );
}

#[test]
fn help_describes_the_anchor_lookup_the_exit_codes_and_every_command() {
    let assert = Command::cargo_bin(BIN)
        .expect("cargo bin")
        .arg("--help")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(stdout.contains("then in the cache directory"), "{stdout}");
    assert!(stdout.contains("checksum-verified"), "{stdout}");
    assert!(stdout.contains("Exit codes: 0 conformant"), "{stdout}");
    assert!(
        stdout.contains("a report that cannot be written (stdout or --output file)"),
        "{stdout}"
    );
    assert!(
        stdout.contains("List the check ids and what each one expects."),
        "{stdout}"
    );
}

#[test]
fn an_empty_selection_is_a_usage_error_never_a_green_report() {
    for args in [
        vec!["run", "--checks", ""],
        vec![
            "run",
            "--nats",
            "nats://127.0.0.1:1",
            "--checks",
            "missing-stream-fails-coverage-probe",
        ],
    ] {
        let assert = Command::cargo_bin(BIN)
            .expect("cargo bin")
            .args(&args)
            .assert()
            .code(2);
        let output = assert.get_output();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        assert!(stderr.contains("no check to run"), "{args:?}: {stderr}");
        assert!(output.stdout.is_empty(), "no report for {args:?}");
    }
}

fn output_of_run_with_nats_url(url: &str, expected_code: i32) -> String {
    let assert = Command::cargo_bin(BIN)
        .expect("cargo bin")
        .args([
            "run",
            "--nats",
            url,
            "--checks",
            "dead-grammar-fails-loud",
            "--timeout",
            "2s",
        ])
        .assert()
        .code(expected_code);
    let output = assert.get_output();
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn a_nats_url_the_harness_refuses_never_reaches_the_logs_with_its_credentials() {
    let unsupported_scheme = ["f", "tp"].concat();
    for url in [
        "nats://operator:hunter2@127.0.0.1:notaport".to_owned(),
        "nats://operator:hunter2@:4222".to_owned(),
        format!("{unsupported_scheme}://operator:hunter2@127.0.0.1:4222"),
        "nats://operator:hunter2@127.0.0.1:999999".to_owned(),
    ] {
        let shown = output_of_run_with_nats_url(&url, 2);
        assert!(!shown.contains("hunter2"), "{url}: {shown}");
        assert!(!shown.contains("operator"), "{url}: {shown}");
    }
}

#[test]
fn a_password_holding_a_slash_never_reaches_the_report_or_the_error_line() {
    for url in [
        "nats://operator:Zk9/hunter2tail@127.0.0.1:1",
        "nats://operator:Zk9?hunter2tail@127.0.0.1:1",
        "nats://operator:Zk9#hunter2tail@127.0.0.1:1",
        "nats://operator:Zk9,hunter2tail@127.0.0.1:1",
    ] {
        let shown = output_of_run_with_nats_url(url, 2);
        assert!(!shown.contains("hunter2tail"), "{url}: {shown}");
        assert!(!shown.contains("Zk9"), "{url}: {shown}");
        assert!(shown.contains("127.0.0.1:1"), "{url}: {shown}");
    }
}

#[test]
fn a_zero_or_overflowing_timeout_is_a_usage_error_with_exit_code_two() {
    for timeout in ["0s", "0", "307445734561825861m"] {
        let assert = Command::cargo_bin(BIN)
            .expect("cargo bin")
            .args([
                "run",
                "--timeout",
                timeout,
                "--checks",
                "anchor-subjects-match-lib",
            ])
            .assert()
            .code(2);
        let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
        assert!(stderr.contains("invalid duration"), "{timeout}: {stderr}");
        assert!(!stderr.contains("panicked"), "{timeout}: {stderr}");
    }
}

fn run_with_closed_stdout(args: &[&str]) -> (Option<i32>, String) {
    let (reader, writer) = std::io::pipe().expect("pipe");
    drop(reader);
    let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin(BIN))
        .args(args)
        .stdout(writer)
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn the cli");

    let mut stderr = String::new();
    std::io::Read::read_to_string(&mut child.stderr.take().expect("stderr"), &mut stderr)
        .expect("read stderr");
    let status = child.wait().expect("wait");
    (status.code(), stderr)
}

#[test]
fn a_closed_stdout_on_list_exits_2_with_one_error_line_instead_of_a_silent_101() {
    let (code, stderr) = run_with_closed_stdout(&["list"]);

    assert_eq!(code, Some(2), "{stderr}");
    assert!(stderr.starts_with("error: writing to stdout"), "{stderr}");
    assert_eq!(stderr.trim_end().lines().count(), 1, "{stderr}");
}

#[test]
#[ignore = "requires `nats-server` and the prebuilt `nats-fabric` anchor on PATH"]
fn a_closed_stdout_on_run_exits_2_with_one_error_line_instead_of_a_silent_101() {
    for format in ["json", "human", "junit"] {
        let (code, stderr) = run_with_closed_stdout(&[
            "run",
            "--checks",
            "anchor-subjects-match-lib",
            "--format",
            format,
        ]);

        assert_eq!(code, Some(2), "{format}: {stderr}");
        assert!(
            stderr.starts_with("error: writing to stdout"),
            "{format}: {stderr}"
        );
        assert_eq!(stderr.trim_end().lines().count(), 1, "{format}: {stderr}");
    }
}

#[test]
fn a_closed_stderr_does_not_turn_the_usage_verdict_into_a_panic() {
    let (reader, writer) = std::io::pipe().expect("pipe");
    drop(reader);
    let status = std::process::Command::new(assert_cmd::cargo::cargo_bin(BIN))
        .args(["run", "--timeout", "0s"])
        .stdout(std::process::Stdio::null())
        .stderr(writer)
        .status()
        .expect("run the cli");

    assert_eq!(status.code(), Some(2));
}
