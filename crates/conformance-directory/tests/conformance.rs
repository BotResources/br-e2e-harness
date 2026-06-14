use conformance_directory::{
    CheckOutcome, build_and_emit, consumer_reads_groups, consumer_reads_users, publisher_floor,
    publisher_groups_optional, run_wire_battery,
};

fn assert_pass(outcome: &CheckOutcome) {
    assert!(
        outcome.is_pass(),
        "{} must pass: expected={:?} observed={:?} detail={:?}",
        outcome.id.code(),
        outcome.expected,
        outcome.observed,
        outcome.detail,
    );
}

#[tokio::test]
#[ignore = "wire gate: needs `go` on PATH to build the identity-directory anchor"]
async fn wire_battery_is_conformant_against_the_go_anchor() {
    let snapshot = build_and_emit()
        .await
        .expect("build + emit the anchor snapshot");
    let report = run_wire_battery(&snapshot);
    assert!(
        report.is_conformant(),
        "the W1..W5 wire-deser gate must be conformant: {} passed, {} failed\n{:#?}",
        report.passed(),
        report.failed(),
        report.outcomes,
    );
    assert_eq!(report.passed(), 5, "all five wire checks must pass");
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn p1_publisher_floor() {
    let snapshot = build_and_emit().await.expect("anchor snapshot");
    assert_pass(&publisher_floor(&snapshot).await.expect("p1"));
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + `go` on PATH"]
async fn p2_publisher_groups_optional() {
    let snapshot = build_and_emit().await.expect("anchor snapshot");
    assert_pass(&publisher_groups_optional(&snapshot).await.expect("p2"));
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + a Postgres + `go` on PATH"]
async fn c1_consumer_reads_users() {
    let snapshot = build_and_emit().await.expect("anchor snapshot");
    assert_pass(&consumer_reads_users(&snapshot).await.expect("c1"));
}

#[tokio::test]
#[ignore = "real-infra: needs `nats-server` + a Postgres + `go` on PATH"]
async fn c2_consumer_reads_groups() {
    let snapshot = build_and_emit().await.expect("anchor snapshot");
    assert_pass(&consumer_reads_groups(&snapshot).await.expect("c2"));
}
