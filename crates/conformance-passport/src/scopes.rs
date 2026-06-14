use br_core_auth::{Passport, PassportBuilder, PassportHeader, SCOPES_CLAIM_KEY, ScopeKey};

use crate::outcome::CheckOutcome;

pub fn g4_scopes_claim_round_trips_through_the_header() -> CheckOutcome {
    let id = crate::outcome::CheckId::ScopesClaimRoundTrip;
    let expected = "a Passport carrying a `scopes` claim survives the X-Passport base64 round-trip and the typed scopes()/has_scope() API";
    match assert_round_trip() {
        Ok(observed) => CheckOutcome::pass(id, expected, observed),
        Err(detail) => CheckOutcome::fail(id, expected, "round-trip diverged", detail),
    }
}

fn assert_round_trip() -> std::result::Result<String, String> {
    let granted = ScopeKey::new("notifier:read").map_err(|e| format!("notifier:read: {e}"))?;
    let also = ScopeKey::new("notifier:write").map_err(|e| format!("notifier:write: {e}"))?;
    let ungranted = ScopeKey::new("billing:manage").map_err(|e| format!("billing:manage: {e}"))?;

    let passport = PassportBuilder::new()
        .claim(
            SCOPES_CLAIM_KEY,
            vec![granted.as_str().to_string(), also.as_str().to_string()],
        )
        .build();

    let header = passport.to_header();
    let decoded = Passport::from_header(&header).map_err(|e| format!("from_header: {e}"))?;

    if decoded != passport {
        return Err("the decoded Passport is not equal to the forged one".to_string());
    }

    let scopes = decoded.scopes();
    if scopes != vec![granted.clone(), also.clone()] {
        return Err(format!(
            "scopes() = {:?}, expected the two granted typed keys",
            scopes.iter().map(ScopeKey::as_str).collect::<Vec<_>>()
        ));
    }
    if !decoded.has_scope(&granted) {
        return Err(format!(
            "has_scope({}) was false for a granted scope",
            granted.as_str()
        ));
    }
    if decoded.has_scope(&ungranted) {
        return Err(format!(
            "has_scope({}) was true for an ungranted scope",
            ungranted.as_str()
        ));
    }

    Ok(format!(
        "scopes()=[{}], has_scope granted/ungranted correct",
        scopes
            .iter()
            .map(ScopeKey::as_str)
            .collect::<Vec<_>>()
            .join(",")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn g4_scopes_claim_round_trips() {
        let outcome = g4_scopes_claim_round_trips_through_the_header();
        assert!(outcome.is_pass(), "{outcome:?}");
    }

    #[test]
    fn an_empty_scopes_claim_yields_no_typed_scopes() {
        let passport = PassportBuilder::new().build();
        let decoded = Passport::from_header(&passport.to_header()).unwrap();
        assert!(decoded.scopes().is_empty());
        assert!(!decoded.has_scope(&ScopeKey::new("notifier:read").unwrap()));
    }

    #[test]
    fn a_malformed_scope_entry_is_skipped_keeping_valid_ones() {
        let passport = PassportBuilder::new()
            .claim(
                SCOPES_CLAIM_KEY,
                vec![
                    "notifier:read".to_string(),
                    "not a valid scope key!!".to_string(),
                ],
            )
            .build();
        let decoded = Passport::from_header(&passport.to_header()).unwrap();
        assert_eq!(
            decoded.scopes(),
            vec![ScopeKey::new("notifier:read").unwrap()]
        );
    }
}
