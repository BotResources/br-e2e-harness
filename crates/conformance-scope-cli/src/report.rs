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
            conformant: failed == 0,
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
