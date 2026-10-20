use std::path::{Path, PathBuf};

use assert_cmd::Command;

const UNREACHABLE_FIRST: &str = "  - service_key: first_service
    timeout: 1s
    scopes:
      - key: first_service:read
    attach:
      nats: nats://127.0.0.1:1
      readyz: http://127.0.0.1:1/readyz
";

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("{name}-{}", uuid::Uuid::now_v7().simple()));
    std::fs::create_dir_all(&dir).expect("create scratch directory");
    dir
}

fn refused(manifest: &str) -> String {
    let dir = scratch("manifest-preflight");
    let file = dir.join("services.yaml");
    let output = dir.join("report.json");
    std::fs::write(&file, manifest).expect("write manifest");
    let assert = Command::cargo_bin("conformance-scope")
        .expect("cargo bin")
        .args([
            "manifest",
            &file.to_string_lossy(),
            "--format",
            "json",
            "--output",
            &output.to_string_lossy(),
        ])
        .assert()
        .code(2);
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");
    assert!(
        !output.exists(),
        "no report is written for a refused manifest"
    );
    std::fs::remove_dir_all(&dir).ok();
    stderr
}

#[test]
fn a_manifest_without_services_exits_2_instead_of_judging_nothing() {
    let stderr = refused("services: []\n");

    assert!(stderr.contains("no services"), "{stderr}");
}

#[test]
fn two_attach_entries_on_one_broker_exit_2_naming_both_services_and_the_broker() {
    let manifest = "services:
  - service_key: svc_a
    scopes:
      - key: svc_a:read
    attach:
      nats: nats://127.0.0.1:4222
      readyz: http://127.0.0.1:8081/readyz
  - service_key: svc_b
    scopes:
      - key: svc_b:read
    attach:
      nats: nats://127.0.0.1:4222/
      readyz: http://127.0.0.1:8082/readyz
";
    let stderr = refused(manifest);

    assert!(stderr.contains("svc_a"), "{stderr}");
    assert!(stderr.contains("svc_b"), "{stderr}");
    assert!(stderr.contains("nats://127.0.0.1:4222"), "{stderr}");
    assert!(stderr.contains("fresh handshake stream"), "{stderr}");
}

#[test]
fn a_bad_service_after_a_good_one_is_refused_before_the_good_one_runs() {
    let not_a_file = "/nonexistent/subject-binary";
    let cases: [(&str, &str); 8] = [
        (
            "    timeout: 0s\n    scopes:\n      - key: second_service:read\n    attach:\n      nats: nats://127.0.0.1:2\n      readyz: http://127.0.0.1:2/readyz\n",
            "0ns",
        ),
        (
            "    timeout: 1s\n    scopes:\n      - key: other_service:read\n    attach:\n      nats: nats://127.0.0.1:2\n      readyz: http://127.0.0.1:2/readyz\n",
            "other_service:read",
        ),
        (
            "    timeout: 1s\n    scenarios: s9\n    scopes:\n      - key: second_service:read\n    attach:\n      nats: nats://127.0.0.1:2\n      readyz: http://127.0.0.1:2/readyz\n",
            "s9",
        ),
        (
            "    timeout: 1s\n    scopes:\n      - key: second_service:read\n    spawn:\n      path: /nonexistent/subject-binary\n",
            not_a_file,
        ),
        (
            "    timeout: 1s\n    scopes:\n      - key: second_service:read\n    attach:\n      nats: nats://127.0.0.1:2\n      readyz: not-a-url\n",
            "not-a-url",
        ),
        (
            "    timeout: 1s\n    scopes:\n      - key: second_service:read\n    attach:\n      nats: nats://127.0.0.1:2\n      readyz: http://127.0.0.1:2/readyz\n    spawn:\n      path: ./subject\n",
            "both `attach` and `spawn`",
        ),
        (
            "    timeout: 1s\n    scopes:\n      - key: second_service:read\n",
            "neither `attach` nor `spawn`",
        ),
        (
            "    timeout: 1s\n    scopes:\n      - key: second_service:read\n        platform_only: true\n      - key: second_service:write\n    spawn:\n      path: /nonexistent/subject-binary\n",
            "second_service:read=true",
        ),
    ];
    for (second, named) in cases {
        let manifest =
            format!("services:\n{UNREACHABLE_FIRST}  - service_key: second_service\n{second}");
        let stderr = refused(&manifest);

        assert!(stderr.contains("second_service"), "{named}: {stderr}");
        assert!(stderr.contains(named), "{named}: {stderr}");
        assert!(
            !stderr.contains("nats://127.0.0.1:1"),
            "the first service must not run before the second is validated: {stderr}"
        );
    }
}
