use std::fmt::Write as _;

use crate::report::{OutcomeView, SubjectReport};

pub fn human(report: &SubjectReport) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "subject: {}", report.subject);
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
        "\n{verdict}: {} passed, {} failed, {} skipped",
        report.passed, report.failed, report.skipped
    );
    out
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

pub fn json(report: &SubjectReport) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

pub fn junit(report: &SubjectReport) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    let total = report.passed + report.failed + report.skipped;
    let _ = writeln!(
        out,
        "<testsuites tests=\"{total}\" failures=\"{}\" skipped=\"{}\">",
        report.failed, report.skipped
    );
    let _ = writeln!(
        out,
        "  <testsuite name=\"{}\" tests=\"{total}\" failures=\"{}\" skipped=\"{}\">",
        xml_escape(&report.subject),
        report.failed,
        report.skipped
    );
    for outcome in &report.outcomes {
        junit_case(&mut out, &report.subject, outcome);
    }
    let _ = writeln!(out, "  </testsuite>");
    let _ = writeln!(out, "</testsuites>");
    out
}

fn junit_case(out: &mut String, subject: &str, outcome: &OutcomeView) {
    let _ = write!(
        out,
        "    <testcase classname=\"{}\" name=\"{}\"",
        xml_escape(subject),
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

#[cfg(test)]
mod tests {
    use conformance_passport::{CheckId, CheckOutcome, ConformanceReport};

    use super::*;

    fn failing_report() -> SubjectReport {
        let mut report = ConformanceReport::default();
        report.push(CheckOutcome::pass(
            CheckId::NoCredentialIsAnonymous,
            "200",
            "200",
        ));
        report.push(CheckOutcome::fail(
            CheckId::KvErrorFailsLoud,
            "a 5xx",
            "<200> & \"ok\"",
            "silent 200",
        ));
        SubjectReport::from_report("my-svc", &report)
    }

    #[test]
    fn human_marks_the_failure_with_expected_and_observed() {
        let text = human(&failing_report());
        assert!(text.contains("[FAIL] p8"), "{text}");
        assert!(text.contains("expected: a 5xx"), "{text}");
        assert!(
            text.contains("NON-CONFORMANT: 1 passed, 1 failed, 0 skipped"),
            "{text}"
        );
    }

    #[test]
    fn junit_escapes_the_observed_text() {
        let text = junit(&failing_report());
        assert!(text.contains("&lt;200&gt; &amp; &quot;ok&quot;"), "{text}");
        assert!(
            text.contains("<testsuite name=\"my-svc\" tests=\"2\" failures=\"1\""),
            "{text}"
        );
    }

    #[test]
    fn json_carries_the_verdict_and_the_check_codes() {
        let value: serde_json::Value = serde_json::from_str(&json(&failing_report())).unwrap();
        assert_eq!(value["conformant"], serde_json::json!(false));
        assert_eq!(value["outcomes"][1]["check"], serde_json::json!("p8"));
        assert_eq!(value["outcomes"][1]["status"], serde_json::json!("fail"));
    }
}
