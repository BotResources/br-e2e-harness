mod outcome;
mod parse;

use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use br_core_auth::{Passport, PassportHeader};
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use serde_json::Value;

use crate::spawned_process::SpawnedProcess;
use parse::{event_field, parse_block, take_block};

pub use outcome::{DrainStop, SseOutcome};

const LOG_TAIL_LINES: usize = 80;

pub struct SseSubscription {
    stream: Pin<Box<dyn Stream<Item = reqwest::Result<Bytes>> + Send>>,
    buffer: String,
    closed: bool,
    logs: Option<Arc<Mutex<String>>>,
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
            closed: false,
            logs: None,
        }
    }

    #[must_use]
    pub fn with_logs(mut self, process: &SpawnedProcess) -> Self {
        self.logs = Some(process.logs_handle());
        self
    }

    pub async fn next_outcome(&mut self, timeout: Duration) -> SseOutcome {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if let Some(block) = take_block(&mut self.buffer) {
                if let Some(data) = parse_block(&block) {
                    return SseOutcome::Event(data);
                }
                continue;
            }
            if self.closed {
                assert!(
                    self.buffer.trim().is_empty(),
                    "subscription stream closed with an unterminated block: {:?}{}",
                    self.buffer,
                    self.log_tail()
                );
                return SseOutcome::Closed;
            }

            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return SseOutcome::Timeout;
            }
            match tokio::time::timeout(remaining, self.stream.next()).await {
                Ok(Some(Ok(bytes))) => {
                    self.buffer.push_str(&String::from_utf8_lossy(&bytes));
                }
                Ok(Some(Err(e))) => panic!("subscription stream errored: {e}"),
                Ok(None) => self.closed = true,
                Err(_) => return SseOutcome::Timeout,
            }
        }
    }

    pub async fn next_event(&mut self, timeout: Duration) -> Option<Value> {
        match self.next_outcome(timeout).await {
            SseOutcome::Event(data) => Some(data),
            SseOutcome::Timeout | SseOutcome::Closed => None,
        }
    }

    pub async fn expect_event(&mut self, what: &str, timeout: Duration) -> Value {
        let outcome = self.next_outcome(timeout).await;
        match outcome {
            SseOutcome::Event(data) => data,
            SseOutcome::Timeout => panic!(
                "expected subscription event ({what}), got Timeout: nothing arrived within {timeout:?}{}",
                self.log_tail()
            ),
            SseOutcome::Closed => panic!(
                "expected subscription event ({what}), got Closed: the server closed the stream{}",
                self.log_tail()
            ),
        }
    }

    pub async fn expect_event_on(&mut self, field: &str, timeout: Duration) -> Value {
        let event = self.expect_event(field, timeout).await;
        event_field(&event, field)
    }

    pub async fn expect_silence(&mut self, what: &str, quiet: Duration) {
        let outcome = self.next_outcome(quiet).await;
        match outcome {
            SseOutcome::Timeout => {}
            SseOutcome::Event(event) => {
                panic!("expected no subscription event ({what}), got: {event}")
            }
            SseOutcome::Closed => panic!(
                "expected no subscription event ({what}), got Closed: the server closed the stream, which is not silence{}",
                self.log_tail()
            ),
        }
    }

    pub async fn drain(&mut self, max: usize, timeout: Duration) -> usize {
        self.drain_outcome(max, timeout).await.0
    }

    pub async fn drain_outcome(&mut self, max: usize, timeout: Duration) -> (usize, DrainStop) {
        let mut drained = 0;
        while drained < max {
            match self.next_outcome(timeout).await {
                SseOutcome::Event(_) => drained += 1,
                SseOutcome::Timeout => return (drained, DrainStop::Timeout),
                SseOutcome::Closed => return (drained, DrainStop::Closed),
            }
        }
        (drained, DrainStop::Limit)
    }

    fn log_tail(&self) -> String {
        let Some(logs) = self.logs.as_ref() else {
            return String::new();
        };
        let captured = logs.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut tail: Vec<&str> = captured.lines().rev().take(LOG_TAIL_LINES).collect();
        tail.reverse();
        format!(
            "\n--- service log tail (last {LOG_TAIL_LINES} lines) ---\n{}",
            tail.join("\n")
        )
    }
}
