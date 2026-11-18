use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use br_test_harness::FixedStream;
use tokio::sync::oneshot;

use super::*;
use crate::checks::projection::{MemorySink, assert_bootstrap_under_a_live_watch};
use crate::harness::namespaced_key;

async fn until(what: &str, mut done: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(20), async {
        while !done() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting until {what}"));
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a real nats-server"]
async fn dropping_the_check_future_stops_the_check_body() {
    let probe = Arc::new(());
    let started = Arc::new(AtomicBool::new(false));
    let outer = tokio::spawn({
        let held = Arc::clone(&probe);
        let started = Arc::clone(&started);
        async move {
            on_harness(
                &Broker::Spawn,
                Setup::FixedStreams,
                Duration::from_secs(3600),
                |_opened| async move {
                    let _held = held;
                    started.store(true, Ordering::SeqCst);
                    std::future::pending::<()>().await;
                },
            )
            .await
        }
    });

    until("the check body started", || started.load(Ordering::SeqCst)).await;
    outer.abort();
    let _ = outer.await;
    until("the check body stopped", || Arc::strong_count(&probe) == 1).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a real nats-server"]
async fn a_timed_out_check_leaves_no_watch_task_behind() {
    let sink = MemorySink::default();
    let stall = tokio::spawn({
        let sink = sink.clone();
        async move {
            loop {
                if sink
                    .snapshot()
                    .await
                    .keys()
                    .any(|k| k.as_str().ends_with("canary"))
                {
                    return sink.stall_recording().await;
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        }
    });
    let verdict = on_harness(
        &Broker::Spawn,
        Setup::PublishedLanguage,
        Duration::from_secs(5),
        |opened| {
            let sink = sink.clone();
            async move {
                assert_bootstrap_under_a_live_watch(&opened.harness, &sink)
                    .await
                    .expect("the check runs");
            }
        },
    )
    .await;

    let observed = verdict.expect_err("a check held up mid-flight times out");
    assert!(observed.contains("did not finish within"), "{observed}");
    let _stalled = stall.await.expect("the stall task ran");
    until("the watch task stopped", || sink.holders() == 1).await;
}

async fn provisioned_server() -> FabricTestNats {
    FabricTestNats::start()
        .await
        .with_published_language()
        .await
}

async fn durable_exists(observer: &Arc<FabricTestNats>, durable: &str) -> bool {
    let observer = Arc::clone(observer);
    let durable = durable.to_string();
    tokio::spawn(async move { observer.consumer_pending(FixedStream::Evt, &durable).await })
        .await
        .is_ok()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a real nats-server"]
async fn attach_mode_deletes_the_widened_durable_when_the_check_body_times_out() {
    let server = provisioned_server().await;
    let observer = Arc::new(FabricTestNats::attach_without_provisioning(&server.url()).await);
    let (written, durable) = oneshot::channel::<String>();
    let run = tokio::spawn({
        let broker = Broker::Attach(server.url());
        async move {
            on_harness(
                &broker,
                Setup::WidenedDurable,
                Duration::from_secs(3),
                |opened| async move {
                    let name = opened.widened.as_ref().expect("widened").durable.clone();
                    let _ = written.send(name);
                    std::future::pending::<()>().await;
                },
            )
            .await
        }
    });

    let durable = durable.await.expect("the body reported its durable");
    assert!(
        durable_exists(&observer, &durable).await,
        "the widened durable must exist while the body runs, or the test proves nothing"
    );
    let verdict = run.await.expect("the run finished");

    let observed = verdict.expect_err("a body that never finishes fails");
    assert!(observed.contains("did not finish within"), "{observed}");
    assert!(
        !durable_exists(&observer, &durable).await,
        "the widened durable {durable} must not outlive a timed-out check"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a real nats-server"]
async fn attach_mode_retracts_the_run_keys_when_the_check_body_times_out() {
    let server = provisioned_server().await;
    let observer = FabricTestNats::attach_without_provisioning(&server.url()).await;
    let (written, key) = oneshot::channel();
    let run = tokio::spawn({
        let broker = Broker::Attach(server.url());
        async move {
            on_harness(
                &broker,
                Setup::PublishedLanguage,
                Duration::from_secs(3),
                |opened| async move {
                    let key = namespaced_key(&opened.harness, "users/canary");
                    opened.harness.pl_put_raw(&key, b"{}").await;
                    let _ = written.send(key);
                    std::future::pending::<()>().await;
                },
            )
            .await
        }
    });

    let key = key.await.expect("the body reported its key");
    assert!(
        observer.pl_get_raw(&key).await.is_some(),
        "the key must be live while the body runs, or the test proves nothing"
    );
    let verdict = run.await.expect("the run finished");

    let observed = verdict.expect_err("a body that never finishes fails");
    assert!(observed.contains("did not finish within"), "{observed}");
    assert!(
        observer.pl_get_raw(&key).await.is_none(),
        "the run key {} must not outlive a timed-out check",
        key.as_str()
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a real nats-server"]
async fn a_check_body_that_outlives_its_timeout_fails_and_is_named() {
    let verdict = on_harness(
        &Broker::Spawn,
        Setup::FixedStreams,
        Duration::from_millis(50),
        |_opened| async move {
            std::future::pending::<()>().await;
        },
    )
    .await;

    assert_eq!(verdict, Err("did not finish within 50ms".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a real nats-server"]
async fn the_timeout_bounds_the_check_body_not_the_server_start() {
    let verdict = on_bare(Duration::from_millis(1), |bare| async move { bare }).await;

    assert_eq!(verdict, Ok(()));
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a real nats-server"]
async fn a_bare_check_body_that_outlives_its_timeout_fails_and_is_named() {
    let verdict = on_bare(Duration::from_millis(50), |bare| async move {
        std::future::pending::<()>().await;
        bare
    })
    .await;

    assert_eq!(verdict, Err("did not finish within 50ms".to_string()));
}
