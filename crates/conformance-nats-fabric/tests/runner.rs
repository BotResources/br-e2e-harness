use std::time::Duration;

use br_core_directory::PublishedUser;
use br_test_harness::{BareFabricNats, FabricTestNats};
use br_util_nats_fabric::{KvPrefix, PublishedLanguageReader};
use conformance_nats_fabric::checks::published_language::assert_reconcile_drift_converges;
use conformance_nats_fabric::{
    AttachTarget, CheckId, CheckStatus, ConformanceError, ConformanceReport, attach_default,
    run_attach, run_spawn, spawn_default,
};

const TIMEOUT: Duration = Duration::from_secs(60);

fn statuses(report: &ConformanceReport) -> Vec<(&'static str, CheckStatus)> {
    report
        .outcomes
        .iter()
        .map(|o| (o.id.code(), o.status))
        .collect()
}

fn failures(report: &ConformanceReport) -> Vec<String> {
    report
        .outcomes
        .iter()
        .filter(|o| o.status == CheckStatus::Fail)
        .map(|o| format!("{}: {}", o.id, o.observed))
        .collect()
}

async fn run_keys(harness: &FabricTestNats) -> usize {
    let reader: PublishedLanguageReader<PublishedUser> =
        PublishedLanguageReader::open(&harness.fabric_owned())
            .await
            .expect("open the published-language reader");
    reader
        .keys(&KvPrefix::new("0").expect("a valid prefix"))
        .await
        .expect("list run-namespaced keys")
        .len()
}

#[tokio::test]
#[ignore = "requires a real nats-server and the prebuilt nats-fabric anchor binary"]
async fn spawn_mode_runs_every_check_against_a_throwaway_server() {
    let report = run_spawn(&spawn_default(), TIMEOUT)
        .await
        .expect("spawn run");
    assert!(report.is_conformant(), "{:?}", failures(&report));
    assert_eq!(report.passed(), CheckId::ALL.len());
    assert_eq!(report.skipped(), 0);
}

#[tokio::test]
#[ignore = "requires a real nats-server and the prebuilt nats-fabric anchor binary"]
async fn attach_mode_passes_on_a_provisioned_server_skips_the_throwaway_checks_and_leaves_nothing()
{
    let harness = FabricTestNats::start()
        .await
        .with_published_language()
        .await;
    assert_reconcile_drift_converges(&harness)
        .await
        .expect("the direct check leaves its namespaced keys behind");
    let left_by_the_direct_check = run_keys(&harness).await;
    assert!(
        left_by_the_direct_check > 0,
        "the probe must see leftovers, or the cleanup assertion proves nothing"
    );

    let target = AttachTarget {
        nats_url: harness.url(),
    };
    let report = run_attach(&target, &CheckId::ALL, TIMEOUT)
        .await
        .expect("attach run");

    assert!(report.is_conformant(), "{:?}", failures(&report));
    assert_eq!(report.skipped(), 2, "{:?}", statuses(&report));
    for outcome in &report.outcomes {
        let expected = if outcome.id.needs_throwaway_server() {
            CheckStatus::Skipped
        } else {
            CheckStatus::Pass
        };
        assert_eq!(outcome.status, expected, "{}", outcome.id);
    }
    assert_eq!(
        run_keys(&harness).await,
        left_by_the_direct_check,
        "an attach run must retract every published-language key it wrote"
    );
    harness.shutdown().await;
}

#[tokio::test]
#[ignore = "requires a real nats-server and the prebuilt nats-fabric anchor binary"]
async fn attach_mode_fails_loud_on_a_server_without_the_declared_topology_and_creates_nothing() {
    let bare = BareFabricNats::without_fixed_streams().await;
    let target = AttachTarget {
        nats_url: bare.url(),
    };
    let report = run_attach(&target, &attach_default(), TIMEOUT)
        .await
        .expect("attach run");

    assert!(!report.is_conformant());
    let failed: Vec<CheckId> = report
        .outcomes
        .iter()
        .filter(|o| o.status == CheckStatus::Fail)
        .map(|o| o.id)
        .collect();
    for id in [
        CheckId::WidenedDurableConverges,
        CheckId::DeadGrammarFailsLoud,
        CheckId::PublishedLanguageRetractOrphanDeletes,
        CheckId::PublishedLanguageAnchorPoisonFailsClosed,
    ] {
        assert!(
            failed.contains(&id),
            "{id} must fail: {:?}",
            statuses(&report)
        );
    }
    let widened = report
        .outcomes
        .iter()
        .find(|o| o.id == CheckId::WidenedDurableConverges)
        .expect("widened outcome");
    assert!(
        widened.observed.contains("must exist"),
        "{}",
        widened.observed
    );
    assert!(bare.command_stream_absent().await);
    assert!(bare.event_stream_absent().await);
    assert!(bare.published_language_absent().await);
    bare.shutdown().await;
}

#[tokio::test]
#[ignore = "requires a real nats-server"]
async fn attach_mode_to_an_unreachable_server_is_a_typed_error_not_a_report() {
    let bare = BareFabricNats::without_fixed_streams().await;
    let url = bare.url();
    bare.shutdown().await;
    let err = run_attach(
        &AttachTarget { nats_url: url },
        &[CheckId::DeadGrammarFailsLoud],
        TIMEOUT,
    )
    .await
    .expect_err("an unreachable server");
    assert!(
        matches!(err, ConformanceError::BrokerUnavailable(_)),
        "{err:?}"
    );
}

const WATCH_CHECKS: [CheckId; 5] = [
    CheckId::PublishedLanguageBootstrapThenWatch,
    CheckId::PublishedLanguagePrefixWatchDelivers,
    CheckId::PublishedLanguageReconcileConverges,
    CheckId::PublishedLanguageRetractOrphanDeletes,
    CheckId::PublishedLanguageDecodeFailsClosed,
];

#[tokio::test]
#[ignore = "requires a real nats-server"]
async fn a_check_that_outlives_its_timeout_fails_and_is_named() {
    let report = run_spawn(&WATCH_CHECKS, Duration::from_millis(1))
        .await
        .expect("spawn run");
    assert!(report.failed() >= 1, "{:?}", statuses(&report));
    for outcome in report
        .outcomes
        .iter()
        .filter(|o| o.status == CheckStatus::Fail)
    {
        assert!(
            outcome.observed.contains("did not finish within"),
            "{}: {}",
            outcome.id,
            outcome.observed
        );
    }
}

#[tokio::test]
#[ignore = "requires a real nats-server"]
async fn attach_mode_leaves_no_run_key_behind_when_checks_time_out() {
    let harness = FabricTestNats::start()
        .await
        .with_published_language()
        .await;
    assert_eq!(run_keys(&harness).await, 0);

    let target = AttachTarget {
        nats_url: harness.url(),
    };
    for timeout in [120, 250, 400] {
        let mut selection = WATCH_CHECKS.to_vec();
        selection.push(CheckId::WidenedDurableConverges);
        let report = run_attach(&target, &selection, Duration::from_millis(timeout))
            .await
            .expect("attach run");
        for outcome in report
            .outcomes
            .iter()
            .filter(|o| o.status == CheckStatus::Fail)
        {
            assert!(
                outcome.observed.contains("did not finish within"),
                "{}: {}",
                outcome.id,
                outcome.observed
            );
        }
        assert_eq!(
            run_keys(&harness).await,
            0,
            "a timed-out attach check must retract its keys (timeout {timeout} ms)"
        );
    }
    harness.shutdown().await;
}

#[tokio::test]
async fn an_empty_selection_is_an_error_never_a_green_report() {
    let err = run_spawn(&[], TIMEOUT).await.expect_err("nothing selected");
    assert!(matches!(err, ConformanceError::NoCheckToRun(_)), "{err:?}");

    let target = AttachTarget {
        nats_url: "nats://127.0.0.1:1".to_string(),
    };
    let spawn_only = [
        CheckId::MissingStreamFailsCoverageProbe,
        CheckId::MissingStreamFailsDurableBind,
    ];
    let err = run_attach(&target, &spawn_only, TIMEOUT)
        .await
        .expect_err("only spawn-only checks selected");
    assert!(matches!(err, ConformanceError::NoCheckToRun(_)), "{err:?}");
}

#[tokio::test]
async fn a_repeated_check_is_refused_never_counted_twice() {
    let twice = [CheckId::DeadGrammarFailsLoud, CheckId::DeadGrammarFailsLoud];
    let err = run_spawn(&twice, TIMEOUT)
        .await
        .expect_err("a repeated check");
    assert!(
        matches!(err, ConformanceError::DuplicateCheck(code) if code == "dead-grammar-fails-loud")
    );

    let target = AttachTarget {
        nats_url: "nats://127.0.0.1:1".to_string(),
    };
    let err = run_attach(&target, &twice, TIMEOUT)
        .await
        .expect_err("a repeated check");
    assert!(
        matches!(err, ConformanceError::DuplicateCheck(_)),
        "{err:?}"
    );
}
