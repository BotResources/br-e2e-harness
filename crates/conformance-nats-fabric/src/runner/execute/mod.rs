use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use br_test_harness::{BareFabricNats, FabricTestNats, WidenedDurable};

use crate::abort_on_drop::AbortOnDrop;
use crate::checks::integration::{
    DEAD_COMMAND_SUBJECT, assert_anchor_subjects_match, assert_dead_grammar_fails_loud,
    assert_missing_stream_fails_loud, assert_missing_stream_fails_loud_on_bind,
    assert_no_fixed_stream_captured, assert_widened_durable_converges, widen,
};
use crate::checks::projection::{
    assert_bootstrap_then_watch_is_parallel_safe, assert_prefix_watch_delivery_gap,
};
use crate::checks::published_language::{
    assert_anchor_users_deserialize, assert_decode_fails_closed_naming_the_key,
    assert_poison_from_anchor_names_the_key, assert_reconcile_drift_converges,
    assert_retract_orphan_deletes,
};
use crate::report::CheckId;
use crate::runner::attached::{clean_attached, require_topology};
use crate::runner::panic_message;
use crate::wire::FrozenWire;

const WIDENED_DURABLE: &str = "widened_evt";

pub(super) const CLEANUP_BOUND: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub(super) enum Broker {
    Spawn,
    Attach(String),
}

#[derive(Clone, Copy)]
pub(super) enum Setup {
    FixedStreams,
    WidenedDurable,
    PublishedLanguage,
}

pub(super) struct Opened {
    pub(super) harness: FabricTestNats,
    pub(super) widened: Option<WidenedDurable>,
}

pub(super) async fn execute(
    id: CheckId,
    broker: Broker,
    wire: Option<Arc<FrozenWire>>,
    timeout: Duration,
) -> Result<(), String> {
    match id {
        CheckId::AnchorSubjectsMatchLib => {
            assert_anchor_subjects_match(anchor(wire.as_deref()));
            Ok(())
        }
        CheckId::AnchorUsersDeserializeThroughLib => {
            assert_anchor_users_deserialize(anchor(wire.as_deref()));
            Ok(())
        }
        CheckId::MissingStreamFailsCoverageProbe => {
            on_bare(timeout, |bare| async move {
                assert_missing_stream_fails_loud(&bare).await;
                bare
            })
            .await
        }
        CheckId::MissingStreamFailsDurableBind => {
            on_bare(timeout, |bare| async move {
                assert_missing_stream_fails_loud_on_bind(&bare).await;
                bare
            })
            .await
        }
        CheckId::WidenedDurableConverges => {
            on_harness(
                &broker,
                Setup::WidenedDurable,
                timeout,
                |opened| async move {
                    let marker = opened
                        .widened
                        .as_ref()
                        .expect("the widened durable was set up");
                    assert_widened_durable_converges(&opened.harness, marker).await;
                },
            )
            .await
        }
        CheckId::DeadGrammarFailsLoud => {
            on_harness(&broker, Setup::FixedStreams, timeout, |opened| async move {
                assert_dead_grammar_fails_loud(&opened.harness, DEAD_COMMAND_SUBJECT).await;
                assert_no_fixed_stream_captured(&opened.harness, DEAD_COMMAND_SUBJECT)
                    .await
                    .expect("no fixed stream captured the dead grammar");
            })
            .await
        }
        CheckId::PublishedLanguageRetractOrphanDeletes => {
            on_harness(
                &broker,
                Setup::PublishedLanguage,
                timeout,
                |opened| async move {
                    assert_retract_orphan_deletes(&opened.harness)
                        .await
                        .expect("retract orphan-deletes");
                },
            )
            .await
        }
        CheckId::PublishedLanguageReconcileConverges => {
            on_harness(
                &broker,
                Setup::PublishedLanguage,
                timeout,
                |opened| async move {
                    assert_reconcile_drift_converges(&opened.harness)
                        .await
                        .expect("reconcile converges");
                },
            )
            .await
        }
        CheckId::PublishedLanguageBootstrapThenWatch => {
            on_harness(
                &broker,
                Setup::PublishedLanguage,
                timeout,
                |opened| async move {
                    assert_bootstrap_then_watch_is_parallel_safe(&opened.harness)
                        .await
                        .expect("bootstrap then watch is parallel-safe");
                },
            )
            .await
        }
        CheckId::PublishedLanguagePrefixWatchDelivers => {
            on_harness(
                &broker,
                Setup::PublishedLanguage,
                timeout,
                |opened| async move {
                    let delivered = assert_prefix_watch_delivery_gap(&opened.harness)
                        .await
                        .expect("the gap probe runs");
                    assert!(
                        delivered,
                        "prefix-watch must deliver a live slash-keyed put within the deadline"
                    );
                },
            )
            .await
        }
        CheckId::PublishedLanguageDecodeFailsClosed => {
            on_harness(
                &broker,
                Setup::PublishedLanguage,
                timeout,
                |opened| async move {
                    assert_decode_fails_closed_naming_the_key(&opened.harness)
                        .await
                        .expect("decode fails closed naming the key");
                },
            )
            .await
        }
        CheckId::PublishedLanguageAnchorPoisonFailsClosed => {
            let wire = Arc::clone(wire.as_ref().expect("the anchor wire is loaded"));
            on_harness(
                &broker,
                Setup::PublishedLanguage,
                timeout,
                |opened| async move {
                    assert_poison_from_anchor_names_the_key(
                        &opened.harness,
                        &wire.poison_user_key,
                        &wire.poison_user_value,
                    )
                    .await
                    .expect("anchor poison fails closed naming the key");
                },
            )
            .await
        }
    }
}

fn anchor(wire: Option<&FrozenWire>) -> &FrozenWire {
    wire.expect("the anchor wire is loaded")
}

pub(super) fn did_not_finish(timeout: Duration) -> String {
    format!("did not finish within {timeout:?}")
}

async fn within<T>(timeout: Duration, work: impl Future<Output = T>) -> Result<T, String> {
    tokio::time::timeout(timeout, work)
        .await
        .map_err(|_| did_not_finish(timeout))
}

async fn on_bare<F, Fut>(timeout: Duration, check: F) -> Result<(), String>
where
    F: FnOnce(BareFabricNats) -> Fut,
    Fut: Future<Output = BareFabricNats>,
{
    let bare = BareFabricNats::without_fixed_streams().await;
    let bare = within(timeout, check(bare)).await?;
    bare.shutdown().await;
    Ok(())
}

async fn on_harness<F, Fut>(
    broker: &Broker,
    setup: Setup,
    timeout: Duration,
    check: F,
) -> Result<(), String>
where
    F: FnOnce(Arc<Opened>) -> Fut,
    Fut: Future<Output = ()> + Send + 'static,
{
    let opened = Arc::new(open(broker, setup).await?);
    let mut body = tokio::spawn(check(Arc::clone(&opened)));
    let _abort_with_this_future = AbortOnDrop::new(body.abort_handle());
    let verdict = match tokio::time::timeout(timeout, &mut body).await {
        Ok(joined) => joined.map_err(panic_message),
        Err(_) => {
            body.abort();
            let _ = body.await;
            Err(did_not_finish(timeout))
        }
    };
    let cleaned = match broker {
        Broker::Attach(_) => bounded_cleanup(clean_attached(&opened, setup)).await,
        Broker::Spawn => Ok(()),
    };
    if let Ok(opened) = Arc::try_unwrap(opened) {
        let _ = tokio::time::timeout(CLEANUP_BOUND, opened.harness.shutdown()).await;
    }
    match (verdict, cleaned) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(observed), Ok(())) => Err(observed),
        (Ok(()), Err(left_behind)) => Err(left_behind),
        (Err(observed), Err(left_behind)) => Err(format!("{observed}\n{left_behind}")),
    }
}

async fn bounded_cleanup(cleanup: impl Future<Output = Result<(), String>>) -> Result<(), String> {
    tokio::time::timeout(CLEANUP_BOUND, cleanup)
        .await
        .unwrap_or_else(|_| {
            Err(format!(
                "the cleanup of what this run wrote did not finish within {CLEANUP_BOUND:?}: \
                 run-namespaced keys or a widened durable may remain on the server"
            ))
        })
}

async fn open(broker: &Broker, setup: Setup) -> Result<Opened, String> {
    let harness = match broker {
        Broker::Spawn => {
            let harness = FabricTestNats::start().await;
            match setup {
                Setup::PublishedLanguage => harness.with_published_language().await,
                Setup::FixedStreams | Setup::WidenedDurable => harness,
            }
        }
        Broker::Attach(url) => {
            let harness = FabricTestNats::attach_without_provisioning(url).await;
            require_topology(&harness, setup).await?;
            harness
        }
    };
    match setup {
        Setup::WidenedDurable => {
            let (harness, marker) = widen(harness, WIDENED_DURABLE).await;
            Ok(Opened {
                harness,
                widened: Some(marker),
            })
        }
        Setup::FixedStreams | Setup::PublishedLanguage => Ok(Opened {
            harness,
            widened: None,
        }),
    }
}

#[cfg(test)]
mod tests;
