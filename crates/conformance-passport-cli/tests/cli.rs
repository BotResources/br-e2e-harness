use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use assert_cmd::Command;
use conformance_passport::{ALL, locate_subject};

const WRONG_SEAL_KEY: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

fn cli() -> Command {
    Command::cargo_bin("conformance-passport").expect("cargo bin")
}

fn stderr_of(assert: assert_cmd::assert::Assert) -> String {
    String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr")
}

#[test]
fn a_spawn_path_that_is_not_an_executable_file_exits_2_and_names_the_path() {
    let assert = cli()
        .args(["run", "--spawn", "/nonexistent/never-spawned"])
        .assert()
        .code(2);
    let stderr = stderr_of(assert);
    assert!(stderr.contains("/nonexistent/never-spawned"), "{stderr}");
    assert!(stderr.contains("not an executable file"), "{stderr}");
}

#[test]
fn a_bare_spawn_name_absent_from_path_exits_2_and_names_the_caller_binary() {
    let assert = cli()
        .env("PATH", "")
        .args(["run", "--spawn", "my-service"])
        .assert()
        .code(2);
    let stderr = stderr_of(assert);
    assert!(
        stderr.contains("`my-service` was not found on PATH"),
        "{stderr}"
    );
    assert!(!stderr.contains("identity-passport"), "{stderr}");
}

#[test]
fn a_missing_go_anchor_exits_2_with_the_typed_error() {
    let assert = cli()
        .env("PATH", "")
        .env_remove("HOME")
        .env_remove("XDG_CACHE_HOME")
        .args(["run", "--anchor"])
        .assert()
        .code(2);
    let stderr = stderr_of(assert);
    assert!(
        stderr.contains("the `identity-passport` binary is not available"),
        "{stderr}"
    );
    assert!(stderr.contains("Put it on PATH"), "{stderr}");
}

#[test]
fn a_missing_nats_server_exits_2_with_one_error_line_and_no_panic_text() {
    let assert = cli()
        .env("PATH", "")
        .args(["run", "--spawn", "/bin/sh"])
        .assert()
        .code(2);
    let stderr = stderr_of(assert);
    assert!(stderr.contains("nats-server not found on PATH"), "{stderr}");
    assert!(stderr.starts_with("error: "), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert!(!stderr.contains("RUST_BACKTRACE"), "{stderr}");
    assert_eq!(stderr.trim_end().lines().count(), 1, "{stderr}");
}

#[test]
fn an_overflowing_timeout_exits_2_instead_of_wrapping() {
    let assert = cli()
        .args(["run", "--anchor", "--timeout", "307445734561825861m"])
        .assert()
        .code(2);
    let stderr = stderr_of(assert);
    assert!(stderr.contains("value too large"), "{stderr}");
}

#[test]
fn an_unknown_scenario_exits_2_before_any_infra() {
    let assert = cli()
        .args([
            "run",
            "--spawn",
            "/nonexistent/never-spawned",
            "--scenarios",
            "p1,p42",
        ])
        .assert()
        .code(2);
    let stderr = stderr_of(assert);
    assert!(stderr.contains("unknown scenario \"p42\""), "{stderr}");
}

#[test]
fn the_offline_g4_check_is_not_a_selectable_scenario() {
    let assert = cli()
        .args(["run", "--anchor", "--scenarios", "g4"])
        .assert()
        .code(2);
    let stderr = stderr_of(assert);
    assert!(stderr.contains("unknown scenario \"g4\""), "{stderr}");
}

#[test]
fn a_bad_timeout_exits_2() {
    let assert = cli()
        .args(["run", "--anchor", "--timeout", "soon"])
        .assert()
        .code(2);
    let stderr = stderr_of(assert);
    assert!(stderr.contains("invalid duration"), "{stderr}");
}

#[test]
fn spawn_and_anchor_are_mutually_exclusive_and_one_is_required() {
    cli()
        .args(["run", "--anchor", "--spawn", "./svc"])
        .assert()
        .code(2);
    cli().args(["run"]).assert().code(2);
}

#[test]
fn help_names_the_modes_and_states_there_is_no_attach_mode() {
    let output = cli().args(["run", "--help"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).expect("utf8 help");
    assert!(stdout.contains("--spawn"), "{stdout}");
    assert!(stdout.contains("--anchor"), "{stdout}");
    assert!(!stdout.contains("--attach"), "{stdout}");
    let top = cli().arg("--help").assert().success();
    let top = String::from_utf8(top.get_output().stdout.clone()).expect("utf8 help");
    assert!(top.contains("no attach mode"), "{top}");
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` and the prebuilt `identity-passport` on PATH"]
async fn anchor_run_passes_every_scenario_against_the_go_reference_subject() {
    let stdout = tokio::task::spawn_blocking(|| {
        cli()
            .args(["run", "--anchor", "--format", "json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone()
    })
    .await
    .expect("cli task");

    let stdout = String::from_utf8(stdout).expect("utf8 report");
    let report: serde_json::Value = serde_json::from_str(&stdout).expect("json report");
    assert_eq!(report["conformant"], serde_json::json!(true), "{stdout}");
    assert_eq!(report["passed"], serde_json::json!(ALL.len()), "{stdout}");
    assert_eq!(report["failed"], serde_json::json!(0), "{stdout}");
    assert_eq!(report["skipped"], serde_json::json!(0), "{stdout}");
    let checks: Vec<&str> = report["outcomes"]
        .as_array()
        .expect("outcomes")
        .iter()
        .map(|o| o["check"].as_str().expect("check"))
        .collect();
    assert_eq!(
        checks.last(),
        Some(&"p8"),
        "the destructive p8 runs last: {checks:?}"
    );
    assert_eq!(report["subject"], serde_json::json!("identity-passport"));
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` and the prebuilt `identity-passport` on PATH"]
async fn spawn_run_writes_a_conformant_junit_report_for_a_subset_of_scenarios() {
    let anchor = locate_subject().await.expect("the anchor binary");
    let report_path = scratch().join("passport-junit.xml");
    std::fs::remove_file(&report_path).ok();

    let path = report_path.clone();
    tokio::task::spawn_blocking(move || {
        cli()
            .args([
                "run",
                "--spawn",
                &anchor.to_string_lossy(),
                "--scenarios",
                "p4,p1",
                "--subject-label",
                "ref-subject",
                "--format",
                "junit",
                "--output",
                &path.to_string_lossy(),
            ])
            .assert()
            .success()
            .stdout("");
    })
    .await
    .expect("cli task");

    let xml = std::fs::read_to_string(&report_path).expect("junit report written");
    assert!(
        xml.contains("<testsuite name=\"ref-subject\" tests=\"2\" failures=\"0\" skipped=\"0\">"),
        "{xml}"
    );
    assert!(
        xml.contains("name=\"p1\"") && xml.contains("name=\"p4\""),
        "{xml}"
    );
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` and the prebuilt `identity-passport` on PATH"]
async fn a_subject_that_cannot_open_the_seal_is_non_conformant_with_exit_1() {
    let anchor = locate_subject().await.expect("the anchor binary");
    let twin = scratch().join("wrong-seal-key-twin.sh");
    std::fs::write(
        &twin,
        format!(
            "#!/bin/sh\nexport BEARER_SEAL_KEY={WRONG_SEAL_KEY}\nexec \"{}\"\n",
            anchor.display()
        ),
    )
    .expect("write the twin script");
    std::fs::set_permissions(&twin, std::fs::Permissions::from_mode(0o755)).expect("chmod");

    let stdout = tokio::task::spawn_blocking(move || {
        cli()
            .args([
                "run",
                "--spawn",
                &twin.to_string_lossy(),
                "--scenarios",
                "p1,p4",
                "--format",
                "human",
            ])
            .assert()
            .code(1)
            .get_output()
            .stdout
            .clone()
    })
    .await
    .expect("cli task");

    let stdout = String::from_utf8(stdout).expect("utf8 report");
    assert!(stdout.contains("[FAIL] p1"), "{stdout}");
    assert!(stdout.contains("[PASS] p4"), "{stdout}");
    assert!(stdout.contains("NON-CONFORMANT"), "{stdout}");
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_subject_that_never_reports_ready_exits_2_with_the_readiness_sentence() {
    let script = scratch().join("never-ready.sh");
    std::fs::write(&script, "#!/bin/sh\nexec sleep 30\n").expect("write the script");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");

    let stderr = tokio::task::spawn_blocking(move || {
        cli()
            .args([
                "run",
                "--spawn",
                &script.to_string_lossy(),
                "--timeout",
                "500ms",
            ])
            .assert()
            .code(2)
            .get_output()
            .stderr
            .clone()
    })
    .await
    .expect("cli task");

    let stderr = String::from_utf8(stderr).expect("utf8 stderr");
    assert!(
        stderr.contains("timed out waiting for the passport subject to report /readyz=200"),
        "{stderr}"
    );
}

fn scratch() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("conformance-passport-cli");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}
