use std::fmt;

use crate::error::{ConformanceError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckId {
    AnchorSubjectsMatchLib,
    AnchorUsersDeserializeThroughLib,
    WidenedDurableConverges,
    MissingStreamFailsCoverageProbe,
    MissingStreamFailsDurableBind,
    DeadGrammarFailsLoud,
    PublishedLanguageRetractOrphanDeletes,
    PublishedLanguageReconcileConverges,
    PublishedLanguageBootstrapThenWatch,
    PublishedLanguagePrefixWatchDelivers,
    PublishedLanguageDecodeFailsClosed,
    PublishedLanguageAnchorPoisonFailsClosed,
}

impl CheckId {
    pub const ALL: [CheckId; 12] = [
        CheckId::AnchorSubjectsMatchLib,
        CheckId::AnchorUsersDeserializeThroughLib,
        CheckId::WidenedDurableConverges,
        CheckId::MissingStreamFailsCoverageProbe,
        CheckId::MissingStreamFailsDurableBind,
        CheckId::DeadGrammarFailsLoud,
        CheckId::PublishedLanguageRetractOrphanDeletes,
        CheckId::PublishedLanguageReconcileConverges,
        CheckId::PublishedLanguageBootstrapThenWatch,
        CheckId::PublishedLanguagePrefixWatchDelivers,
        CheckId::PublishedLanguageDecodeFailsClosed,
        CheckId::PublishedLanguageAnchorPoisonFailsClosed,
    ];

    pub fn code(self) -> &'static str {
        match self {
            CheckId::AnchorSubjectsMatchLib => "anchor-subjects-match-lib",
            CheckId::AnchorUsersDeserializeThroughLib => "anchor-users-deserialize-through-lib",
            CheckId::WidenedDurableConverges => "widened-durable-converges",
            CheckId::MissingStreamFailsCoverageProbe => "missing-stream-fails-coverage-probe",
            CheckId::MissingStreamFailsDurableBind => "missing-stream-fails-durable-bind",
            CheckId::DeadGrammarFailsLoud => "dead-grammar-fails-loud",
            CheckId::PublishedLanguageRetractOrphanDeletes => "pl-retract-orphan-deletes",
            CheckId::PublishedLanguageReconcileConverges => "pl-reconcile-converges",
            CheckId::PublishedLanguageBootstrapThenWatch => "pl-bootstrap-then-watch",
            CheckId::PublishedLanguagePrefixWatchDelivers => "pl-prefix-watch-delivers",
            CheckId::PublishedLanguageDecodeFailsClosed => "pl-decode-fails-closed",
            CheckId::PublishedLanguageAnchorPoisonFailsClosed => "pl-anchor-poison-fails-closed",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|id| id.code() == code)
    }

    pub fn expectation(self) -> &'static str {
        match self {
            CheckId::AnchorSubjectsMatchLib => {
                "every subject the Go anchor renders equals the lib renderer's output byte for byte"
            }
            CheckId::AnchorUsersDeserializeThroughLib => {
                "every user the Go anchor freezes deserializes through the lib's PublishedUser"
            }
            CheckId::WidenedDurableConverges => {
                "a durable widened to integration.evt.> is narrowed back to the exact coordinate filter by the lib's create-or-bind"
            }
            CheckId::MissingStreamFailsCoverageProbe => {
                "on a server without the fixed streams the coverage probe fails with Consume(NoStream)"
            }
            CheckId::MissingStreamFailsDurableBind => {
                "on a server without the fixed streams the durable bind fails with Consume(NoStream) and creates no stream"
            }
            CheckId::DeadGrammarFailsLoud => {
                "a publish on the dead identity.cmd.* grammar lands on no fixed stream and no stream captures it"
            }
            CheckId::PublishedLanguageRetractOrphanDeletes => {
                "retract orphan-deletes the key from PUBLISHED_LANGUAGE"
            }
            CheckId::PublishedLanguageReconcileConverges => {
                "reconcile adds the missing, repairs the changed and deletes the orphaned entries"
            }
            CheckId::PublishedLanguageBootstrapThenWatch => {
                "bootstrap is parallel-safe under a live watch"
            }
            CheckId::PublishedLanguagePrefixWatchDelivers => {
                "a live slash-keyed put is delivered by the prefix watch within the deadline"
            }
            CheckId::PublishedLanguageDecodeFailsClosed => {
                "a malformed value fails closed with a Decode error naming the offending key"
            }
            CheckId::PublishedLanguageAnchorPoisonFailsClosed => {
                "the anchor's poison value fails closed with a Decode error naming the offending key"
            }
        }
    }

    pub fn needs_throwaway_server(self) -> bool {
        matches!(
            self,
            CheckId::MissingStreamFailsCoverageProbe | CheckId::MissingStreamFailsDurableBind
        )
    }

    pub fn needs_anchor(self) -> bool {
        matches!(
            self,
            CheckId::AnchorSubjectsMatchLib
                | CheckId::AnchorUsersDeserializeThroughLib
                | CheckId::PublishedLanguageAnchorPoisonFailsClosed
        )
    }

    pub fn needs_broker(self) -> bool {
        !matches!(
            self,
            CheckId::AnchorSubjectsMatchLib | CheckId::AnchorUsersDeserializeThroughLib
        )
    }
}

impl fmt::Display for CheckId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

pub fn spawn_default() -> Vec<CheckId> {
    CheckId::ALL.to_vec()
}

pub fn attach_default() -> Vec<CheckId> {
    CheckId::ALL
        .into_iter()
        .filter(|id| !id.needs_throwaway_server())
        .collect()
}

pub fn parse_checks(csv: &str) -> Result<Vec<CheckId>> {
    let checks = csv
        .split(',')
        .map(str::trim)
        .filter(|code| !code.is_empty())
        .map(|code| {
            CheckId::from_code(code).ok_or_else(|| ConformanceError::UnknownCheck(code.to_string()))
        })
        .collect::<Result<Vec<_>>>()?;
    ensure_distinct(&checks)?;
    Ok(checks)
}

pub(crate) fn ensure_distinct(checks: &[CheckId]) -> Result<()> {
    for (index, id) in checks.iter().enumerate() {
        if checks[..index].contains(id) {
            return Err(ConformanceError::DuplicateCheck(id.code().to_string()));
        }
    }
    Ok(())
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
    pub fn pass(id: CheckId) -> Self {
        Self {
            id,
            status: CheckStatus::Pass,
            expected: id.expectation().to_string(),
            observed: "as expected".to_string(),
            detail: None,
        }
    }

    pub fn fail(id: CheckId, observed: impl Into<String>) -> Self {
        Self {
            id,
            status: CheckStatus::Fail,
            expected: id.expectation().to_string(),
            observed: observed.into(),
            detail: None,
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
}

#[derive(Debug, Clone, Default)]
pub struct ConformanceReport {
    pub outcomes: Vec<CheckOutcome>,
}

impl ConformanceReport {
    fn count(&self, status: CheckStatus) -> usize {
        self.outcomes.iter().filter(|o| o.status == status).count()
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

    pub fn is_conformant(&self) -> bool {
        self.failed() == 0
    }
}

#[cfg(test)]
mod tests;
