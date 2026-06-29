use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckId {
    ValidBearerResolvesToPassport,
    RevokedBearerIsAnonymous,
    UnknownBearerIsAnonymous,
    NoCredentialIsAnonymous,
    DistinctTokensDistinctPassports,
    WrongSealKeyFailsClosed,
    TamperedEnvelopeFailsClosed,
    KvErrorIs500,
    ScopesClaimRoundTrip,
}

impl CheckId {
    pub fn code(self) -> &'static str {
        match self {
            CheckId::ValidBearerResolvesToPassport => "p1",
            CheckId::RevokedBearerIsAnonymous => "p2",
            CheckId::UnknownBearerIsAnonymous => "p3",
            CheckId::NoCredentialIsAnonymous => "p4",
            CheckId::DistinctTokensDistinctPassports => "p5",
            CheckId::WrongSealKeyFailsClosed => "p6",
            CheckId::TamperedEnvelopeFailsClosed => "p7",
            CheckId::KvErrorIs500 => "p8",
            CheckId::ScopesClaimRoundTrip => "g4",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "p1" => Some(CheckId::ValidBearerResolvesToPassport),
            "p2" => Some(CheckId::RevokedBearerIsAnonymous),
            "p3" => Some(CheckId::UnknownBearerIsAnonymous),
            "p4" => Some(CheckId::NoCredentialIsAnonymous),
            "p5" => Some(CheckId::DistinctTokensDistinctPassports),
            "p6" => Some(CheckId::WrongSealKeyFailsClosed),
            "p7" => Some(CheckId::TamperedEnvelopeFailsClosed),
            "p8" => Some(CheckId::KvErrorIs500),
            "g4" => Some(CheckId::ScopesClaimRoundTrip),
            _ => None,
        }
    }
}

impl fmt::Display for CheckId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    Pass,
    Fail,
    Skipped,
}

impl CheckStatus {
    pub fn code(self) -> &'static str {
        match self {
            CheckStatus::Pass => "pass",
            CheckStatus::Fail => "fail",
            CheckStatus::Skipped => "skipped",
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            CheckStatus::Pass => "PASS",
            CheckStatus::Fail => "FAIL",
            CheckStatus::Skipped => "SKIP",
        }
    }
}

impl fmt::Display for CheckStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

#[derive(Debug, Clone)]
pub struct CheckOutcome {
    pub id: CheckId,
    pub status: CheckStatus,
    pub expected: String,
    pub observed: String,
    pub detail: Option<String>,
}

impl CheckOutcome {
    pub fn pass(id: CheckId, expected: impl Into<String>, observed: impl Into<String>) -> Self {
        Self {
            id,
            status: CheckStatus::Pass,
            expected: expected.into(),
            observed: observed.into(),
            detail: None,
        }
    }

    pub fn fail(
        id: CheckId,
        expected: impl Into<String>,
        observed: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            id,
            status: CheckStatus::Fail,
            expected: expected.into(),
            observed: observed.into(),
            detail: Some(detail.into()),
        }
    }

    pub fn skipped(id: CheckId, detail: impl Into<String>) -> Self {
        Self {
            id,
            status: CheckStatus::Skipped,
            expected: String::new(),
            observed: String::new(),
            detail: Some(detail.into()),
        }
    }

    pub fn is_pass(&self) -> bool {
        self.status == CheckStatus::Pass
    }
}

#[derive(Debug, Clone, Default)]
pub struct ConformanceReport {
    pub outcomes: Vec<CheckOutcome>,
}

impl ConformanceReport {
    pub fn push(&mut self, outcome: CheckOutcome) {
        self.outcomes.push(outcome);
    }

    pub fn extend(&mut self, other: ConformanceReport) {
        self.outcomes.extend(other.outcomes);
    }

    pub fn passed(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|o| o.status == CheckStatus::Pass)
            .count()
    }

    pub fn failed(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|o| o.status == CheckStatus::Fail)
            .count()
    }

    pub fn skipped(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|o| o.status == CheckStatus::Skipped)
            .count()
    }

    pub fn is_conformant(&self) -> bool {
        !self.outcomes.is_empty() && self.failed() == 0 && self.skipped() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_report_is_not_conformant() {
        assert!(!ConformanceReport::default().is_conformant());
    }

    #[test]
    fn a_skipped_only_report_is_not_conformant() {
        let mut report = ConformanceReport::default();
        report.push(CheckOutcome::skipped(CheckId::KvErrorIs500, "no infra"));
        assert!(!report.is_conformant());
    }

    #[test]
    fn an_all_pass_report_is_conformant() {
        let mut report = ConformanceReport::default();
        report.push(CheckOutcome::pass(
            CheckId::ValidBearerResolvesToPassport,
            "ok",
            "ok",
        ));
        assert!(report.is_conformant());
    }
}
