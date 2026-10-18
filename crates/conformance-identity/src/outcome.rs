use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckId {
    CleanDeclarationAccepted,
    OwnedScopeReclaimAfterPriorAccept,
    IntraDeclarationDuplicateRejected,
    PrefixMismatchRejected,
    InvalidScopeKeyRejected,
    IdempotentRedeclareAccepted,
    MalformedScopeKeyRejected,
}

impl CheckId {
    pub fn code(self) -> &'static str {
        match self {
            CheckId::CleanDeclarationAccepted => "a1",
            CheckId::OwnedScopeReclaimAfterPriorAccept => "a2",
            CheckId::IntraDeclarationDuplicateRejected => "a3",
            CheckId::PrefixMismatchRejected => "a4",
            CheckId::InvalidScopeKeyRejected => "a5",
            CheckId::IdempotentRedeclareAccepted => "a6",
            CheckId::MalformedScopeKeyRejected => "a7",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "a1" => Some(CheckId::CleanDeclarationAccepted),
            "a2" => Some(CheckId::OwnedScopeReclaimAfterPriorAccept),
            "a3" => Some(CheckId::IntraDeclarationDuplicateRejected),
            "a4" => Some(CheckId::PrefixMismatchRejected),
            "a5" => Some(CheckId::InvalidScopeKeyRejected),
            "a6" => Some(CheckId::IdempotentRedeclareAccepted),
            "a7" => Some(CheckId::MalformedScopeKeyRejected),
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

    fn pass() -> CheckOutcome {
        CheckOutcome::pass(CheckId::CleanDeclarationAccepted, "e", "o")
    }

    fn fail() -> CheckOutcome {
        CheckOutcome::fail(CheckId::PrefixMismatchRejected, "e", "o", "d")
    }

    fn skipped() -> CheckOutcome {
        CheckOutcome::skipped(CheckId::InvalidScopeKeyRejected, "d")
    }

    fn report(outcomes: Vec<CheckOutcome>) -> ConformanceReport {
        ConformanceReport { outcomes }
    }

    #[test]
    fn a_report_with_no_outcome_is_not_conformant() {
        assert!(!ConformanceReport::default().is_conformant());
    }

    #[test]
    fn a_report_with_only_skipped_outcomes_is_not_conformant() {
        assert!(!report(vec![skipped(), skipped()]).is_conformant());
    }

    #[test]
    fn a_report_with_a_failure_is_not_conformant() {
        assert!(!report(vec![pass(), fail()]).is_conformant());
    }

    #[test]
    fn a_report_of_passes_is_conformant() {
        assert!(report(vec![pass()]).is_conformant());
        assert!(report(vec![pass(), pass()]).is_conformant());
    }

    #[test]
    fn a_pass_beside_a_skip_is_not_conformant() {
        assert!(!report(vec![pass(), skipped()]).is_conformant());
    }
}
