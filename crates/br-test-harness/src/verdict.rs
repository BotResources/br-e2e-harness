use serde_json::Value;

pub fn is_ack(response: &Value) -> bool {
    !has_top_level_errors(response) && mutation_error_code(response).is_none()
}

pub fn expect_ack(response: &Value, what: &str) {
    assert!(
        is_ack(response),
        "{what} expected to ack, but was rejected: {response}"
    );
}

pub fn expect_rejected(response: &Value) -> String {
    mutation_error_code(response)
        .unwrap_or_else(|| panic!("expected a rejection with a stable code, got ack: {response}"))
}

pub fn expect_code_shaped(response: &Value, what: &str) -> String {
    let code = expect_rejected(response);
    assert!(
        is_code_shaped(&code),
        "{what}: error code must match ^[A-Z][A-Z0-9_]+$ (stable code, not prose), \
         got '{code}' from response: {response}"
    );
    code
}

pub fn is_code_shaped(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) if first.is_ascii_uppercase() => {}
        _ => return false,
    }
    let mut rest = chars.peekable();
    rest.peek().is_some() && rest.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

pub fn mutation_error_code(response: &Value) -> Option<String> {
    if let Some(errors) = response.get("errors").and_then(Value::as_array)
        && let Some(first) = errors.first()
    {
        if let Some(code) = first
            .get("extensions")
            .and_then(|e| e.get("code"))
            .and_then(Value::as_str)
        {
            return Some(code.to_string());
        }
        if let Some(msg) = first.get("message").and_then(Value::as_str) {
            return Some(msg.to_string());
        }
    }
    response.get("data").and_then(find_error_code)
}

fn has_top_level_errors(response: &Value) -> bool {
    response
        .get("errors")
        .and_then(Value::as_array)
        .map(|e| !e.is_empty())
        .unwrap_or(false)
}

fn find_error_code(value: &Value) -> Option<String> {
    fn walk(value: &Value, under_affordances: bool) -> Option<String> {
        match value {
            Value::Object(map) => {
                if !under_affordances {
                    for key in ["code", "errorCode", "reasonCode"] {
                        if let Some(code) = map.get(key).and_then(Value::as_str) {
                            return Some(code.to_string());
                        }
                    }
                }
                for (k, v) in map {
                    let affordance_ctx = under_affordances || k == "affordances";
                    if let Some(found) = walk(v, affordance_ctx) {
                        return Some(found);
                    }
                }
                None
            }
            Value::Array(items) => items.iter().find_map(|v| walk(v, under_affordances)),
            _ => None,
        }
    }
    walk(value, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ack_when_no_errors_and_no_embedded_code() {
        let response = json!({ "data": { "createThing": { "id": "abc" } } });
        assert!(is_ack(&response));
        assert!(mutation_error_code(&response).is_none());
        expect_ack(&response, "createThing");
    }

    #[test]
    fn top_level_graphql_error_extensions_code_is_the_rejection() {
        let response = json!({
            "data": null,
            "errors": [{ "message": "boom", "extensions": { "code": "FORBIDDEN" } }],
        });
        assert!(!is_ack(&response));
        assert_eq!(mutation_error_code(&response).as_deref(), Some("FORBIDDEN"));
        assert_eq!(expect_rejected(&response), "FORBIDDEN");
    }

    #[test]
    fn top_level_error_falls_back_to_message_when_no_extensions_code() {
        let response = json!({ "errors": [{ "message": "SOME_PROSE" }] });
        assert_eq!(
            mutation_error_code(&response).as_deref(),
            Some("SOME_PROSE")
        );
    }

    #[test]
    fn payload_union_code_under_data_is_the_rejection() {
        let response = json!({ "data": { "signCharter": { "code": "ALREADY_SIGNED" } } });
        assert!(!is_ack(&response));
        assert_eq!(expect_rejected(&response), "ALREADY_SIGNED");
    }

    #[test]
    fn affordance_reason_code_is_never_mistaken_for_a_mutation_error() {
        let response = json!({
            "data": {
                "createThing": {
                    "id": "abc",
                    "affordances": [
                        { "action": "sign", "allowed": false, "reasonCode": "NOT_YET_OPEN" }
                    ]
                }
            }
        });
        assert!(is_ack(&response));
        assert!(mutation_error_code(&response).is_none());
        expect_ack(&response, "createThing");
    }

    #[test]
    fn mutation_error_wins_even_alongside_an_affordance_reason_code() {
        let response = json!({
            "data": {
                "signCharter": {
                    "code": "ALREADY_SIGNED",
                    "affordances": [
                        { "action": "sign", "allowed": false, "reasonCode": "NOT_YET_OPEN" }
                    ]
                }
            }
        });
        assert_eq!(expect_rejected(&response), "ALREADY_SIGNED");
    }

    #[test]
    fn nested_affordances_anywhere_under_data_are_still_skipped() {
        let response = json!({
            "data": {
                "thing": {
                    "child": {
                        "affordances": [{ "reasonCode": "BLOCKED_DEEP" }]
                    }
                }
            }
        });
        assert!(is_ack(&response));
    }

    #[test]
    fn code_shape_accepts_stable_codes() {
        assert!(is_code_shaped("S3_FOO"));
        assert!(is_code_shaped("ALREADY_SIGNED"));
        assert!(is_code_shaped("FORBIDDEN"));
        assert!(is_code_shaped("A1"));
    }

    #[test]
    fn code_shape_rejects_prose_and_empty() {
        assert!(!is_code_shaped("lowercase"));
        assert!(!is_code_shaped("Bad Code"));
        assert!(!is_code_shaped(""));
        assert!(!is_code_shaped("A"));
        assert!(!is_code_shaped("1FOO"));
        assert!(!is_code_shaped("_FOO"));
    }

    #[test]
    fn expect_code_shaped_returns_the_pinned_code() {
        let response = json!({ "data": { "signCharter": { "code": "ALREADY_SIGNED" } } });
        assert_eq!(expect_code_shaped(&response, "sign"), "ALREADY_SIGNED");
    }

    #[test]
    #[should_panic]
    fn expect_code_shaped_panics_on_a_single_char_code() {
        let response = json!({ "data": { "x": { "code": "A" } } });
        expect_code_shaped(&response, "x");
    }
}
