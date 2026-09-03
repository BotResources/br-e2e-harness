use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckId {
    WireUserDeserializes,
    WireGroupDeserializes,
    WireMetaDeserializes,
    WireExtensionRidesFlat,
    WireMetaAutoDegrades,
    PublisherFloor,
    PublisherGroupsOptional,
    ConsumerReadsUsers,
    ConsumerReadsGroups,
    ConsumerExtensionSurvives,
    ConsumerFilterFlipOrphans,
    WireReservedKeyRejected,
    ConsumerUsersOnlyNarrows,
    ConsumerStagerTransaction,
}

impl CheckId {
    pub fn code(self) -> &'static str {
        match self {
            CheckId::WireUserDeserializes => "w1",
            CheckId::WireGroupDeserializes => "w2",
            CheckId::WireMetaDeserializes => "w3",
            CheckId::WireExtensionRidesFlat => "w4",
            CheckId::WireMetaAutoDegrades => "w5",
            CheckId::PublisherFloor => "p1",
            CheckId::PublisherGroupsOptional => "p2",
            CheckId::ConsumerReadsUsers => "c1",
            CheckId::ConsumerReadsGroups => "c2",
            CheckId::ConsumerExtensionSurvives => "c3",
            CheckId::ConsumerFilterFlipOrphans => "c4",
            CheckId::WireReservedKeyRejected => "w6",
            CheckId::ConsumerUsersOnlyNarrows => "c5",
            CheckId::ConsumerStagerTransaction => "c6",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "w1" => Some(CheckId::WireUserDeserializes),
            "w2" => Some(CheckId::WireGroupDeserializes),
            "w3" => Some(CheckId::WireMetaDeserializes),
            "w4" => Some(CheckId::WireExtensionRidesFlat),
            "w5" => Some(CheckId::WireMetaAutoDegrades),
            "p1" => Some(CheckId::PublisherFloor),
            "p2" => Some(CheckId::PublisherGroupsOptional),
            "c1" => Some(CheckId::ConsumerReadsUsers),
            "c2" => Some(CheckId::ConsumerReadsGroups),
            "c3" => Some(CheckId::ConsumerExtensionSurvives),
            "c4" => Some(CheckId::ConsumerFilterFlipOrphans),
            "w6" => Some(CheckId::WireReservedKeyRejected),
            "c5" => Some(CheckId::ConsumerUsersOnlyNarrows),
            "c6" => Some(CheckId::ConsumerStagerTransaction),
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
        self.count(CheckStatus::Pass)
    }

    pub fn failed(&self) -> usize {
        self.count(CheckStatus::Fail)
    }

    pub fn skipped(&self) -> usize {
        self.count(CheckStatus::Skipped)
    }

    fn count(&self, status: CheckStatus) -> usize {
        self.outcomes.iter().filter(|o| o.status == status).count()
    }

    pub fn is_conformant(&self) -> bool {
        self.failed() == 0
    }
}
