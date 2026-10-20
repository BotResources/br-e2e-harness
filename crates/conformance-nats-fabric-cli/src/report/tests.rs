use conformance_nats_fabric::{CheckId, CheckOutcome, ConformanceReport};

use super::Report;

fn one_pass() -> ConformanceReport {
    let mut report = ConformanceReport::default();
    report
        .outcomes
        .push(CheckOutcome::pass(CheckId::DeadGrammarFailsLoud));
    report
}

#[test]
fn an_attached_report_names_the_server_without_its_credentials() {
    for url in [
        "nats://operator:Zk9/hunter2tail@127.0.0.1:1",
        "nats://operator:Zk9?hunter2tail@127.0.0.1:1",
        "nats://operator:Zk9,hunter2tail@127.0.0.1:1",
        "nats://a:b@10.0.0.1:4222,nats://operator:hunter2tail@10.0.0.2:4222",
    ] {
        let report = Report::attached(url, &one_pass());
        let shown = serde_json::to_string(&report).expect("serializes");
        assert!(!shown.contains("hunter2tail"), "{url}: {shown}");
        assert!(!shown.contains("Zk9"), "{url}: {shown}");
        assert!(!shown.contains("operator"), "{url}: {shown}");
        assert_eq!(report.mode, "attach");
    }
    let report = Report::attached("nats://operator:Zk9/hunter2tail@127.0.0.1:1", &one_pass());
    assert_eq!(report.nats.as_deref(), Some("nats://127.0.0.1:1"));
}

#[test]
fn a_spawned_report_names_no_server() {
    let report = Report::spawned(&one_pass());
    assert_eq!(report.mode, "spawn");
    assert_eq!(report.nats, None);
}
