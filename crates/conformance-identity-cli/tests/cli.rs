use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use assert_cmd::Command;
use conformance_identity::{
    ALL, IdentityHarness, ReadyzProbe, Subject, SubjectConfig, locate_subject, provision,
};

fn cli() -> Command {
    Command::cargo_bin("conformance-identity").expect("cargo bin")
}

fn stderr_of(assert: assert_cmd::assert::Assert) -> String {
    String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr")
}

fn scratch(label: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("conformance-identity-cli")
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
fn a_spawn_path_that_is_not_an_executable_file_exits_2_and_names_the_path() {
    let assert = cli()
        .args(["run", "--spawn", "/nonexistent/never-spawned"])
        .assert()
        .code(2);
    let stderr = stderr_of(assert);
    assert!(stderr.contains("/nonexistent/never-spawned"), "{stderr}");
    assert!(stderr.contains("is not executable"), "{stderr}");
}

#[test]
fn a_bare_spawn_name_absent_from_path_exits_2_and_names_the_caller_binary() {
    let assert = cli()
        .env("PATH", "")
        .args(["run", "--spawn", "my-acceptor"])
        .assert()
        .code(2);
    let stderr = stderr_of(assert);
    assert!(
        stderr.contains("the `my-acceptor` binary is not available"),
        "{stderr}"
    );
    assert!(stderr.contains("pass its path"), "{stderr}");
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
        stderr.contains("the `identity-acceptor` binary is not available"),
        "{stderr}"
    );
    assert!(
        stderr.contains("Put it on an absolute PATH entry"),
        "{stderr}"
    );
}

#[test]
fn a_missing_fabric_nats_beside_the_executable_exits_2_with_one_error_line() {
    let dir = scratch("no-fabric-nats");
    let copy = dir.join("conformance-identity");
    std::fs::copy(assert_cmd::cargo::cargo_bin("conformance-identity"), &copy).expect("copy");
    let assert = Command::new(&copy)
        .args(["run", "--spawn", "/bin/sh"])
        .assert()
        .code(2);
    let stderr = stderr_of(assert);
    assert!(stderr.starts_with("error: "), "{stderr}");
    assert!(stderr.contains("fabric-nats"), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert_eq!(stderr.trim_end().lines().count(), 1, "{stderr}");
}

#[test]
fn a_timeout_the_clock_cannot_represent_exits_2_before_any_infra() {
    for timeout in [
        "18446744073709551615s",
        "9223372036854775807s",
        "200000000000000000m",
        "307445734561825861m",
    ] {
        let assert = cli()
            .args([
                "run",
                "--spawn",
                "/nonexistent/never-spawned",
                "--timeout",
                timeout,
            ])
            .assert()
            .code(2);
        let stderr = stderr_of(assert);
        assert!(stderr.contains("--timeout"), "{timeout}: {stderr}");
        assert!(stderr.contains("value too large"), "{timeout}: {stderr}");
        assert!(!stderr.contains("is not executable"), "{timeout}: {stderr}");
        assert_eq!(stderr.trim_end().lines().count(), 1, "{stderr}");
    }
}

#[test]
fn a_bad_timeout_exits_2() {
    let assert = cli()
        .args(["run", "--anchor", "--timeout", "soon"])
        .assert()
        .code(2);
    assert!(stderr_of(assert).contains("invalid duration"));
}

#[test]
fn an_unknown_scenario_exits_2_before_any_infra() {
    let assert = cli()
        .args([
            "run",
            "--spawn",
            "/nonexistent/never-spawned",
            "--scenarios",
            "a1,a42",
        ])
        .assert()
        .code(2);
    let stderr = stderr_of(assert);
    assert!(stderr.contains("unknown scenario \"a42\""), "{stderr}");
}

#[test]
fn exactly_one_mode_is_required() {
    cli().args(["run"]).assert().code(2);
    cli()
        .args(["run", "--anchor", "--spawn", "./acceptor"])
        .assert()
        .code(2);
    cli()
        .args([
            "run",
            "--anchor",
            "--attach",
            "--nats",
            "nats://127.0.0.1:1",
            "--readyz",
            "http://127.0.0.1:1/readyz",
        ])
        .assert()
        .code(2);
}

#[test]
fn attach_needs_both_urls_and_the_urls_need_attach() {
    cli().args(["run", "--attach"]).assert().code(2);
    cli()
        .args(["run", "--attach", "--nats", "nats://127.0.0.1:1"])
        .assert()
        .code(2);
    cli()
        .args(["run", "--anchor", "--nats", "nats://127.0.0.1:1"])
        .assert()
        .code(2);
}

#[test]
fn help_names_the_three_modes() {
    let output = cli().args(["run", "--help"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).expect("utf8 help");
    for flag in ["--spawn", "--anchor", "--attach", "--nats", "--readyz"] {
        assert!(stdout.contains(flag), "{flag}: {stdout}");
    }
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server`, the prebuilt `identity-acceptor` on PATH and `fabric-nats` built beside the test executable"]
async fn anchor_run_passes_every_scenario_against_the_go_reference_acceptor() {
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
    assert_eq!(report["subject"], serde_json::json!("identity-acceptor"));
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server`, the prebuilt `identity-acceptor` on PATH and `fabric-nats` built beside the test executable"]
async fn spawn_run_writes_a_conformant_junit_report_for_a_subset_of_scenarios() {
    let acceptor = locate_subject().await.expect("the acceptor binary");
    let report_path = scratch("junit").join("identity-junit.xml");

    let path = report_path.clone();
    tokio::task::spawn_blocking(move || {
        cli()
            .args([
                "run",
                "--spawn",
                &acceptor.to_string_lossy(),
                "--scenarios",
                "a4,a1",
                "--subject-label",
                "ref-acceptor",
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
        xml.contains("<testsuite name=\"ref-acceptor\" tests=\"2\" failures=\"0\" skipped=\"0\">"),
        "{xml}"
    );
    assert!(
        xml.contains("name=\"a1\"") && xml.contains("name=\"a4\""),
        "{xml}"
    );
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server`, the prebuilt `identity-acceptor` on PATH and `fabric-nats` built beside the test executable"]
async fn attach_run_against_a_live_acceptor_is_conformant() {
    let harness = IdentityHarness::start().await.expect("harness");
    provision(&harness.nats_url(), "scope_declaration.toml")
        .await
        .expect("fabric-nats provision");
    let binary = locate_subject().await.expect("acceptor binary");
    let subject = Subject::spawn(&binary, &SubjectConfig::new(&harness.nats_url()));
    let readyz_url = format!("{}/readyz", subject.base_url());
    let probe = ReadyzProbe::new(&readyz_url).expect("readyz");
    for _ in 0..100 {
        if probe.is_ready().await {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let nats_url = harness.nats_url();

    let stdout = tokio::task::spawn_blocking(move || {
        cli()
            .args([
                "run",
                "--attach",
                "--nats",
                &nats_url,
                "--readyz",
                &readyz_url,
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

    subject.shutdown().await;
    harness.shutdown().await;
    let stdout = String::from_utf8(stdout).expect("utf8 report");
    let report: serde_json::Value = serde_json::from_str(&stdout).expect("json report");
    assert_eq!(report["conformant"], serde_json::json!(true), "{stdout}");
    assert_eq!(report["passed"], serde_json::json!(ALL.len()), "{stdout}");
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` and `fabric-nats` built beside the test executable"]
async fn an_attached_acceptor_that_never_reports_ready_exits_2() {
    let harness = IdentityHarness::start_with_binary(PathBuf::from("/bin/sh"))
        .await
        .expect("harness");
    let nats_url = harness.nats_url();
    harness.shutdown().await;

    let stderr = tokio::task::spawn_blocking(move || {
        let assert = cli()
            .args([
                "run",
                "--attach",
                "--nats",
                &nats_url,
                "--readyz",
                "http://127.0.0.1:1/readyz",
                "--timeout",
                "500ms",
            ])
            .assert()
            .code(2);
        stderr_of(assert)
    })
    .await
    .expect("cli task");
    assert!(stderr.starts_with("error: "), "{stderr}");
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server`, the prebuilt `identity-acceptor` on PATH and `fabric-nats` built beside the test executable"]
async fn an_acceptor_that_answers_nothing_is_non_conformant_with_exit_1() {
    let acceptor = locate_subject().await.expect("the acceptor binary");
    let dir = scratch("silent-acceptor");
    let twin = executable_script(
        &dir,
        "silent-acceptor.sh",
        &format!(
            "#!/bin/sh\nexport SCOPE_ACCEPTANCE_ENABLED=false\nexec \"{}\"\n",
            acceptor.display()
        ),
    );

    let stdout = tokio::task::spawn_blocking(move || {
        cli()
            .args([
                "run",
                "--spawn",
                &twin.to_string_lossy(),
                "--scenarios",
                "a1",
                "--timeout",
                "1s",
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
    assert!(stdout.contains("[FAIL] a1"), "{stdout}");
    assert!(stdout.contains("NON-CONFORMANT"), "{stdout}");
}

#[tokio::test]
#[ignore = "real-infra: needs `fabric-nats` built beside the test executable"]
async fn a_missing_nats_server_exits_2_with_one_error_line_and_no_panic_text() {
    let stderr = tokio::task::spawn_blocking(|| {
        let assert = cli()
            .env("PATH", "")
            .args(["run", "--spawn", "/bin/sh"])
            .assert()
            .code(2);
        stderr_of(assert)
    })
    .await
    .expect("cli task");
    assert!(stderr.starts_with("error: "), "{stderr}");
    assert!(stderr.contains("nats-server"), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert_eq!(stderr.trim_end().lines().count(), 1, "{stderr}");
}

#[test]
#[ignore = "real-infra: needs `nats-server`, the prebuilt `identity-acceptor` on PATH and `fabric-nats` built beside the test executable"]
fn a_closed_stdout_exits_2_with_one_error_line_instead_of_a_silent_101() {
    let (reader, writer) = std::io::pipe().expect("pipe");
    let bin = assert_cmd::cargo::cargo_bin("conformance-identity");
    let mut child = std::process::Command::new(bin)
        .args(["run", "--anchor", "--scenarios", "a1", "--format", "json"])
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
