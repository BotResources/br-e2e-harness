#[path = "common/cli.rs"]
mod cli;
#[path = "common/fixture.rs"]
mod fixture;
#[path = "common/scratch.rs"]
mod scratch;
#[path = "common/stderr.rs"]
mod stderr;

use assert_cmd::cargo::cargo_bin;
use cli::cli;
use fixture::{SCOPES, SERVICE_KEY};
use scratch::scratch;
use stderr::stderr_of;

#[test]
fn attach_to_an_unreachable_broker_exits_2_naming_the_broker() {
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
    assert!(stderr.contains("nats://127.0.0.1:1"), "{stderr}");
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
fn a_readyz_url_that_is_not_an_http_url_exits_2_naming_it() {
    let stderr = stderr_of(
        cli()
            .args([
                "run",
                "--nats",
                "nats://127.0.0.1:1",
                "--readyz",
                "127.0.0.1:8080/readyz",
                "--service-key",
                SERVICE_KEY,
                "--scopes",
                SCOPES,
            ])
            .assert()
            .code(2),
    );
    assert!(stderr.contains("127.0.0.1:8080/readyz"), "{stderr}");
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
fn spawn_next_to_attach_endpoints_exits_2_naming_the_flag() {
    for (flag, value) in [
        ("--nats", "nats://127.0.0.1:1"),
        ("--readyz", "http://127.0.0.1:1/readyz"),
    ] {
        let stderr = stderr_of(
            cli()
                .args([
                    "run",
                    "--spawn",
                    "/nonexistent/service-binary",
                    flag,
                    value,
                    "--service-key",
                    SERVICE_KEY,
                    "--scopes",
                    SCOPES,
                ])
                .assert()
                .code(2),
        );
        assert!(stderr.contains(flag), "{flag}: {stderr}");
        assert!(stderr.contains("--spawn"), "{flag}: {stderr}");
    }
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
