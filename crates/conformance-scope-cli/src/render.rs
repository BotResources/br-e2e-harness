use std::fmt::Write as _;

use crate::report::{AggregateReport, OutcomeView, ServiceReport};

pub fn human(report: &AggregateReport) -> String {
    let mut out = String::new();
    for service in &report.services {
        human_service(&mut out, service);
    }
    let verdict = if report.conformant {
        "CONFORMANT"
    } else {
        "NON-CONFORMANT"
    };
    let _ = writeln!(
        out,
        "{verdict}: {} passed, {} failed, {} skipped",
        report.passed, report.failed, report.skipped
    );
    out
}

fn human_service(out: &mut String, service: &ServiceReport) {
    let _ = writeln!(out, "service: {}", service.service_key);
    for outcome in &service.outcomes {
        human_outcome(out, outcome);
    }
    let _ = writeln!(out);
}

fn human_outcome(out: &mut String, outcome: &OutcomeView) {
    let _ = writeln!(out, "  [{}] {}", outcome.glyph, outcome.check);
    if outcome.status == "fail" {
        let _ = writeln!(out, "      expected: {}", outcome.expected);
        let _ = writeln!(out, "      observed: {}", outcome.observed);
        if let Some(detail) = &outcome.detail {
            for line in detail.lines() {
                let _ = writeln!(out, "      {line}");
            }
        }
    } else if outcome.status == "skipped"
        && let Some(detail) = &outcome.detail
    {
        let _ = writeln!(out, "      {detail}");
    }
}

pub fn json(report: &AggregateReport) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

pub fn junit(report: &AggregateReport) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    let _ = writeln!(
        out,
        "<testsuites tests=\"{}\" failures=\"{}\" skipped=\"{}\">",
        report.passed + report.failed + report.skipped,
        report.failed,
        report.skipped
    );
    for service in &report.services {
        junit_service(&mut out, service);
    }
    let _ = writeln!(out, "</testsuites>");
    out
}

fn junit_service(out: &mut String, service: &ServiceReport) {
    let total = service.passed + service.failed + service.skipped;
    let _ = writeln!(
        out,
        "  <testsuite name=\"{}\" tests=\"{total}\" failures=\"{}\" skipped=\"{}\">",
        xml_escape(&service.service_key),
        service.failed,
        service.skipped
    );
    for outcome in &service.outcomes {
        junit_case(out, &service.service_key, outcome);
    }
    let _ = writeln!(out, "  </testsuite>");
}

fn junit_case(out: &mut String, service_key: &str, outcome: &OutcomeView) {
    let _ = write!(
        out,
        "    <testcase classname=\"{}\" name=\"{}\"",
        xml_escape(service_key),
        xml_escape(&outcome.check)
    );
    match outcome.status.as_str() {
        "fail" => {
            let _ = writeln!(out, ">");
            let message = format!(
                "expected: {}; observed: {}",
                outcome.expected, outcome.observed
            );
            let body = outcome.detail.clone().unwrap_or_default();
            let _ = writeln!(
                out,
                "      <failure message=\"{}\">{}</failure>",
                xml_escape(&message),
                xml_escape(&body)
            );
            let _ = writeln!(out, "    </testcase>");
        }
        "skipped" => {
            let _ = writeln!(out, ">");
            let _ = writeln!(out, "      <skipped/>");
            let _ = writeln!(out, "    </testcase>");
        }
        _ => {
            let _ = writeln!(out, "/>");
        }
    }
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
