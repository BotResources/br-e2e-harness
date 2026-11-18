use assert_cmd::Command;
use br_test_harness::BareFabricNats;

mod common;

use common::{Scratch, strings};

const BIN: &str = "conformance-nats-fabric";
const SECRET_URL: &str = "nats://operator:Zk9/hunter2tail@127.0.0.1:1";

struct Run {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn run(args: &[String]) -> Run {
    finish(Command::cargo_bin(BIN).expect("cargo bin").args(args))
}

fn finish(command: &mut Command) -> Run {
    let output = command.output().expect("run the cli");
    Run {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

async fn run_blocking(args: Vec<String>) -> Run {
    tokio::task::spawn_blocking(move || run(&args))
        .await
        .expect("cli task")
}

#[tokio::test]
#[ignore = "requires the prebuilt `nats-fabric` anchor on PATH"]
async fn the_credentials_of_the_nats_url_reach_no_report_format_and_no_stderr() {
    for format in ["json", "human", "junit"] {
        let result = run_blocking(strings(&[
            "run",
            "--nats",
            SECRET_URL,
            "--checks",
            "anchor-subjects-match-lib",
            "--format",
            format,
        ]))
        .await;
        assert_eq!(result.code, Some(0), "{format}: {}", result.stderr);
        for shown in [&result.stdout, &result.stderr] {
            assert!(!shown.contains("hunter2tail"), "{format}: {shown}");
            assert!(!shown.contains("Zk9"), "{format}: {shown}");
            assert!(!shown.contains("operator"), "{format}: {shown}");
        }
        match format {
            "json" => {
                let report: serde_json::Value =
                    serde_json::from_str(&result.stdout).expect("json report");
                assert_eq!(report["nats"], "nats://127.0.0.1:1");
            }
            "human" => assert!(
                result.stdout.contains("mode: attach (nats://127.0.0.1:1)"),
                "{}",
                result.stdout
            ),
            _ => {}
        }
    }
}

#[tokio::test]
#[ignore = "requires the prebuilt `nats-fabric` anchor on PATH"]
async fn output_writes_the_junit_file_and_prints_nothing() {
    let scratch = Scratch::new("nf-cli-output");
    let file = scratch.0.join("report.xml");
    let result = run_blocking(strings(&[
        "run",
        "--nats",
        "nats://127.0.0.1:1",
        "--checks",
        "anchor-subjects-match-lib,anchor-users-deserialize-through-lib",
        "--format",
        "junit",
        "--output",
        file.to_str().expect("utf8 path"),
    ]))
    .await;

    assert_eq!(result.code, Some(0), "{}", result.stderr);
    assert!(result.stdout.is_empty(), "{}", result.stdout);
    let xml = std::fs::read_to_string(&file).expect("the report file");
    assert!(
        xml.contains("<testsuites tests=\"2\" failures=\"0\" skipped=\"0\">"),
        "{xml}"
    );
    assert_eq!(xml.matches("<testcase ").count(), 2, "{xml}");
}

#[tokio::test]
#[ignore = "requires `nats-server` on PATH"]
async fn output_writes_a_failing_junit_report_to_the_file_and_exits_one() {
    let bare = BareFabricNats::without_fixed_streams().await;
    let scratch = Scratch::new("nf-cli-output-fail");
    let file = scratch.0.join("report.xml");
    let result = run_blocking(strings(&[
        "run",
        "--nats",
        &bare.url(),
        "--checks",
        "dead-grammar-fails-loud",
        "--format",
        "junit",
        "--output",
        file.to_str().expect("utf8 path"),
    ]))
    .await;

    assert_eq!(result.code, Some(1), "{}", result.stderr);
    assert!(result.stdout.is_empty(), "{}", result.stdout);
    let xml = std::fs::read_to_string(&file).expect("the report file");
    assert!(
        xml.contains("<testsuites tests=\"1\" failures=\"1\" skipped=\"0\">"),
        "{xml}"
    );
    assert!(xml.contains("<failure message=\"expected: "), "{xml}");
    bare.shutdown().await;
}

#[test]
fn spawn_mode_without_nats_server_on_path_exits_two_naming_the_missing_binary() {
    let scratch = Scratch::new("nf-cli-no-nats-server");
    let empty = &scratch.0;

    let result = finish(
        Command::cargo_bin(BIN)
            .expect("cargo bin")
            .env_clear()
            .env("PATH", empty)
            .args(["run", "--checks", "dead-grammar-fails-loud"]),
    );

    assert_eq!(result.code, Some(2), "{}", result.stderr);
    assert!(result.stderr.contains("nats-server"), "{}", result.stderr);
    assert!(
        result.stderr.contains("cannot be run from PATH"),
        "{}",
        result.stderr
    );
    assert!(result.stdout.is_empty(), "{}", result.stdout);
}

#[test]
fn a_repeated_check_id_is_a_usage_error_never_a_report_counting_it_twice() {
    for format in ["json", "human", "junit"] {
        let result = run(&strings(&[
            "run",
            "--checks",
            "dead-grammar-fails-loud,dead-grammar-fails-loud",
            "--format",
            format,
        ]));
        assert_eq!(result.code, Some(2), "{format}: {}", result.stderr);
        assert!(
            result
                .stderr
                .contains("`dead-grammar-fails-loud` is selected more than once"),
            "{format}: {}",
            result.stderr
        );
        assert!(result.stdout.is_empty(), "{format}: {}", result.stdout);
    }
}
