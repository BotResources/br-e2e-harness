use conformance_passport::{CheckOutcome, ConformanceReport};
use serde::Serialize;

use crate::cli::Format;

#[derive(Debug, Clone, Serialize)]
pub struct SubjectReport {
    pub subject: String,
    pub outcomes: Vec<OutcomeView>,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub conformant: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutcomeView {
    pub check: String,
    pub status: String,
    #[serde(skip)]
    pub glyph: &'static str,
    pub expected: String,
    pub observed: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl SubjectReport {
    pub fn from_report(subject: impl Into<String>, report: &ConformanceReport) -> Self {
        Self {
            subject: subject.into(),
            outcomes: report.outcomes.iter().map(OutcomeView::from).collect(),
            passed: report.passed(),
            failed: report.failed(),
            skipped: report.skipped(),
            conformant: report.is_conformant(),
        }
    }

    pub fn render(&self, format: Format) -> String {
        match format {
            Format::Human => crate::render::human(self),
            Format::Json => crate::render::json(self),
            Format::Junit => crate::render::junit(self),
        }
    }
}

impl From<&CheckOutcome> for OutcomeView {
    fn from(o: &CheckOutcome) -> Self {
        Self {
            check: o.id.code().to_string(),
            status: o.status.code().to_string(),
            glyph: o.status.glyph(),
            expected: o.expected.clone(),
            observed: o.observed.clone(),
            detail: o.detail.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use conformance_passport::{CheckId, CheckOutcome};

    use super::*;

    fn report_of(outcomes: Vec<CheckOutcome>) -> ConformanceReport {
        let mut report = ConformanceReport::default();
        for outcome in outcomes {
            report.push(outcome);
        }
        report
    }

    #[test]
    fn an_all_pass_report_is_conformant() {
        let report = SubjectReport::from_report(
            "svc",
            &report_of(vec![CheckOutcome::pass(
                CheckId::ValidBearerResolvesToPassport,
                "200",
                "200",
            )]),
        );
        assert!(report.conformant);
        assert_eq!((report.passed, report.failed, report.skipped), (1, 0, 0));
    }

    #[test]
    fn a_failure_makes_the_report_non_conformant() {
        let report = SubjectReport::from_report(
            "svc",
            &report_of(vec![
                CheckOutcome::pass(CheckId::NoCredentialIsAnonymous, "200", "200"),
                CheckOutcome::fail(CheckId::KvErrorFailsLoud, "5xx", "200", "silent 200"),
            ]),
        );
        assert!(!report.conformant);
        assert_eq!((report.passed, report.failed), (1, 1));
    }

    #[test]
    fn a_skipped_check_is_never_conformant() {
        let report = SubjectReport::from_report(
            "svc",
            &report_of(vec![CheckOutcome::skipped(
                CheckId::KvErrorFailsLoud,
                "unproven",
            )]),
        );
        assert!(!report.conformant);
    }

    #[test]
    fn an_empty_report_is_never_conformant() {
        let report = SubjectReport::from_report("svc", &ConformanceReport::default());
        assert!(!report.conformant);
    }
}
