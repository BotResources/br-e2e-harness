use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use conformance_scope::locate_subject;

const SERVICE_KEY: &str = "notifier";
const SCOPES: &str = "notifier:read,notifier:admin";
const OS_TEXT: &str = "Error: Address already in use (os error 98)";

fn stand_in(name: &str, body: &str) -> (PathBuf, PathBuf) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("{name}-{}", uuid::Uuid::now_v7().simple()));
    std::fs::create_dir_all(&dir).expect("create the subject directory");
    let binary = dir.join("scope-service");
    std::fs::write(&binary, format!("#!/bin/sh\n{body}")).expect("write the stand-in subject");
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755))
        .expect("mark the stand-in executable");
    (dir, binary)
}

fn attempts(dir: &Path) -> usize {
    std::fs::read_to_string(dir.join("attempts"))
        .map(|raw| raw.lines().count())
        .unwrap_or(0)
}

fn spawn_run(binary: PathBuf, scenarios: &str) -> assert_cmd::assert::Assert {
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
            "--scenarios",
            scenarios,
        ])
        .assert()
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `fabric-nats` beside the CLI + the `scope-service` binary on PATH"]
async fn a_subject_that_loses_its_port_once_with_the_os_text_is_conformant() {
    let reference = locate_subject()
        .await
        .expect("locate the reference subject");
    let (dir, binary) = stand_in(
        "cli-port-race-recovers",
        &format!(
            "echo x >> \"$(dirname \"$0\")/attempts\"\n\
             if [ \"$(wc -l < \"$(dirname \"$0\")/attempts\")\" -lt 2 ]; then\n\
             echo \"{OS_TEXT}\" >&2\n\
             exit 98\n\
             fi\n\
             exec \"{}\"\n",
            reference.display()
        ),
    );

    let binary_for_run = binary.clone();
    let output = tokio::task::spawn_blocking(move || {
        spawn_run(binary_for_run, "s1,declaration-content")
            .success()
            .get_output()
            .stdout
            .clone()
    })
    .await
    .expect("cli task");
    let tries = attempts(&dir);
    std::fs::remove_dir_all(&dir).ok();

    let stdout = String::from_utf8(output).expect("utf8 report");
    assert!(stdout.contains("CONFORMANT"), "{stdout}");
    assert_eq!(tries, 2, "{stdout}");
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `fabric-nats` beside the CLI on PATH"]
async fn a_subject_that_never_keeps_its_port_exits_2_as_a_setup_error() {
    let (dir, binary) = stand_in(
        "cli-port-race-exhausted",
        &format!("echo x >> \"$(dirname \"$0\")/attempts\"\necho \"{OS_TEXT}\" >&2\nexit 98\n"),
    );

    let (stdout, stderr) = tokio::task::spawn_blocking(move || {
        let assert = spawn_run(binary, "s1,declaration-content").code(2);
        let output = assert.get_output();
        (output.stdout.clone(), output.stderr.clone())
    })
    .await
    .expect("cli task");
    let tries = attempts(&dir);
    std::fs::remove_dir_all(&dir).ok();

    let stderr = String::from_utf8(stderr).expect("utf8 stderr");
    assert!(stderr.contains("5 attempts"), "{stderr}");
    assert!(stderr.contains("Address already in use"), "{stderr}");
    assert!(
        !String::from_utf8_lossy(&stdout).contains("NON-CONFORMANT"),
        "an environment fault is not a verdict on the service: {}",
        String::from_utf8_lossy(&stdout)
    );
    assert_eq!(tries, 5);
}
