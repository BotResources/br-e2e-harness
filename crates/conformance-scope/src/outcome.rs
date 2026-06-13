use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckId {
    DeclareWellFormed,
    DeclarationContent,
    ReadinessGated,
    RepublishesSameCorrelationId,
    RejectionStopsReadiness,
    DuplicateConfirmationsTolerated,
    DisabledModeReadyWithoutDeclare,
}

impl CheckId {
    pub fn code(self) -> &'static str {
        match self {
            CheckId::DeclareWellFormed => "s1",
            CheckId::ReadinessGated => "s2",
            CheckId::RepublishesSameCorrelationId => "s3",
            CheckId::RejectionStopsReadiness => "s4",
            CheckId::DuplicateConfirmationsTolerated => "s5",
            CheckId::DisabledModeReadyWithoutDeclare => "s6",
            CheckId::DeclarationContent => "declaration-content",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "s1" => Some(CheckId::DeclareWellFormed),
            "s2" => Some(CheckId::ReadinessGated),
            "s3" => Some(CheckId::RepublishesSameCorrelationId),
            "s4" => Some(CheckId::RejectionStopsReadiness),
            "s5" => Some(CheckId::DuplicateConfirmationsTolerated),
            "s6" => Some(CheckId::DisabledModeReadyWithoutDeclare),
            "declaration-content" => Some(CheckId::DeclarationContent),
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
        self.failed() == 0
    }
}
