use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_nats::jetstream::{self, consumer};
use br_core_integration::{CommandCoords, EventCoords};
use br_util_nats_fabric::{
    CorrelatedAwaiter, EventMetadata, INTEGRATION_CMD, INTEGRATION_EVT, command_subject,
    event_subject,
};
use futures_util::StreamExt as _;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct CapturedMessage {
    pub subject: String,
    pub metadata: EventMetadata,
    pub payload: Vec<u8>,
}

#[derive(serde::Deserialize)]
struct CorrelationProbe {
    metadata: EventMetadata,
}

struct CaptureState {
    messages: Vec<CapturedMessage>,
    drain_error: Option<String>,
}

pub struct EventCapture(Capture);
pub struct CommandCapture(Capture);

struct Capture {
    state: Arc<Mutex<CaptureState>>,
    task: tokio::task::JoinHandle<()>,
}

impl EventCapture {
    pub fn count(&self) -> usize {
        self.0.count()
    }
    pub fn first(&self) -> Option<CapturedMessage> {
        self.0.first()
    }
    pub fn for_correlation(&self, correlation_id: Uuid) -> Vec<CapturedMessage> {
        self.0.for_correlation(correlation_id)
    }
    pub fn correlation_ids(&self) -> Vec<Uuid> {
        self.0.correlation_ids()
    }
    pub async fn stop(self) {
        self.0.stop().await
    }
}

impl CommandCapture {
    pub fn count(&self) -> usize {
        self.0.count()
    }
    pub fn first(&self) -> Option<CapturedMessage> {
        self.0.first()
    }
    pub fn for_correlation(&self, correlation_id: Uuid) -> Vec<CapturedMessage> {
        self.0.for_correlation(correlation_id)
    }
    pub fn correlation_ids(&self) -> Vec<Uuid> {
        self.0.correlation_ids()
    }
    pub async fn stop(self) {
        self.0.stop().await
    }
}

impl Capture {
    fn count(&self) -> usize {
        self.guarded().messages.len()
    }

    fn first(&self) -> Option<CapturedMessage> {
        self.guarded().messages.first().cloned()
    }

    fn for_correlation(&self, correlation_id: Uuid) -> Vec<CapturedMessage> {
        self.guarded()
            .messages
            .iter()
            .filter(|m| m.metadata.correlation_id == correlation_id)
            .cloned()
            .collect()
    }

    fn correlation_ids(&self) -> Vec<Uuid> {
        self.guarded()
            .messages
            .iter()
            .map(|m| m.metadata.correlation_id)
            .collect()
    }

    async fn stop(self) {
        self.task.abort();
        let _ = self.task.await;
    }

    fn guarded(&self) -> std::sync::MutexGuard<'_, CaptureState> {
        let state = self.state.lock().expect("capture mutex poisoned");
        if let Some(cause) = &state.drain_error {
            panic!("fabric capture drain failed: {cause}");
        }
        state
    }
}

async fn start_capture(
    js: &jetstream::Context,
    stream_name: &'static str,
    filters: Vec<String>,
) -> Capture {
    let stream = js
        .get_stream(stream_name)
        .await
        .unwrap_or_else(|e| panic!("capture: fixed stream {stream_name} must exist: {e}"));
    let consumer = stream
        .create_consumer(consumer::pull::Config {
            deliver_policy: consumer::DeliverPolicy::All,
            ack_policy: consumer::AckPolicy::None,
            filter_subjects: filters,
            inactive_threshold: Duration::from_secs(300),
            ..Default::default()
        })
        .await
        .unwrap_or_else(|e| panic!("capture: create consumer on {stream_name}: {e}"));

    let state = Arc::new(Mutex::new(CaptureState {
        messages: Vec::new(),
        drain_error: None,
    }));
    let sink = state.clone();
    let task = tokio::spawn(async move {
        let mut messages = match consumer.messages().await {
            Ok(messages) => messages,
            Err(e) => {
                record_error(&sink, format!("opening {stream_name}: {e}"));
                return;
            }
        };
        while let Some(result) = messages.next().await {
            let message = match result {
                Ok(message) => message,
                Err(e) => {
                    record_error(&sink, format!("draining {stream_name}: {e}"));
                    return;
                }
            };
            let payload = message.payload.to_vec();
            let Ok(probe) = serde_json::from_slice::<CorrelationProbe>(&payload) else {
                continue;
            };
            sink.lock()
                .expect("capture mutex poisoned")
                .messages
                .push(CapturedMessage {
                    subject: message.subject.as_str().to_string(),
                    metadata: probe.metadata,
                    payload,
                });
        }
    });

    Capture { state, task }
}

fn record_error(sink: &Arc<Mutex<CaptureState>>, cause: String) {
    let mut state = sink.lock().expect("capture mutex poisoned");
    if state.drain_error.is_none() {
        state.drain_error = Some(cause);
    }
}

pub async fn capture_events(js: &jetstream::Context, coords: &[&EventCoords]) -> EventCapture {
    let filters = coords.iter().map(|c| event_subject(c)).collect();
    EventCapture(start_capture(js, INTEGRATION_EVT, filters).await)
}

pub async fn capture_commands(
    js: &jetstream::Context,
    coords: &[&CommandCoords],
) -> CommandCapture {
    let filters = coords.iter().map(|c| command_subject(c)).collect();
    CommandCapture(start_capture(js, INTEGRATION_CMD, filters).await)
}

pub struct FabricAwaiter {
    inner: CorrelatedAwaiter,
}

impl FabricAwaiter {
    pub fn new(inner: CorrelatedAwaiter) -> Self {
        Self { inner }
    }

    pub async fn await_correlation(
        &mut self,
        correlation_id: Uuid,
        deadline: Duration,
    ) -> Option<CapturedMessage> {
        match self
            .inner
            .await_correlation(correlation_id, deadline)
            .await
            .expect("fabric awaiter: subscription ended unexpectedly")
        {
            Some(m) => Some(CapturedMessage {
                subject: m.subject,
                metadata: m.metadata,
                payload: m.payload,
            }),
            None => None,
        }
    }
}

pub async fn subscribe_command(js: &jetstream::Context, coords: &CommandCoords) -> CommandAwaiter {
    let subject = command_subject(coords);
    let subscriber = js
        .client()
        .subscribe(subject.clone())
        .await
        .unwrap_or_else(|e| panic!("await_command: subscribe {subject}: {e}"));
    CommandAwaiter { subscriber }
}

pub struct CommandAwaiter {
    subscriber: async_nats::Subscriber,
}

impl CommandAwaiter {
    pub async fn await_correlation(
        &mut self,
        correlation_id: Uuid,
        deadline: Duration,
    ) -> Option<CapturedMessage> {
        let wait = tokio::time::sleep(deadline);
        tokio::pin!(wait);
        loop {
            tokio::select! {
                () = &mut wait => return None,
                next = self.subscriber.next() => {
                    let message = next?;
                    let payload = message.payload.to_vec();
                    let Ok(probe) = serde_json::from_slice::<CorrelationProbe>(&payload) else {
                        continue;
                    };
                    if probe.metadata.correlation_id == correlation_id {
                        return Some(CapturedMessage {
                            subject: message.subject.as_str().to_string(),
                            metadata: probe.metadata,
                            payload,
                        });
                    }
                }
            }
        }
    }
}
