use std::pin::Pin;
use std::time::Duration;

use br_core_auth::{Passport, PassportHeader};
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use serde_json::Value;

pub struct SseSubscription {
    stream: Pin<Box<dyn Stream<Item = reqwest::Result<Bytes>> + Send>>,
    buffer: String,
}

impl SseSubscription {
    pub async fn open(base: &str, passport: &Passport, query: &str) -> Self {
        Self::open_at(base, "/graphql", passport, query).await
    }

    pub async fn open_at(base: &str, path: &str, passport: &Passport, query: &str) -> Self {
        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{base}{path}"))
            .header("X-Passport", passport.to_header())
            .header("Accept", "text/event-stream")
            .json(&serde_json::json!({ "query": query }))
            .send()
            .await
            .expect("subscription request failed");
        assert!(
            resp.status().is_success(),
            "subscription request returned {}",
            resp.status()
        );
        Self {
            stream: Box::pin(resp.bytes_stream()),
            buffer: String::new(),
        }
    }

    pub async fn next_event(&mut self, timeout: Duration) -> Option<Value> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if let Some(block) = self.take_block() {
                if let Some(data) = Self::parse_block(&block) {
                    return Some(data);
                }
                continue;
            }

            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return None;
            }
            match tokio::time::timeout(remaining, self.stream.next()).await {
                Ok(Some(Ok(bytes))) => {
                    self.buffer.push_str(&String::from_utf8_lossy(&bytes));
                }
                Ok(Some(Err(e))) => panic!("subscription stream errored: {e}"),
                Ok(None) | Err(_) => return None,
            }
        }
    }

    pub async fn expect_event(&mut self, what: &str, timeout: Duration) -> Value {
        self.next_event(timeout)
            .await
            .unwrap_or_else(|| panic!("expected subscription event ({what}), got none"))
    }

    pub async fn expect_silence(&mut self, what: &str, quiet: Duration) {
        if let Some(event) = self.next_event(quiet).await {
            panic!("expected no subscription event ({what}), got: {event}");
        }
    }

    fn take_block(&mut self) -> Option<String> {
        let block_end = self.buffer.find("\n\n")?;
        let block = self.buffer[..block_end].to_string();
        self.buffer = self.buffer[block_end + 2..].to_string();
        Some(block)
    }

    fn parse_block(block: &str) -> Option<Value> {
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
}
