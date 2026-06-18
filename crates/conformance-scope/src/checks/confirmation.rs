use br_test_harness::wait_until;

use crate::acceptor::{accept, reject};
use crate::outcome::{CheckId, CheckOutcome};
use crate::scenario::AcceptorBehavior;

use super::{CheckContext, QUIET_WINDOW, await_first_correlation, readyz_status};

pub async fn readiness_gated(ctx: &CheckContext<'_>) -> CheckOutcome {
    let id = CheckId::ReadinessGated;
    let expected = "/readyz is 503 before acceptance and 200 after";

    let Some(correlation_id) = await_first_correlation(ctx).await else {
        return CheckOutcome::fail(
            id,
            expected,
            "no declare captured within the timeout",
            "the subject did not publish a declare command",
        );
    };
    if !ctx.readyz.is_not_ready().await {
        return CheckOutcome::fail(
            id,
            expected,
            "503 before acceptance",
            format!(
                "/readyz was not 503 before acceptance: {}",
                readyz_status(ctx).await
            ),
        );
    }
    if let Err(detail) = accept(ctx.fabric, ctx.service_key, correlation_id).await {
        return CheckOutcome::fail(id, expected, "acceptance published", detail.to_string());
    }
    if wait_until(ctx.timeout, || async { ctx.readyz.is_ready().await }).await {
        CheckOutcome::pass(id, expected, "503 before, 200 after acceptance")
    } else {
        CheckOutcome::fail(
            id,
            expected,
            "200 after acceptance",
            format!(
                "/readyz did not reach 200 after acceptance: {}",
                readyz_status(ctx).await
            ),
        )
    }
}

pub async fn republishes_same_correlation_id(ctx: &CheckContext<'_>) -> CheckOutcome {
    let id = CheckId::RepublishesSameCorrelationId;
    let expected = "the subject re-publishes the SAME correlation_id past its wait timeout";

    if !wait_until(ctx.timeout, || async { ctx.capture.count() >= 2 }).await {
        return CheckOutcome::fail(
            id,
            expected,
            "at least two declares with one correlation_id",
            format!(
                "saw {} declare(s); the subject must re-publish past its wait timeout",
                ctx.capture.count()
            ),
        );
    }
    let ids = ctx.capture.correlation_ids();
    let first = ids[0];
    if !ids.iter().all(|cid| *cid == first) {
        return CheckOutcome::fail(
            id,
            expected,
            "every re-publish carries one correlation_id",
            format!("correlation_ids diverged: {ids:?}"),
        );
    }
    if let Err(detail) = accept(ctx.fabric, ctx.service_key, first).await {
        return CheckOutcome::fail(id, expected, "acceptance published", detail.to_string());
    }
    if wait_until(ctx.timeout, || async { ctx.readyz.is_ready().await }).await {
        CheckOutcome::pass(
            id,
            expected,
            "same correlation_id re-published, then accepted",
        )
    } else {
        CheckOutcome::fail(
            id,
            expected,
            "200 after accepting the re-published id",
            format!("/readyz did not reach 200: {}", readyz_status(ctx).await),
        )
    }
}

pub async fn rejection_stops_readiness(ctx: &CheckContext<'_>) -> CheckOutcome {
    let id = CheckId::RejectionStopsReadiness;
    let expected = "a rejection keeps /readyz at 503, surfaces its reason, and stops re-publishing";

    let Some(correlation_id) = await_first_correlation(ctx).await else {
        return CheckOutcome::fail(
            id,
            expected,
            "a declare to reject",
            "the subject did not publish a declare command",
        );
    };
    let reason = match ctx.behavior {
        AcceptorBehavior::Reject(reason) => reason.clone(),
        AcceptorBehavior::Accept => {
            return CheckOutcome::skipped(
                id,
                "scenario s4 requires --reject; acceptor is in accept mode",
            );
        }
    };
    let expected_body = format!("scope declaration rejected: {reason}");
    if let Err(detail) = reject(ctx.fabric, ctx.service_key, reason, correlation_id).await {
        return CheckOutcome::fail(id, expected, "rejection published", detail.to_string());
    }
    let surfaced = wait_until(ctx.timeout, || async {
        ctx.readyz.body().await.as_deref() == Some(expected_body.as_str())
    })
    .await;
    if !surfaced {
        return CheckOutcome::fail(
            id,
            &expected_body,
            ctx.readyz.body().await.unwrap_or_default(),
            "the subject did not surface the rejection reason in its /readyz body",
        );
    }
    let count_at_reject = ctx.capture.count();
    let still_publishing = wait_until(QUIET_WINDOW, || async {
        ctx.capture.count() > count_at_reject
    })
    .await;
    if still_publishing {
        return CheckOutcome::fail(
            id,
            expected,
            "re-publishing continued after a processed rejection",
            "the subject kept re-publishing after the rejection it had processed",
        );
    }
    if ctx.readyz.is_not_ready().await {
        CheckOutcome::pass(id, expected, "503 with reason, re-publishing stopped")
    } else {
        CheckOutcome::fail(
            id,
            "/readyz stays 503 after rejection",
            readyz_status(ctx).await,
            "/readyz left 503 after a rejection",
        )
    }
}

pub async fn duplicate_confirmations_tolerated(ctx: &CheckContext<'_>) -> CheckOutcome {
    let id = CheckId::DuplicateConfirmationsTolerated;
    let expected =
        "duplicate acceptances are tolerated: the subject reaches ready once and stays alive";

    let Some(correlation_id) = await_first_correlation(ctx).await else {
        return CheckOutcome::fail(
            id,
            expected,
            "a declare to confirm",
            "the subject did not publish a declare command",
        );
    };
    if let Err(detail) = accept(ctx.fabric, ctx.service_key, correlation_id).await {
        return CheckOutcome::fail(
            id,
            expected,
            "first acceptance published",
            detail.to_string(),
        );
    }
    if let Err(detail) = accept(ctx.fabric, ctx.service_key, correlation_id).await {
        return CheckOutcome::fail(
            id,
            expected,
            "second acceptance published",
            detail.to_string(),
        );
    }
    if wait_until(ctx.timeout, || async { ctx.readyz.is_ready().await }).await {
        CheckOutcome::pass(id, expected, "ready and tolerant of a duplicate acceptance")
    } else {
        CheckOutcome::fail(
            id,
            expected,
            "200 despite a duplicate acceptance",
            format!("/readyz did not reach 200: {}", readyz_status(ctx).await),
        )
    }
}
