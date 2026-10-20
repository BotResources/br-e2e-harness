mod attached;
mod execute;
mod redact;

use std::sync::Arc;
use std::time::Duration;

use br_test_harness::FabricTestNats;
use tokio::task::JoinError;

use crate::anchor::frozen_wire;
use crate::error::{ConformanceError, Result};
use crate::report::{CheckId, CheckOutcome, ConformanceReport, ensure_distinct};
use crate::wire::FrozenWire;
use execute::{Broker, CLEANUP_BOUND, did_not_finish, execute};
use redact::redact_in;
pub use redact::redact_nats_url;

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct AttachTarget {
    pub nats_url: String,
}

pub async fn run_spawn(checks: &[CheckId], timeout: Duration) -> Result<ConformanceReport> {
    ensure_distinct(checks)?;
    if checks.is_empty() {
        return Err(ConformanceError::NoCheckToRun("the selection is empty"));
    }
    if checks.iter().any(|id| id.needs_broker()) {
        ensure_nats_server_on_path().await?;
    }
    let wire = load_wire(checks).await?;
    Ok(run(checks, &Broker::Spawn, wire, timeout).await)
}

pub async fn run_attach(
    target: &AttachTarget,
    checks: &[CheckId],
    timeout: Duration,
) -> Result<ConformanceReport> {
    ensure_distinct(checks)?;
    let runnable: Vec<CheckId> = checks
        .iter()
        .copied()
        .filter(|id| !id.needs_throwaway_server())
        .collect();
    if runnable.is_empty() {
        return Err(ConformanceError::NoCheckToRun(
            "the selection is empty or holds only spawn-only checks, which attach mode skips",
        ));
    }
    if runnable.iter().any(|id| id.needs_broker()) {
        probe_attach(&target.nats_url, timeout).await?;
    }
    let wire = load_wire(&runnable).await?;
    let broker = Broker::Attach(target.nats_url.clone());
    Ok(run(checks, &broker, wire, timeout).await)
}

async fn ensure_nats_server_on_path() -> Result<()> {
    tokio::process::Command::new("nats-server")
        .arg("--version")
        .output()
        .await
        .map(|_| ())
        .map_err(|e| {
            ConformanceError::BrokerUnavailable(format!(
                "`nats-server` cannot be run from PATH ({e}); spawn mode starts a throwaway \
                 nats-server, so install it or attach to a running server with --nats"
            ))
        })
}

async fn probe_attach(urls: &str, timeout: Duration) -> Result<()> {
    let target = urls.to_string();
    let probe = tokio::spawn(async move {
        let harness = FabricTestNats::attach_without_provisioning(&target).await;
        harness.fixed_streams_present().await;
        harness.shutdown().await;
    });
    let abort = probe.abort_handle();
    match tokio::time::timeout(timeout, probe).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(ConformanceError::BrokerUnavailable(redact_in(
            &panic_message(e),
            urls,
        ))),
        Err(_) => {
            abort.abort();
            Err(ConformanceError::BrokerUnavailable(format!(
                "{} did not answer within {timeout:?}",
                redact_nats_url(urls)
            )))
        }
    }
}

async fn load_wire(checks: &[CheckId]) -> Result<Option<Arc<FrozenWire>>> {
    if checks.iter().any(|id| id.needs_anchor()) {
        Ok(Some(Arc::new(frozen_wire().await?)))
    } else {
        Ok(None)
    }
}

async fn run(
    checks: &[CheckId],
    broker: &Broker,
    wire: Option<Arc<FrozenWire>>,
    timeout: Duration,
) -> ConformanceReport {
    let mut report = ConformanceReport::default();
    for &id in checks {
        let mut outcome = if matches!(broker, Broker::Attach(_)) && id.needs_throwaway_server() {
            CheckOutcome::skipped(
                id,
                "needs a throwaway nats-server without the fixed streams: spawn mode only",
            )
        } else {
            run_one(id, broker.clone(), wire.clone(), timeout).await
        };
        if let Broker::Attach(url) = broker {
            outcome.observed = redact_in(&outcome.observed, url);
        }
        report.outcomes.push(outcome);
    }
    report
}

async fn run_one(
    id: CheckId,
    broker: Broker,
    wire: Option<Arc<FrozenWire>>,
    timeout: Duration,
) -> CheckOutcome {
    let task = tokio::spawn(execute(id, broker, wire, timeout));
    let abort = task.abort_handle();
    let last_resort = timeout.saturating_add(CLEANUP_BOUND.saturating_mul(3));
    match tokio::time::timeout(last_resort, task).await {
        Ok(Ok(Ok(()))) => CheckOutcome::pass(id),
        Ok(Ok(Err(observed))) => CheckOutcome::fail(id, observed),
        Ok(Err(join)) => CheckOutcome::fail(id, panic_message(join)),
        Err(_) => {
            abort.abort();
            CheckOutcome::fail(id, did_not_finish(last_resort))
        }
    }
}

pub(crate) fn panic_message(error: JoinError) -> String {
    if !error.is_panic() {
        return error.to_string();
    }
    let payload = error.into_panic();
    match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(payload) => match payload.downcast::<&'static str>() {
            Ok(message) => (*message).to_string(),
            Err(_) => "the check panicked with a non-string payload".to_string(),
        },
    }
}
