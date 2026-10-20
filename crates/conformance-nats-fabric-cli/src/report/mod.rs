use conformance_nats_fabric::{CheckOutcome, ConformanceReport, redact_nats_url};
use serde::Serialize;

use crate::cli::Format;

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub mode: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nats: Option<String>,
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

impl Report {
    pub fn spawned(report: &ConformanceReport) -> Self {
        Self::from_report("spawn", None, report)
    }

    pub fn attached(nats_url: &str, report: &ConformanceReport) -> Self {
        Self::from_report("attach", Some(redact_nats_url(nats_url)), report)
    }

    fn from_report(mode: &'static str, nats: Option<String>, report: &ConformanceReport) -> Self {
        Self {
            mode,
            nats,
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
mod tests;
