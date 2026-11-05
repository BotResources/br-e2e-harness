use br_test_harness::wait_until;

use crate::outcome::{CheckId, CheckOutcome};

use super::{
    CheckContext, QUIET_WINDOW, await_first_declare, capture_failed, no_declare, readyz_seen,
    wire_excerpt,
};

pub async fn declare_well_formed(ctx: &CheckContext<'_>) -> CheckOutcome {
    let id = CheckId::DeclareWellFormed;
    let expected = "a declare is published on boot and deserializes into \
                    IntegrationCommand<DeclareServiceScopes>";

    let declare = match await_first_declare(ctx).await {
        Ok(declare) => declare,
        Err(wait) => {
            return no_declare(
                id,
                expected,
                wait,
                "no declare captured within the timeout",
                "the subject did not publish a declare command on boot",
            );
        }
    };
    match declare.decode() {
        Ok(_) => CheckOutcome::pass(id, expected, "declare captured and deserialized"),
        Err(e) => CheckOutcome::fail(
            id,
            expected,
            "the declare did not deserialize",
            format!("{e}\nwire: {}", wire_excerpt(&declare.raw)),
        ),
    }
}

pub async fn declaration_content(ctx: &CheckContext<'_>) -> CheckOutcome {
    let id = CheckId::DeclarationContent;
    let expected = format!(
        "service_key={:?}, scopes=[{}]",
        ctx.expected.service_key,
        ctx.expected.scope_keys_csv()
    );

    let declare = match await_first_declare(ctx).await {
        Ok(declare) => declare,
        Err(wait) => {
            return no_declare(
                id,
                &expected,
                wait,
                "no declare captured within the timeout",
                "the subject did not publish a declare command on boot",
            );
        }
    };
    let command = match declare.decode() {
        Ok(command) => command,
        Err(e) => {
            return CheckOutcome::fail(
                id,
                expected,
                "the declare did not deserialize",
                format!("{e}\nwire: {}", wire_excerpt(&declare.raw)),
            );
        }
    };
    let declared = match command.payload.validate() {
        Ok(declared) => declared,
        Err(e) => {
            return CheckOutcome::fail(
                id,
                expected,
                "the declared scopes did not validate",
                format!("{e}\nwire: {}", wire_excerpt(&declare.raw)),
            );
        }
    };
    match ctx.expected.assert_matches(&declared) {
        Ok(()) => CheckOutcome::pass(id, expected, "declared content matches"),
        Err(diff) => {
            let observed = format!(
                "service_key={:?}, scopes=[{}]",
                declared.manifest().key.as_str(),
                declared
                    .scopes()
                    .iter()
                    .map(|s| s.key.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            );
            CheckOutcome::fail(id, expected, observed, diff)
        }
    }
}

pub async fn disabled_mode_ready_without_declare(ctx: &CheckContext<'_>) -> CheckOutcome {
    let id = CheckId::DisabledModeReadyWithoutDeclare;
    let expected =
        "with scope declaration disabled, /readyz is 200 immediately and no declare is published";

    if !wait_until(ctx.wait(), || async { ctx.readyz.is_ready().await }).await {
        return CheckOutcome::fail(
            id,
            expected,
            readyz_seen(ctx, "in disabled mode").await,
            "/readyz did not reach 200",
        );
    }
    wait_until(QUIET_WINDOW, || async {
        ctx.capture
            .try_count()
            .map(|count| count > 0)
            .unwrap_or(true)
    })
    .await;
    match ctx.capture.try_count() {
        Ok(0) => CheckOutcome::pass(id, expected, "ready immediately, no declare published"),
        Ok(count) => CheckOutcome::fail(
            id,
            expected,
            format!("{count} declare(s) published"),
            "disabled mode must publish no declare command",
        ),
        Err(cause) => capture_failed(id, expected, &cause),
    }
}
