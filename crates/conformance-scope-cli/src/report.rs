use conformance_scope::{CheckOutcome, ConformanceReport};
use serde::Serialize;

use crate::cli::Format;

#[derive(Debug, Clone, Serialize)]
pub struct ServiceReport {
    pub service_key: String,
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

#[derive(Debug, Clone, Serialize)]
pub struct AggregateReport {
    pub services: Vec<ServiceReport>,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub conformant: bool,
}

impl ServiceReport {
    pub fn from_report(service_key: impl Into<String>, report: &ConformanceReport) -> Self {
        Self {
            service_key: service_key.into(),
            outcomes: report.outcomes.iter().map(OutcomeView::from).collect(),
            passed: report.passed(),
            failed: report.failed(),
            skipped: report.skipped(),
            conformant: report.is_conformant(),
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

impl AggregateReport {
    pub fn from_services(services: Vec<ServiceReport>) -> Self {
        let passed = services.iter().map(|s| s.passed).sum();
        let failed = services.iter().map(|s| s.failed).sum();
        let skipped = services.iter().map(|s| s.skipped).sum();
        Self {
            conformant: !services.is_empty() && services.iter().all(|s| s.conformant),
            passed,
            failed,
            skipped,
            services,
        }
    }

    pub fn single(service: ServiceReport) -> Self {
        Self::from_services(vec![service])
    }

    pub fn render(&self, format: Format) -> String {
        match format {
            Format::Human => crate::render::human(self),
            Format::Json => crate::render::json(self),
            Format::Junit => crate::render::junit(self),
        }
    }
}

#[cfg(test)]
mod tests {
    use conformance_scope::{CheckId, CheckOutcome};

    use super::*;

    fn service(outcomes: Vec<CheckOutcome>) -> ServiceReport {
        ServiceReport::from_report("svc", &ConformanceReport { outcomes })
    }

    fn pass() -> CheckOutcome {
        CheckOutcome::pass(CheckId::DeclareWellFormed, "e", "o")
    }

    fn skipped() -> CheckOutcome {
        CheckOutcome::skipped(CheckId::RejectionStopsReadiness, "d")
    }

    #[test]
    fn an_aggregate_of_no_service_is_not_conformant() {
        assert!(!AggregateReport::from_services(Vec::new()).conformant);
    }

    #[test]
    fn a_service_that_ran_nothing_is_not_conformant_and_neither_is_the_aggregate() {
        let empty = service(Vec::new());
        assert!(!empty.conformant);
        assert!(!AggregateReport::single(empty).conformant);
    }

    #[test]
    fn a_service_with_only_skipped_checks_is_not_conformant_and_neither_is_the_aggregate() {
        let skipped_only = service(vec![skipped()]);
        assert!(!skipped_only.conformant);
        assert!(!AggregateReport::single(skipped_only).conformant);
    }

    #[test]
    fn one_non_conformant_service_makes_the_aggregate_non_conformant() {
        let aggregate =
            AggregateReport::from_services(vec![service(vec![pass()]), service(vec![skipped()])]);
        assert!(!aggregate.conformant);
        assert_eq!((aggregate.passed, aggregate.skipped), (1, 1));
    }

    #[test]
    fn conformant_services_make_a_conformant_aggregate() {
        let aggregate =
            AggregateReport::from_services(vec![service(vec![pass()]), service(vec![pass()])]);
        assert!(aggregate.conformant);
    }
}
