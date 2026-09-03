use serde_json::Value;

pub(crate) fn take_block(buffer: &mut String) -> Option<String> {
    let block_end = buffer.find("\n\n")?;
    let block = buffer[..block_end].to_string();
    *buffer = buffer[block_end + 2..].to_string();
    Some(block)
}

pub(crate) fn parse_block(block: &str) -> Option<Value> {
    let mut event_type = None;
    let mut data = None;
    for line in block.lines() {
        if let Some(val) = line.strip_prefix("event:") {
            event_type = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("data:") {
            data = Some(val.trim().to_string());
        }
    }
    if event_type.as_deref() != Some("next") {
        return None;
    }
    let payload: Value = serde_json::from_str(&data?).ok()?;
    if payload["errors"] != Value::Null {
        panic!("subscription stream returned errors: {}", payload["errors"]);
    }
    let data = payload["data"].clone();
    (data != Value::Null).then_some(data)
}

pub(crate) fn event_field(event: &Value, field: &str) -> Value {
    let payload = &event[field];
    if payload.is_null() {
        panic!("expected subscription event to carry field '{field}', got: {event}");
    }
    payload.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_block_unwraps_data_on_a_next_frame() {
        let block = "event: next\ndata: {\"data\":{\"charterProposalChanged\":{\"id\":\"p1\"}}}";
        let data = parse_block(block).expect("next frame yields data");
        assert_eq!(data["charterProposalChanged"]["id"], "p1");
    }

    #[test]
    fn parse_block_ignores_non_next_frames() {
        assert!(parse_block("event: complete\ndata: {}").is_none());
        assert!(parse_block(": keep-alive comment").is_none());
    }

    #[test]
    fn parse_block_skips_a_null_data_payload() {
        assert!(parse_block("event: next\ndata: {\"data\":null}").is_none());
    }

    #[test]
    #[should_panic(expected = "subscription stream returned errors")]
    fn parse_block_fails_loud_on_an_errors_payload() {
        parse_block("event: next\ndata: {\"errors\":[{\"message\":\"boom\"}]}");
    }

    #[test]
    fn take_block_splits_on_the_frame_separator_and_keeps_the_rest() {
        let mut buffer = String::from("event: next\ndata: {}\n\nevent: next\ndata: {}\n\n");
        let block = take_block(&mut buffer).expect("a complete frame is available");
        assert_eq!(block, "event: next\ndata: {}");
        assert_eq!(buffer, "event: next\ndata: {}\n\n");
    }

    #[test]
    fn take_block_holds_a_partial_frame_back() {
        let mut buffer = String::from("event: next\ndata: {}");
        assert!(take_block(&mut buffer).is_none());
        assert_eq!(buffer, "event: next\ndata: {}");
    }

    #[test]
    fn event_field_pulls_the_named_subscription_root() {
        let event = json!({ "charterProposalChanged": { "id": "p1", "revision": 1 } });
        let pulled = event_field(&event, "charterProposalChanged");
        assert_eq!(pulled["id"], "p1");
        assert_eq!(pulled["revision"], 1);
    }

    #[test]
    #[should_panic(expected = "carry field 'missing'")]
    fn event_field_fails_loud_when_the_named_root_is_absent() {
        event_field(&json!({ "charterTenetChanged": {} }), "missing");
    }
}
