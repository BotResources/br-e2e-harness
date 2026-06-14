use br_core_auth::{AuthMethod, Passport};
use uuid::Uuid;

use crate::endpoint::Resolution;
use crate::outcome::{CheckId, CheckOutcome};
use crate::seed::SeededToken;

use super::CheckContext;

pub async fn run_valid_bearer(ctx: &CheckContext<'_>) -> CheckOutcome {
    let id = CheckId::ValidBearerResolvesToPassport;
    let expected = "human passport with pat(token_id) + claims.email == seeded";
    let token = match ctx.seeder.seed(ctx.namespace, "valid").await {
        Ok(token) => token,
        Err(e) => return CheckOutcome::fail(id, expected, "seeding failed", format!("{e}")),
    };
    let resolution = match ctx.endpoint.resolve_bearer(&token.raw).await {
        Ok(resolution) => resolution,
        Err(e) => {
            return CheckOutcome::fail(id, expected, "the endpoint call failed", format!("{e}"));
        }
    };
    if let Err(detail) = assert_resolves_to(&resolution, &token) {
        return CheckOutcome::fail(id, expected, resolution.label(), detail);
    }

    let again = match ctx.endpoint.resolve_bearer(&token.raw).await {
        Ok(resolution) => resolution,
        Err(e) => {
            return CheckOutcome::fail(
                id,
                expected,
                "the second endpoint call failed",
                format!("{e}"),
            );
        }
    };
    if let Err(detail) = assert_deterministic(&resolution, &again) {
        return CheckOutcome::fail(id, expected, again.label(), detail);
    }

    CheckOutcome::pass(id, expected, resolution.label())
}

fn assert_deterministic(
    first: &Resolution,
    second: &Resolution,
) -> std::result::Result<(), String> {
    let (first_user_id, first_auth_method) = human_identity(first)?;
    let (second_user_id, second_auth_method) = human_identity(second)?;
    if first_user_id != second_user_id {
        return Err(format!(
            "second resolution differs: user_id {first_user_id} != {second_user_id}; resolution must be deterministic"
        ));
    }
    if first_auth_method != second_auth_method {
        return Err(format!(
            "second resolution differs: auth_method {first_auth_method:?} != {second_auth_method:?}; resolution must be deterministic"
        ));
    }
    Ok(())
}

fn human_identity(resolution: &Resolution) -> std::result::Result<(Uuid, AuthMethod), String> {
    match resolution {
        Resolution::Resolved(Passport::Human {
            user_id,
            auth_method,
            ..
        }) => Ok((*user_id, auth_method.clone())),
        Resolution::Resolved(Passport::Service { .. }) => Err(
            "second resolution is a Service passport; resolution must be deterministic Human"
                .to_string(),
        ),
        Resolution::Anonymous => {
            Err("second resolution is anonymous; resolution must be deterministic".to_string())
        }
    }
}

pub async fn run_distinct_tokens(ctx: &CheckContext<'_>) -> CheckOutcome {
    let id = CheckId::DistinctTokensDistinctPassports;
    let expected = "each bearer resolves to its own passport, no cross-talk";
    let first = match ctx.seeder.seed(ctx.namespace, "distinct_a").await {
        Ok(token) => token,
        Err(e) => return CheckOutcome::fail(id, expected, "seeding first failed", format!("{e}")),
    };
    let second = match ctx.seeder.seed(ctx.namespace, "distinct_b").await {
        Ok(token) => token,
        Err(e) => return CheckOutcome::fail(id, expected, "seeding second failed", format!("{e}")),
    };

    let first_resolution = match ctx.endpoint.resolve_bearer(&first.raw).await {
        Ok(resolution) => resolution,
        Err(e) => return CheckOutcome::fail(id, expected, "first call failed", format!("{e}")),
    };
    let second_resolution = match ctx.endpoint.resolve_bearer(&second.raw).await {
        Ok(resolution) => resolution,
        Err(e) => return CheckOutcome::fail(id, expected, "second call failed", format!("{e}")),
    };

    if let Err(detail) = assert_resolves_to(&first_resolution, &first) {
        return CheckOutcome::fail(
            id,
            expected,
            first_resolution.label(),
            format!("first: {detail}"),
        );
    }
    if let Err(detail) = assert_resolves_to(&second_resolution, &second) {
        return CheckOutcome::fail(
            id,
            expected,
            second_resolution.label(),
            format!("second: {detail}"),
        );
    }

    let observed = format!(
        "{} / {}",
        first_resolution.label(),
        second_resolution.label()
    );
    if first.token_id == second.token_id {
        return CheckOutcome::fail(
            id,
            expected,
            observed,
            "the two seeded tokens collided on token_id — the harness must seed distinct entries",
        );
    }
    CheckOutcome::pass(id, expected, observed)
}

fn assert_resolves_to(
    resolution: &Resolution,
    token: &SeededToken,
) -> std::result::Result<(), String> {
    let passport = match resolution {
        Resolution::Resolved(passport) => passport,
        Resolution::Anonymous => {
            return Err(
                "resolved to anonymous; a seeded bearer must yield an X-Passport".to_string(),
            );
        }
    };
    let (user_id, auth_method, claims) = match passport {
        Passport::Human {
            user_id,
            auth_method,
            claims,
            ..
        } => (user_id, auth_method, claims),
        Passport::Service { .. } => {
            return Err(
                "resolved to a Service passport; a bearer must resolve to Human".to_string(),
            );
        }
    };

    match auth_method {
        AuthMethod::Pat { token_id } if *token_id == token.token_id => {}
        AuthMethod::Pat { token_id } => {
            return Err(format!(
                "auth_method.token_id {token_id} != seeded token_id {}",
                token.token_id
            ));
        }
        AuthMethod::Jwt => {
            return Err("auth_method is jwt; a bearer resolution must be pat".to_string());
        }
    }

    let email = match claims.get("email") {
        None => return Err("claims has no email; the seeded entry carries one".to_string()),
        Some(v) => v
            .as_str()
            .ok_or_else(|| format!("claims.email is not a string: {v}"))?,
    };
    if email != token.email {
        return Err(format!(
            "claims.email {email:?} != seeded email {:?}",
            token.email
        ));
    }

    if *user_id == Uuid::nil() {
        return Err("user_id is the nil UUID; it must be a present, valid identifier".to_string());
    }

    Ok(())
}
