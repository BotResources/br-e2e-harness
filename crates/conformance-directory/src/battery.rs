use crate::anchor::DirectorySnapshotWire;
use crate::consumer::{consumer_reads_groups, consumer_reads_users};
use crate::error::{ConformanceError, Result};
use crate::extensions::{
    extension_survives_projection, filter_flip_orphan_deletes, reserved_key_rejected,
};
use crate::outcome::{CheckId, CheckOutcome, ConformanceReport};
use crate::publisher::{publisher_floor, publisher_groups_optional};
use crate::stager::stager_stages_in_the_projection_transaction;
use crate::users_only::users_only_narrows_projection;
use crate::wire::run_wire_battery;

pub const ALL_CHECKS: [CheckId; 14] = [
    CheckId::WireUserDeserializes,
    CheckId::WireGroupDeserializes,
    CheckId::WireMetaDeserializes,
    CheckId::WireExtensionRidesFlat,
    CheckId::WireMetaAutoDegrades,
    CheckId::WireReservedKeyRejected,
    CheckId::PublisherFloor,
    CheckId::PublisherGroupsOptional,
    CheckId::ConsumerReadsUsers,
    CheckId::ConsumerReadsGroups,
    CheckId::ConsumerExtensionSurvives,
    CheckId::ConsumerFilterFlipOrphans,
    CheckId::ConsumerUsersOnlyNarrows,
    CheckId::ConsumerStagerTransaction,
];

pub fn parse_checks(raw: &str) -> Result<Vec<CheckId>> {
    let mut checks = Vec::new();
    for part in raw.split(',') {
        let code = part.trim();
        if code.is_empty() {
            continue;
        }
        let check = CheckId::from_code(code)
            .ok_or_else(|| ConformanceError::InvalidInput(format!("unknown check {code:?}")))?;
        if !checks.contains(&check) {
            checks.push(check);
        }
    }
    if checks.is_empty() {
        return Err(ConformanceError::InvalidInput(
            "no checks selected".to_string(),
        ));
    }
    Ok(checks)
}

pub async fn run_battery(
    snapshot: &DirectorySnapshotWire,
    checks: &[CheckId],
) -> Result<ConformanceReport> {
    if checks.is_empty() {
        return Err(ConformanceError::InvalidInput(
            "no checks selected: a battery that checks nothing cannot report conformance"
                .to_string(),
        ));
    }
    let wire = run_wire_battery(snapshot);
    let mut report = ConformanceReport::default();
    for id in ALL_CHECKS.into_iter().filter(|id| checks.contains(id)) {
        report.push(run_check(id, snapshot, &wire).await?);
    }
    Ok(report)
}

async fn run_check(
    id: CheckId,
    snapshot: &DirectorySnapshotWire,
    wire: &ConformanceReport,
) -> Result<CheckOutcome> {
    match id {
        CheckId::WireUserDeserializes
        | CheckId::WireGroupDeserializes
        | CheckId::WireMetaDeserializes
        | CheckId::WireExtensionRidesFlat
        | CheckId::WireMetaAutoDegrades => Ok(wire
            .outcomes
            .iter()
            .find(|outcome| outcome.id == id)
            .cloned()
            .unwrap_or_else(|| {
                CheckOutcome::skipped(id, "the wire battery produced no outcome for this check")
            })),
        CheckId::WireReservedKeyRejected => Ok(reserved_key_rejected()),
        CheckId::PublisherFloor => publisher_floor(snapshot).await,
        CheckId::PublisherGroupsOptional => publisher_groups_optional(snapshot).await,
        CheckId::ConsumerReadsUsers => consumer_reads_users(snapshot).await,
        CheckId::ConsumerReadsGroups => consumer_reads_groups(snapshot).await,
        CheckId::ConsumerExtensionSurvives => extension_survives_projection(snapshot).await,
        CheckId::ConsumerFilterFlipOrphans => filter_flip_orphan_deletes(snapshot).await,
        CheckId::ConsumerUsersOnlyNarrows => users_only_narrows_projection(snapshot).await,
        CheckId::ConsumerStagerTransaction => {
            stager_stages_in_the_projection_transaction(snapshot).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_snapshot::canonical_snapshot;

    #[test]
    fn every_check_code_round_trips_through_the_selection() {
        for id in ALL_CHECKS {
            assert_eq!(parse_checks(id.code()).unwrap(), vec![id]);
        }
        assert_eq!(ALL_CHECKS.len(), 14);
    }

    #[test]
    fn a_selection_keeps_each_check_once() {
        assert_eq!(
            parse_checks(" w2 ,p1,,w2").unwrap(),
            vec![CheckId::WireGroupDeserializes, CheckId::PublisherFloor]
        );
    }

    #[test]
    fn an_unknown_or_empty_selection_is_refused() {
        for raw in ["w1,w9", "", " , "] {
            assert!(
                matches!(parse_checks(raw), Err(ConformanceError::InvalidInput(_))),
                "{raw:?}"
            );
        }
    }

    #[tokio::test]
    async fn an_empty_selection_never_reports_conformance() {
        assert!(matches!(
            run_battery(&canonical_snapshot(), &[]).await,
            Err(ConformanceError::InvalidInput(_))
        ));
    }

    #[tokio::test]
    async fn the_offline_checks_run_in_battery_order_whatever_the_selection_order() {
        let selection = parse_checks("w6,w1,w5").unwrap();
        let report = run_battery(&canonical_snapshot(), &selection)
            .await
            .unwrap();
        let codes: Vec<_> = report.outcomes.iter().map(|o| o.id.code()).collect();
        assert_eq!(codes, ["w1", "w5", "w6"]);
        assert!(report.is_conformant(), "{report:#?}");
    }

    #[tokio::test]
    async fn a_wire_defect_is_a_failed_outcome_of_the_selected_check_only() {
        let mut snapshot = canonical_snapshot();
        snapshot.users[0].key = "identity/users/not-a-uuid".to_string();
        let selection = parse_checks("w1,w3").unwrap();
        let report = run_battery(&snapshot, &selection).await.unwrap();
        assert_eq!((report.failed(), report.passed()), (1, 1));
        assert_eq!(report.outcomes[0].id, CheckId::WireUserDeserializes);
        assert!(!report.is_conformant());
    }
}
