use std::fmt::Write as _;

use crate::report::{OutcomeView, SubjectReport};

pub fn human(report: &SubjectReport) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "subject: {}", visible(&report.subject));
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
    let _ = writeln!(out, "  [{}] {}", outcome.glyph, visible(&outcome.check));
    if outcome.status == "fail" {
        let _ = writeln!(out, "      expected: {}", visible(&outcome.expected));
        let _ = writeln!(out, "      observed: {}", visible(&outcome.observed));
        if let Some(detail) = &outcome.detail {
            for line in detail.lines() {
                let _ = writeln!(out, "      {}", visible(line));
            }
        }
    } else if outcome.status == "skipped"
        && let Some(detail) = &outcome.detail
    {
        let _ = writeln!(out, "      {}", visible(detail));
    }
}

fn visible(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_control() && c != '\n' && c != '\t' {
                char::REPLACEMENT_CHARACTER
            } else {
                c
            }
        })
        .collect()
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
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if is_xml_char(c) => out.push(c),
            _ => out.push(char::REPLACEMENT_CHARACTER),
        }
    }
    out
}

fn is_xml_char(c: char) -> bool {
    matches!(
        c,
        '\u{9}' | '\u{A}' | '\u{D}' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}'
    )
}

#[cfg(test)]
mod tests {
    use conformance_identity::{CheckId, CheckOutcome, ConformanceReport};

    use super::*;

    fn failing_report() -> SubjectReport {
        let mut report = ConformanceReport::default();
        report.push(CheckOutcome::pass(
            CheckId::PrefixMismatchRejected,
            "200",
            "200",
        ));
        report.push(CheckOutcome::fail(
            CheckId::InvalidScopeKeyRejected,
            "a 5xx",
            "<200> & \"ok\"",
            "silent 200",
        ));
        SubjectReport::from_report("my-svc", &report)
    }

    #[test]
    fn human_marks_the_failure_with_expected_and_observed() {
        let text = human(&failing_report());
        assert!(text.contains("[FAIL] a5"), "{text}");
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

    fn hostile_report() -> SubjectReport {
        let mut report = ConformanceReport::default();
        report.push(CheckOutcome::fail(
            CheckId::InvalidScopeKeyRejected,
            "ex\u{1b}pected\u{0}",
            "obs\u{1b}[31mer\u{0}ved\u{FFFE}",
            "de\u{1b}tail\u{0}\nsecond\tline",
        ));
        SubjectReport::from_report("sub\u{1b}ject", &report)
    }

    fn forbidden(c: char) -> bool {
        !is_xml_char(c)
    }

    #[test]
    fn junit_holds_no_char_forbidden_by_xml() {
        let text = junit(&hostile_report());
        assert!(!text.chars().any(forbidden), "{text:?}");
        assert!(text.contains('\u{FFFD}'), "{text:?}");
    }

    #[test]
    fn human_holds_no_control_char_but_newline_and_tab() {
        let text = human(&hostile_report());
        assert!(
            !text
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t'),
            "{text:?}"
        );
        assert!(text.contains("second\tline"), "{text:?}");
    }

    #[test]
    fn json_carries_the_verdict_and_the_check_codes() {
        let value: serde_json::Value = serde_json::from_str(&json(&failing_report())).unwrap();
        assert_eq!(value["conformant"], serde_json::json!(false));
        assert_eq!(value["outcomes"][1]["check"], serde_json::json!("a5"));
        assert_eq!(value["outcomes"][1]["status"], serde_json::json!("fail"));
    }
}
