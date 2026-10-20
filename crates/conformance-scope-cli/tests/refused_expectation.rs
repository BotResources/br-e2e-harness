#[path = "common/cli.rs"]
mod cli;
#[path = "common/scratch.rs"]
mod scratch;
#[path = "common/stderr.rs"]
mod stderr;

use br_test_harness::FabricTestNats;
use cli::cli;
use scratch::scratch;
use stderr::stderr_of;

async fn stderr_of_run_against_a_broker_with_both_streams(extra: &[&str]) -> String {
    let nats = FabricTestNats::start().await;
    let mut args: Vec<String> = [
        "run",
        "--nats",
        nats.url().as_str(),
        "--readyz",
        "http://127.0.0.1:1/readyz",
        "--service-key",
        "my_service",
    ]
    .iter()
    .map(|arg| (*arg).to_string())
    .collect();
    args.extend(extra.iter().map(|arg| (*arg).to_string()));
    if !extra.contains(&"--timeout") {
        args.extend(["--timeout".to_string(), "1s".to_string()]);
    }

    let stderr = tokio::task::spawn_blocking(move || stderr_of(cli().args(&args).assert().code(2)))
        .await
        .expect("cli task");
    nats.shutdown().await;
    stderr
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_platform_only_key_naming_no_expected_scope_exits_2_naming_it() {
    let stderr = stderr_of_run_against_a_broker_with_both_streams(&[
        "--scopes",
        "my_service:read,my_service:admin",
        "--platform-only",
        "my_service:admn=true",
    ])
    .await;

    assert!(stderr.contains("my_service:admn"), "{stderr}");
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_platform_only_key_given_twice_exits_2_naming_it() {
    let stderr = stderr_of_run_against_a_broker_with_both_streams(&[
        "--scopes",
        "my_service:read,my_service:admin",
        "--platform-only",
        "my_service:admin=true,my_service:admin=false",
    ])
    .await;

    assert!(stderr.contains("my_service:admin"), "{stderr}");
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn scope_keys_that_no_declaration_can_hold_exit_2_naming_the_key() {
    for (scopes, culprit) in [
        ("My-Service:Read,my_service:admin", "My-Service:Read"),
        ("my_service:read,other_service:read", "other_service:read"),
        (
            "my_service:read,my_service:read,my_service:admin",
            "my_service:read",
        ),
    ] {
        let stderr = stderr_of_run_against_a_broker_with_both_streams(&["--scopes", scopes]).await;

        assert!(stderr.contains(culprit), "{scopes}: {stderr}");
    }
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_timeout_the_battery_refuses_or_that_overflows_exits_2_naming_the_value() {
    let base = ["--scopes", "my_service:read"];
    for (value, named) in [
        ("0s", "0ns"),
        ("0ms", "0ns"),
        ("0m", "0ns"),
        ("86401s", "86401s"),
        ("1441m", "86460s"),
        ("18446744073709551615s", "18446744073709551615s"),
        ("307445734561825862m", "307445734561825862m"),
        ("9000000000000000000s", "9000000000000000000s"),
    ] {
        let mut args = base.to_vec();
        args.extend(["--timeout", value]);
        let stderr = stderr_of_run_against_a_broker_with_both_streams(&args).await;

        assert!(stderr.contains(named), "{value}: {stderr}");
    }
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_manifest_with_an_unknown_field_exits_2_naming_it() {
    let nats = FabricTestNats::start().await;
    let dir = scratch("manifest-typo");
    let manifest = dir.join("services.yaml");
    std::fs::write(
        &manifest,
        format!(
            "services:\n  - service_key: my_service\n    timeout: 1s\n    scopes:\n      - key: my_service:admin\n        platform_onyl: true\n    attach:\n      nats: {}\n      readyz: http://127.0.0.1:1/readyz\n",
            nats.url()
        ),
    )
    .expect("write manifest");
    let manifest_arg = manifest.to_string_lossy().into_owned();

    let stderr = tokio::task::spawn_blocking(move || {
        stderr_of(cli().args(["manifest", &manifest_arg]).assert().code(2))
    })
    .await
    .expect("cli task");
    nats.shutdown().await;
    std::fs::remove_dir_all(&dir).ok();

    assert!(stderr.contains("invalid manifest"), "{stderr}");
    assert!(stderr.contains("platform_onyl"), "{stderr}");
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_manifest_whose_scope_is_not_owned_by_its_service_exits_2_naming_the_key() {
    let nats = FabricTestNats::start().await;
    let dir = scratch("manifest-unowned");
    let manifest = dir.join("services.yaml");
    std::fs::write(
        &manifest,
        format!(
            "services:\n  - service_key: my_service\n    timeout: 1s\n    scopes:\n      - key: other_service:read\n    attach:\n      nats: {}\n      readyz: http://127.0.0.1:1/readyz\n",
            nats.url()
        ),
    )
    .expect("write manifest");
    let manifest_arg = manifest.to_string_lossy().into_owned();

    let stderr = tokio::task::spawn_blocking(move || {
        stderr_of(cli().args(["manifest", &manifest_arg]).assert().code(2))
    })
    .await
    .expect("cli task");
    nats.shutdown().await;
    std::fs::remove_dir_all(&dir).ok();

    assert!(stderr.contains("other_service:read"), "{stderr}");
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_manifest_listing_a_scope_twice_exits_2_saying_it_is_listed_twice() {
    let nats = FabricTestNats::start().await;
    let dir = scratch("manifest-duplicate-scope");
    let manifest = dir.join("services.yaml");
    std::fs::write(
        &manifest,
        format!(
            "services:\n  - service_key: my_service\n    timeout: 1s\n    scopes:\n      - key: my_service:read\n      - key: my_service:read\n    attach:\n      nats: {}\n      readyz: http://127.0.0.1:1/readyz\n",
            nats.url()
        ),
    )
    .expect("write manifest");
    let manifest_arg = manifest.to_string_lossy().into_owned();

    let stderr = tokio::task::spawn_blocking(move || {
        stderr_of(cli().args(["manifest", &manifest_arg]).assert().code(2))
    })
    .await
    .expect("cli task");
    nats.shutdown().await;
    std::fs::remove_dir_all(&dir).ok();

    assert!(stderr.contains("my_service:read"), "{stderr}");
    assert!(stderr.contains("listed twice"), "{stderr}");
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn a_manifest_timeout_of_zero_or_above_the_cap_exits_2_naming_the_value() {
    for (timeout, named) in [("0s", "0ns"), ("86401s", "86401s")] {
        let nats = FabricTestNats::start().await;
        let dir = scratch("manifest-timeout");
        let manifest = dir.join("services.yaml");
        std::fs::write(
            &manifest,
            format!(
                "services:\n  - service_key: my_service\n    timeout: {timeout}\n    scopes:\n      - key: my_service:read\n    attach:\n      nats: {}\n      readyz: http://127.0.0.1:1/readyz\n",
                nats.url()
            ),
        )
        .expect("write manifest");
        let manifest_arg = manifest.to_string_lossy().into_owned();

        let stderr = tokio::task::spawn_blocking(move || {
            stderr_of(cli().args(["manifest", &manifest_arg]).assert().code(2))
        })
        .await
        .expect("cli task");
        nats.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();

        assert!(stderr.contains(named), "{timeout}: {stderr}");
    }
}
