use serde_json::Value;

#[derive(Default)]
pub(crate) struct FrameBuffer {
    text: String,
    partial_utf8: Vec<u8>,
    after_carriage_return: bool,
}

impl FrameBuffer {
    pub(crate) fn push(&mut self, bytes: &[u8]) {
        self.partial_utf8.extend_from_slice(bytes);
        let complete_len = match std::str::from_utf8(&self.partial_utf8) {
            Err(e) if e.error_len().is_none() => e.valid_up_to(),
            _ => self.partial_utf8.len(),
        };
        let complete: Vec<u8> = self.partial_utf8.drain(..complete_len).collect();
        for c in String::from_utf8_lossy(&complete).chars() {
            match c {
                '\r' => {
                    self.text.push('\n');
                    self.after_carriage_return = true;
                }
                '\n' if self.after_carriage_return => self.after_carriage_return = false,
                c => {
                    self.after_carriage_return = false;
                    self.text.push(c);
                }
            }
        }
    }

    pub(crate) fn take_block(&mut self) -> Option<String> {
        let block_end = self.text.find("\n\n")?;
        let block = self.text[..block_end].to_string();
        self.text.drain(..block_end + 2);
        Some(block)
    }

    pub(crate) fn is_blank(&self) -> bool {
        self.partial_utf8.is_empty() && self.text.trim().is_empty()
    }

    pub(crate) fn unterminated(&self) -> String {
        format!(
            "{}{}",
            self.text,
            String::from_utf8_lossy(&self.partial_utf8)
        )
    }
}

pub(crate) fn parse_block(block: &str) -> Option<Value> {
    let mut event_type = None;
    let mut data_lines = Vec::new();
    for line in block.lines() {
        if let Some(val) = line.strip_prefix("event:") {
            event_type = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("data:") {
            data_lines.push(val.trim().to_string());
        }
    }
    if event_type.as_deref() != Some("next") {
        return None;
    }
    if data_lines.is_empty() {
        panic!("subscription next frame carries no data: {block:?}");
    }
    let data = data_lines.join("\n");
    let payload: Value = serde_json::from_str(&data)
        .unwrap_or_else(|e| panic!("subscription next frame data is not JSON ({e}): {data:?}"));
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
    #[should_panic(expected = "next frame data is not JSON")]
    fn parse_block_fails_loud_on_a_next_frame_whose_data_is_not_json() {
        parse_block("event: next\ndata: {\"data\":");
    }

    #[test]
    #[should_panic(expected = "next frame carries no data")]
    fn parse_block_fails_loud_on_a_next_frame_without_data() {
        parse_block("event: next");
    }

    #[test]
    fn parse_block_joins_the_data_lines_of_one_frame() {
        let data =
            parse_block("event: next\ndata: {\"data\":\ndata: {\"a\":1}}").expect("a next frame");
        assert_eq!(data["a"], 1);
    }

    fn drained(chunks: &[&[u8]]) -> Vec<String> {
        let mut buffer = FrameBuffer::default();
        let mut blocks = Vec::new();
        for chunk in chunks {
            buffer.push(chunk);
            while let Some(block) = buffer.take_block() {
                blocks.push(block);
            }
        }
        blocks
    }

    #[test]
    fn take_block_splits_on_the_frame_separator_and_keeps_the_rest() {
        let mut buffer = FrameBuffer::default();
        buffer.push(b"event: next\ndata: {}\n\nevent: next\ndata: {}\n\n");
        let block = buffer.take_block().expect("a complete frame is available");
        assert_eq!(block, "event: next\ndata: {}");
        assert_eq!(buffer.unterminated(), "event: next\ndata: {}\n\n");
    }

    #[test]
    fn take_block_holds_a_partial_frame_back() {
        let mut buffer = FrameBuffer::default();
        buffer.push(b"event: next\ndata: {}");
        assert!(buffer.take_block().is_none());
        assert_eq!(buffer.unterminated(), "event: next\ndata: {}");
        assert!(!buffer.is_blank());
    }

    #[test]
    fn crlf_cr_and_lf_terminated_frames_all_split_into_the_same_block() {
        let expected = vec!["event: next\ndata: {}".to_string()];
        assert_eq!(drained(&[b"event: next\r\ndata: {}\r\n\r\n"]), expected);
        assert_eq!(drained(&[b"event: next\rdata: {}\r\r"]), expected);
        assert_eq!(drained(&[b"event: next\ndata: {}\n\n"]), expected);
        assert_eq!(drained(&[b"event: next\r\ndata: {}\n\n"]), expected);
    }

    #[test]
    fn a_crlf_split_across_two_chunks_is_one_line_end() {
        let expected = vec!["event: next\ndata: {}".to_string()];
        assert_eq!(
            drained(&[b"event: next\r\ndata: {}\r\n\r", b"\n"]),
            expected
        );
        assert_eq!(
            drained(&[b"event: next\r", b"\ndata: {}\r\n\r\n"]),
            expected
        );
    }

    #[test]
    fn a_multibyte_char_split_across_two_chunks_survives() {
        let frame = "event: next\ndata: {\"data\":{\"name\":\"é\"}}\n\n".as_bytes();
        let cut = frame
            .iter()
            .position(|b| *b == 0xC3)
            .expect("a multibyte lead byte")
            + 1;
        let blocks = drained(&[&frame[..cut], &frame[cut..]]);
        let data = parse_block(&blocks[0]).expect("a next frame");
        assert_eq!(data["name"], "é");
    }

    #[test]
    fn a_half_received_multibyte_char_is_not_a_blank_buffer() {
        let mut buffer = FrameBuffer::default();
        buffer.push(&"é".as_bytes()[..1]);
        assert!(!buffer.is_blank());
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
