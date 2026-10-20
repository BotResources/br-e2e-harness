use std::fmt::Write as _;

use crate::report::{OutcomeView, Report};

pub fn human(report: &Report) -> String {
    let mut out = String::new();
    let target = report.nats.as_deref().unwrap_or("throwaway nats-server");
    let _ = writeln!(out, "mode: {} ({target})", report.mode);
    for outcome in &report.outcomes {
        human_outcome(&mut out, outcome);
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

fn human_outcome(out: &mut String, outcome: &OutcomeView) {
    let _ = writeln!(out, "  [{}] {}", outcome.glyph, outcome.check);
    if outcome.status == "fail" {
        let _ = writeln!(out, "      expected: {}", outcome.expected);
        for (index, line) in outcome.observed.lines().enumerate() {
            let label = if index == 0 {
                "observed: "
            } else {
                "          "
            };
            let _ = writeln!(out, "      {label}{line}");
        }
    } else if outcome.status == "skipped"
        && let Some(detail) = &outcome.detail
    {
        let _ = writeln!(out, "      {detail}");
    }
}

pub fn json(report: &Report) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

pub fn junit(report: &Report) -> String {
    let mut out = String::new();
    let total = report.passed + report.failed + report.skipped;
    let _ = writeln!(out, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    let _ = writeln!(
        out,
        "<testsuites tests=\"{total}\" failures=\"{}\" skipped=\"{}\">",
        report.failed, report.skipped
    );
    let _ = writeln!(
        out,
        "  <testsuite name=\"conformance-nats-fabric\" tests=\"{total}\" failures=\"{}\" skipped=\"{}\">",
        report.failed, report.skipped
    );
    for outcome in &report.outcomes {
        junit_case(&mut out, outcome);
    }
    let _ = writeln!(out, "  </testsuite>");
    let _ = writeln!(out, "</testsuites>");
    out
}

fn junit_case(out: &mut String, outcome: &OutcomeView) {
    let _ = write!(
        out,
        "    <testcase classname=\"conformance-nats-fabric\" name=\"{}\"",
        xml_escape(&outcome.check)
    );
    match outcome.status.as_str() {
        "fail" => {
            let _ = writeln!(out, ">");
            let _ = writeln!(
                out,
                "      <failure message=\"{}\">{}</failure>",
                xml_escape(&format!("expected: {}", outcome.expected)),
                xml_escape(&outcome.observed)
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

#[cfg(test)]
mod tests;
