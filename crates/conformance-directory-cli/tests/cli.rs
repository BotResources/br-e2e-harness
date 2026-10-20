use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use conformance_directory::{ALL_CHECKS, locate_anchor};

fn cli() -> Command {
    Command::cargo_bin("conformance-directory").expect("cargo bin")
}

fn stderr_of(assert: assert_cmd::assert::Assert) -> String {
    String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr")
}

fn scratch(label: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("conformance-directory-cli")
        .join(label);
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn executable_script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, body).expect("write the script");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

#[test]
fn an_explicit_anchor_path_that_cannot_run_exits_2_and_names_the_path() {
    let assert = cli()
        .args(["run", "--anchor-binary", "/nonexistent/identity-directory"])
        .assert()
        .code(2);
    let stderr = stderr_of(assert);
    assert!(
        stderr.contains("the `identity-directory` binary is not available"),
        "{stderr}"
    );
    assert!(
        stderr.contains("/nonexistent/identity-directory"),
        "{stderr}"
    );
}

#[test]
fn a_missing_go_anchor_exits_2_with_the_typed_error() {
    let assert = cli()
        .env("PATH", "")
        .env_remove("HOME")
        .env_remove("XDG_CACHE_HOME")
        .args(["run"])
        .assert()
        .code(2);
    let stderr = stderr_of(assert);
    assert!(
        stderr.contains("the `identity-directory` binary is not available"),
        "{stderr}"
    );
    assert!(stderr.contains("Put it on PATH"), "{stderr}");
    assert!(stderr.starts_with("error: "), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
}

#[test]
fn an_unknown_check_exits_2_before_the_anchor_is_looked_up() {
    let assert = cli()
        .env("PATH", "")
        .args(["run", "--checks", "w1,x9"])
        .assert()
        .code(2);
    let stderr = stderr_of(assert);
    assert!(stderr.contains("unknown check \"x9\""), "{stderr}");
    assert!(!stderr.contains("binary is not available"), "{stderr}");
}

#[test]
fn an_empty_selection_exits_2() {
    let assert = cli().args(["run", "--checks", " , "]).assert().code(2);
    assert!(stderr_of(assert).contains("no checks selected"));
}

#[test]
fn help_states_there_is_no_service_to_spawn_or_attach() {
    let top = cli().arg("--help").assert().success();
    let top = String::from_utf8(top.get_output().stdout.clone()).expect("utf8 help");
    assert!(top.contains("no spawn or attach mode"), "{top}");
    let run = cli().args(["run", "--help"]).assert().success();
    let run = String::from_utf8(run.get_output().stdout.clone()).expect("utf8 help");
    assert!(run.contains("--anchor-binary"), "{run}");
    assert!(run.contains("--checks"), "{run}");
    assert!(!run.contains("--spawn"), "{run}");
    assert!(!run.contains("--attach"), "{run}");
}

#[tokio::test]
#[ignore = "wire gate: needs the prebuilt `identity-directory` on PATH"]
async fn the_offline_checks_pass_against_the_go_anchor() {
    let stdout = tokio::task::spawn_blocking(|| {
        cli()
            .args(["run", "--checks", "w1,w2,w3,w4,w5,w6", "--format", "json"])
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
    assert_eq!(report["passed"], serde_json::json!(6), "{stdout}");
    assert_eq!(report["failed"], serde_json::json!(0), "{stdout}");
    assert_eq!(report["subject"], serde_json::json!("identity-directory"));
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server`, a Postgres (E2E_PG_ADMIN_URL), the prebuilt `identity-directory` on PATH and `fabric-nats` built beside the test executable"]
async fn the_full_battery_is_conformant_against_the_go_anchor() {
    let stdout = tokio::task::spawn_blocking(|| {
        cli()
            .args(["run", "--format", "json"])
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
    assert_eq!(
        report["passed"],
        serde_json::json!(ALL_CHECKS.len()),
        "{stdout}"
    );
    let checks: Vec<&str> = report["outcomes"]
        .as_array()
        .expect("outcomes")
        .iter()
        .map(|o| o["check"].as_str().expect("check"))
        .collect();
    assert_eq!(
        checks,
        [
            "w1", "w2", "w3", "w4", "w5", "w6", "p1", "p2", "c1", "c2", "c3", "c4", "c5", "c6"
        ]
    );
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server`, the prebuilt `identity-directory` on PATH and `fabric-nats` built beside the test executable"]
async fn the_publisher_checks_write_a_conformant_junit_report_with_an_explicit_anchor() {
    let anchor = locate_anchor().await.expect("the anchor binary");
    let report_path = scratch("junit").join("directory-junit.xml");

    let path = report_path.clone();
    tokio::task::spawn_blocking(move || {
        cli()
            .args([
                "run",
                "--anchor-binary",
                &anchor.to_string_lossy(),
                "--checks",
                "p2,p1",
                "--subject-label",
                "kit",
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
        xml.contains("<testsuite name=\"kit\" tests=\"2\" failures=\"0\" skipped=\"0\">"),
        "{xml}"
    );
    assert!(
        xml.find("name=\"p1\"") < xml.find("name=\"p2\""),
        "checks run in battery order: {xml}"
    );
}

#[tokio::test]
#[ignore = "wire gate: needs the prebuilt `identity-directory` on PATH"]
async fn an_anchor_that_emits_foreign_user_keys_is_non_conformant_with_exit_1() {
    let anchor = locate_anchor().await.expect("the anchor binary");
    let twin = executable_script(
        &scratch("foreign-keys"),
        "foreign-keys-twin.sh",
        &format!(
            "#!/bin/sh\n\"{}\" \"$@\" | sed 's#identity/users/#identity/people/#g'\n",
            anchor.display()
        ),
    );

    let stdout = tokio::task::spawn_blocking(move || {
        cli()
            .args([
                "run",
                "--anchor-binary",
                &twin.to_string_lossy(),
                "--checks",
                "w1,w3",
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
    assert!(stdout.contains("[FAIL] w1"), "{stdout}");
    assert!(stdout.contains("[PASS] w3"), "{stdout}");
    assert!(stdout.contains("NON-CONFORMANT"), "{stdout}");
}

#[test]
#[ignore = "wire gate: needs the prebuilt `identity-directory` on PATH"]
fn a_closed_stdout_exits_2_with_one_error_line_instead_of_a_silent_101() {
    let (reader, writer) = std::io::pipe().expect("pipe");
    let bin = assert_cmd::cargo::cargo_bin("conformance-directory");
    let mut child = std::process::Command::new(bin)
        .args(["run", "--checks", "w6", "--format", "json"])
        .stdout(writer)
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn the cli");
    drop(reader);

    let mut stderr = String::new();
    std::io::Read::read_to_string(&mut child.stderr.take().expect("stderr"), &mut stderr)
        .expect("read stderr");
    let status = child.wait().expect("wait");

    assert_eq!(status.code(), Some(2), "{stderr}");
    assert!(
        stderr.starts_with("error: writing report to stdout"),
        "{stderr}"
    );
    assert_eq!(stderr.trim_end().lines().count(), 1, "{stderr}");
}

#[test]
#[ignore = "real-infra: needs the prebuilt `identity-directory` on PATH and `fabric-nats` built beside the test executable"]
fn a_missing_nats_server_exits_2_with_one_error_line_and_no_panic_text() {
    let anchor = locate_anchor_blocking();
    let only_sh = scratch("only-sh");
    std::os::unix::fs::symlink("/bin/sh", only_sh.join("sh")).expect("link sh");
    let assert = cli()
        .env("PATH", &only_sh)
        .args([
            "run",
            "--anchor-binary",
            &anchor.to_string_lossy(),
            "--checks",
            "p1",
        ])
        .assert()
        .code(2);
    let stderr = stderr_of(assert);
    assert!(stderr.starts_with("error: "), "{stderr}");
    assert!(stderr.contains("nats-server"), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert_eq!(stderr.trim_end().lines().count(), 1, "{stderr}");
}

fn locate_anchor_blocking() -> PathBuf {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(locate_anchor())
        .expect("the anchor binary")
}
