#![cfg(feature = "nats-fabric")]

use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

use br_test_harness::fabric_nats::BareFabricNats;
use br_test_harness::{FabricTestNats, run_once, workspace_bin};

const DECLARE_MANIFEST: &str = r#"
[[command_durable]]
durable = "verify_probe_cmd"
receiver = "identity"
aggregate = "service_scope"
verb = "declare"
version = 1
"#;

const ACCEPTED_MANIFEST: &str = r#"
[[command_durable]]
durable = "verify_probe_cmd"
receiver = "identity"
aggregate = "service_scope"
verb = "declare"
version = 1

[[event_durable]]
durable = "verify_probe_evt"
producer = "identity"
aggregate = "service_scope"
fact = "accepted"
version = 1
"#;

const REAIMED_MANIFEST: &str = r#"
[[command_durable]]
durable = "verify_probe_cmd"
receiver = "identity"
aggregate = "service_scope"
verb = "revoke"
version = 1
"#;

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn verify_fails_loud_without_creating_the_fixed_streams_it_probes() {
    let bare = BareFabricNats::without_fixed_streams().await;
    let dir = tempfile::tempdir().expect("manifest dir");
    let manifest = write_manifest(dir.path(), DECLARE_MANIFEST);

    let out = cli("verify", &bare.url(), &manifest).await;

    assert_eq!(out.status.code(), Some(4), "{}", rendered(&out));
    assert!(
        stderr(&out).contains("INTEGRATION_CMD"),
        "the failure must name the stream it probed: {}",
        rendered(&out)
    );
    assert!(
        bare.command_stream_absent().await,
        "verify must not create the fixed stream it failed to find"
    );

    bare.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn verify_fails_while_the_durable_is_absent_and_passes_once_provisioned() {
    let harness = FabricTestNats::start().await;
    let dir = tempfile::tempdir().expect("manifest dir");
    let manifest = write_manifest(dir.path(), ACCEPTED_MANIFEST);

    let absent = cli("verify", &harness.url(), &manifest).await;
    assert_eq!(absent.status.code(), Some(4), "{}", rendered(&absent));
    assert!(
        stderr(&absent).contains("verify_probe_cmd") && stderr(&absent).contains("absent"),
        "the failure must name the missing durable: {}",
        rendered(&absent)
    );
    assert!(
        harness
            .durable_filter_subjects_if_present("INTEGRATION_CMD", "verify_probe_cmd")
            .await
            .is_none(),
        "verify must not create the durable it failed to find"
    );

    let provisioned = cli("provision", &harness.url(), &manifest).await;
    assert!(provisioned.status.success(), "{}", rendered(&provisioned));

    let verified = cli("verify", &harness.url(), &manifest).await;
    assert!(verified.status.success(), "{}", rendered(&verified));
    let stdout = String::from_utf8_lossy(&verified.stdout).to_string();
    assert!(
        stdout.contains(
            "ok cmd verify_probe_cmd -> integration.cmd.identity.service_scope.declare.v1"
        ) && stdout.contains(
            "ok evt verify_probe_evt -> integration.evt.identity.service_scope.accepted.v1"
        ),
        "verify must report the coordinate it checked: {stdout}"
    );

    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` on PATH"]
async fn verify_rejects_a_durable_whose_filter_is_not_the_coordinate() {
    let harness = FabricTestNats::start().await;
    let dir = tempfile::tempdir().expect("manifest dir");
    let provisioned = write_manifest(dir.path(), DECLARE_MANIFEST);
    let reaimed = write_manifest(dir.path(), REAIMED_MANIFEST);

    let out = cli("provision", &harness.url(), &provisioned).await;
    assert!(out.status.success(), "{}", rendered(&out));

    let out = cli("verify", &harness.url(), &reaimed).await;

    assert_eq!(out.status.code(), Some(4), "{}", rendered(&out));
    assert!(
        stderr(&out).contains("integration.cmd.identity.service_scope.declare.v1")
            && stderr(&out).contains("integration.cmd.identity.service_scope.revoke.v1"),
        "the failure must name both the filter found and the coordinate expected: {}",
        rendered(&out)
    );

    harness.shutdown().await;
}

fn write_manifest(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join(format!("{}.toml", uuid::Uuid::now_v7().simple()));
    std::fs::write(&path, body).expect("write the fabric-nats manifest");
    path
}

async fn cli(subcommand: &str, nats: &str, manifest: &Path) -> Output {
    let bin = workspace_bin("fabric-nats");
    run_once(
        &bin.to_string_lossy(),
        &[
            subcommand,
            "--nats",
            nats,
            "--manifest",
            &manifest.to_string_lossy(),
        ],
        &[],
        Duration::from_secs(30),
    )
    .await
    .unwrap_or_else(|e| panic!("spawning fabric-nats {subcommand}: {e}"))
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

fn rendered(out: &Output) -> String {
    format!(
        "status={:?}\nstdout={}\nstderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        stderr(out)
    )
}
