use conformance_nats_fabric::{CheckId, CheckOutcome, ConformanceReport};

use super::{human, junit, xml_escape};
use crate::report::Report;

fn mixed_report() -> Report {
    let mut report = ConformanceReport::default();
    report
        .outcomes
        .push(CheckOutcome::pass(CheckId::DeadGrammarFailsLoud));
    report.outcomes.push(CheckOutcome::fail(
        CheckId::WidenedDurableConverges,
        "first <line> & \"two\" 'three'\nsecond line",
    ));
    report.outcomes.push(CheckOutcome::skipped(
        CheckId::MissingStreamFailsCoverageProbe,
        "needs a throwaway server",
    ));
    Report::spawned(&report)
}

fn full_report() -> Report {
    let mut report = ConformanceReport::default();
    for id in CheckId::ALL {
        report.outcomes.push(CheckOutcome::pass(id));
    }
    Report::spawned(&report)
}

#[test]
fn xml_escape_replaces_the_five_reserved_characters() {
    assert_eq!(
        xml_escape("a&b<c>d\"e'f"),
        "a&amp;b&lt;c&gt;d&quot;e&apos;f"
    );
    assert_eq!(xml_escape("&lt;"), "&amp;lt;");
}

#[test]
fn junit_holds_one_testcase_per_check_with_the_right_counts() {
    let xml = junit(&full_report());
    assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"));
    assert_eq!(xml.matches("<testcase ").count(), 12, "{xml}");
    assert!(xml.contains("<testsuites tests=\"12\" failures=\"0\" skipped=\"0\">"));
    assert!(xml.contains(
        "<testsuite name=\"conformance-nats-fabric\" tests=\"12\" failures=\"0\" skipped=\"0\">"
    ));
    assert!(!xml.contains("<failure"));
    assert!(!xml.contains("<skipped"));
}

#[test]
fn junit_reports_a_failure_with_its_text_escaped_and_a_skip() {
    let xml = junit(&mixed_report());
    assert!(xml.contains("<testsuites tests=\"3\" failures=\"1\" skipped=\"1\">"));
    assert_eq!(xml.matches("<testcase ").count(), 3, "{xml}");
    assert_eq!(xml.matches("</testcase>").count(), 2, "{xml}");
    assert!(xml.contains(
        "first &lt;line&gt; &amp; &quot;two&quot; &apos;three&apos;\nsecond line</failure>"
    ));
    assert!(xml.contains("<failure message=\"expected: a durable widened"));
    assert!(xml.contains("<skipped/>"));
    assert!(!xml.contains("first <line>"), "{xml}");
}

#[test]
fn junit_is_well_formed_xml() {
    let xml = junit(&mixed_report());
    let mut depth = 0i32;
    let mut rest = xml.as_str();
    while let Some(start) = rest.find('<') {
        let text = &rest[..start];
        assert!(!text.contains('>'), "raw > in text of {xml}");
        let end = rest[start..].find('>').expect("every tag closes") + start;
        let tag = &rest[start + 1..end];
        assert!(!tag.contains('<'), "raw < inside a tag of {xml}");
        if tag.starts_with('?') || tag.ends_with('/') {
        } else if tag.starts_with('/') {
            depth -= 1;
        } else {
            depth += 1;
        }
        assert!(depth >= 0, "closing tag without opener in {xml}");
        rest = &rest[end + 1..];
    }
    assert_eq!(depth, 0, "{xml}");
}

#[test]
fn human_shows_expected_and_every_observed_line_of_a_failure_and_the_reason_of_a_skip() {
    let text = human(&mixed_report());
    assert!(text.starts_with("mode: spawn (throwaway nats-server)\n"));
    assert!(text.contains("  [PASS] dead-grammar-fails-loud\n"));
    assert!(text.contains("  [FAIL] widened-durable-converges\n"));
    assert!(text.contains("      expected: a durable widened to integration.evt.>"));
    assert!(text.contains("      observed: first <line> & \"two\" 'three'\n"));
    assert!(text.contains("                second line\n"));
    assert!(text.contains("  [SKIP] missing-stream-fails-coverage-probe\n"));
    assert!(text.contains("      needs a throwaway server\n"));
    assert!(text.ends_with("NON-CONFORMANT: 1 passed, 1 failed, 1 skipped\n"));
}

#[test]
fn human_says_conformant_when_nothing_failed() {
    let text = human(&full_report());
    assert!(text.ends_with("CONFORMANT: 12 passed, 0 failed, 0 skipped\n"));
}
