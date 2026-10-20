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
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if is_xml_char(c) => out.push(c),
            _ => out.push('\u{FFFD}'),
        }
    }
    out
}

fn is_xml_char(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}'
    )
}

#[cfg(test)]
mod tests {
    use conformance_scope::{CheckId, CheckOutcome, ConformanceReport};

    use super::*;

    const HOSTILE: &str = "a&b <c> \"d\" 'e'";
    const HOSTILE_ESCAPED: &str = "a&amp;b &lt;c&gt; &quot;d&quot; &apos;e&apos;";

    fn aggregate(outcomes: Vec<CheckOutcome>) -> AggregateReport {
        AggregateReport::single(ServiceReport::from_report(
            "svc",
            &ConformanceReport { outcomes },
        ))
    }

    fn pass() -> CheckOutcome {
        CheckOutcome::pass(CheckId::DeclareWellFormed, "e", "o")
    }

    fn fail() -> CheckOutcome {
        CheckOutcome::fail(CheckId::DeclarationContent, HOSTILE, HOSTILE, HOSTILE)
    }

    fn skipped() -> CheckOutcome {
        CheckOutcome::skipped(CheckId::RejectionStopsReadiness, "needs a lifecycle")
    }

    #[test]
    fn junit_counts_a_pass_a_fail_and_a_skip_and_marks_each_case() {
        let xml = junit(&aggregate(vec![pass(), fail(), skipped()]));
        assert!(
            xml.contains("<testsuites tests=\"3\" failures=\"1\" skipped=\"1\">"),
            "{xml}"
        );
        assert!(
            xml.contains("<testsuite name=\"svc\" tests=\"3\" failures=\"1\" skipped=\"1\">"),
            "{xml}"
        );
        assert!(
            xml.contains("name=\"s1\"/>") || xml.contains("name=\"declare-well-formed\"/>"),
            "a passing case is a self-closing testcase: {xml}"
        );
        assert_eq!(xml.matches("<failure ").count(), 1, "{xml}");
        assert_eq!(xml.matches("<skipped/>").count(), 1, "{xml}");
    }

    #[test]
    fn junit_escapes_the_five_xml_specials_in_the_failure_message_and_body() {
        let xml = junit(&aggregate(vec![fail()]));
        let expected_message = format!(
            "<failure message=\"expected: {HOSTILE_ESCAPED}; observed: {HOSTILE_ESCAPED}\">"
        );
        assert!(xml.contains(&expected_message), "{xml}");
        assert!(
            xml.contains(&format!("{HOSTILE_ESCAPED}</failure>")),
            "{xml}"
        );
        assert!(!xml.contains(HOSTILE), "{xml}");
    }

    #[test]
    fn junit_escapes_the_service_key_and_check_attributes() {
        let report = AggregateReport::single(ServiceReport::from_report(
            HOSTILE,
            &ConformanceReport {
                outcomes: vec![pass()],
            },
        ));
        let xml = junit(&report);
        assert!(
            xml.contains(&format!("<testsuite name=\"{HOSTILE_ESCAPED}\"")),
            "{xml}"
        );
        assert!(
            xml.contains(&format!("classname=\"{HOSTILE_ESCAPED}\"")),
            "{xml}"
        );
    }

    #[test]
    fn junit_replaces_the_characters_xml_forbids_in_every_attribute_and_text() {
        let hostile = "a\u{0}b\u{1b}[31mred\u{1b}[0m\u{FFFE}c\u{FFFF}d\u{b}e\tf\ng\rh";
        let outcome = CheckOutcome::fail(CheckId::DeclarationContent, hostile, hostile, hostile);
        let report = AggregateReport::single(ServiceReport::from_report(
            hostile,
            &ConformanceReport {
                outcomes: vec![outcome],
            },
        ));
        let xml = junit(&report);
        assert!(xml.chars().all(is_xml_char), "{xml:?}");
        assert!(
            xml.contains("a\u{FFFD}b\u{FFFD}[31mred\u{FFFD}[0m\u{FFFD}c\u{FFFD}d\u{FFFD}e"),
            "{xml:?}"
        );
        assert!(xml.contains("e\tf\ng\rh"), "{xml:?}");
    }

    #[test]
    fn xml_escape_keeps_astral_and_non_ascii_characters() {
        assert_eq!(xml_escape("é€😀\u{10FFFF}"), "é€😀\u{10FFFF}");
    }

    #[test]
    fn human_shows_the_diff_of_a_failure_and_the_detail_of_a_skip() {
        let text = human(&aggregate(vec![pass(), fail(), skipped()]));
        assert!(text.contains("service: svc"), "{text}");
        assert!(text.contains("[PASS]"), "{text}");
        assert!(text.contains("[FAIL]"), "{text}");
        assert!(text.contains("[SKIP]"), "{text}");
        assert!(text.contains(&format!("expected: {HOSTILE}")), "{text}");
        assert!(text.contains(&format!("observed: {HOSTILE}")), "{text}");
        assert!(text.contains("needs a lifecycle"), "{text}");
        assert!(
            text.contains("NON-CONFORMANT: 1 passed, 1 failed, 1 skipped"),
            "{text}"
        );
    }

    #[test]
    fn human_states_conformant_when_every_check_passed() {
        let text = human(&aggregate(vec![pass()]));
        assert!(
            text.contains("CONFORMANT: 1 passed, 0 failed, 0 skipped"),
            "{text}"
        );
        assert!(!text.contains("NON-CONFORMANT"), "{text}");
    }
}
