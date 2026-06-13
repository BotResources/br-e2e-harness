use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_nats::jetstream::{self, consumer};
use br_core_integration::IntegrationCommand;
use br_core_scope::DeclareServiceScopes;
use futures_util::StreamExt as _;
use uuid::Uuid;

use crate::error::{ConformanceError, Result};
use crate::subjects::DECLARE_SUBJECT;

#[derive(Clone)]
pub struct CapturedDeclare {
    pub raw: Vec<u8>,
    pub correlation_id: Uuid,
}

impl CapturedDeclare {
    pub fn decode(&self) -> Result<IntegrationCommand<DeclareServiceScopes>> {
        serde_json::from_slice::<IntegrationCommand<DeclareServiceScopes>>(&self.raw)
            .map_err(|e| ConformanceError::NonConformantDeclare(e.to_string()))
    }
}

pub struct DeclareCapture {
    captured: Arc<Mutex<Vec<CapturedDeclare>>>,
    drain_error: Arc<Mutex<Option<String>>>,
    task: tokio::task::JoinHandle<()>,
}

impl DeclareCapture {
    pub async fn start(js: &jetstream::Context, stream_name: &str) -> Result<Self> {
        let stream = js
            .get_stream(stream_name)
            .await
            .map_err(|e| ConformanceError::Jetstream(format!("get stream '{stream_name}': {e}")))?;
        let consumer = stream
            .create_consumer(consumer::pull::Config {
                deliver_policy: consumer::DeliverPolicy::New,
                ack_policy: consumer::AckPolicy::None,
                filter_subject: DECLARE_SUBJECT.to_string(),
                inactive_threshold: Duration::from_secs(300),
                ..Default::default()
            })
            .await
            .map_err(|e| ConformanceError::Jetstream(format!("create declare consumer: {e}")))?;

        let captured = Arc::new(Mutex::new(Vec::new()));
        let drain_error = Arc::new(Mutex::new(None));
        let sink = captured.clone();
        let error_sink = drain_error.clone();
        let task = tokio::spawn(async move {
            let mut messages = match consumer.messages().await {
                Ok(messages) => messages,
                Err(e) => {
                    record_drain_error(&error_sink, format!("opening the declare stream: {e}"));
                    return;
                }
            };
            while let Some(result) = messages.next().await {
                let message = match result {
                    Ok(message) => message,
                    Err(e) => {
                        record_drain_error(
                            &error_sink,
                            format!("draining the declare stream: {e}"),
                        );
                        return;
                    }
                };
                let raw = message.payload.to_vec();
                let Some(correlation_id) = correlation_id_of(&raw) else {
                    continue;
                };
                sink.lock()
                    .expect("capture mutex poisoned")
                    .push(CapturedDeclare {
                        raw,
                        correlation_id,
                    });
            }
        });

        Ok(Self {
            captured,
            drain_error,
            task,
        })
    }

    pub fn declares(&self) -> Vec<CapturedDeclare> {
        self.assert_draining();
        self.captured
            .lock()
            .expect("capture mutex poisoned")
            .clone()
    }

    pub fn count(&self) -> usize {
        self.assert_draining();
        self.captured.lock().expect("capture mutex poisoned").len()
    }

    fn assert_draining(&self) {
        if let Some(cause) = self
            .drain_error
            .lock()
            .expect("drain-error mutex poisoned")
            .as_ref()
        {
            panic!("declare capture drain failed: {cause}");
        }
    }

    pub fn first(&self) -> Option<CapturedDeclare> {
        self.captured
            .lock()
            .expect("capture mutex poisoned")
            .first()
            .cloned()
    }

    pub fn correlation_ids(&self) -> Vec<Uuid> {
        self.captured
            .lock()
            .expect("capture mutex poisoned")
            .iter()
            .map(|d| d.correlation_id)
            .collect()
    }

    pub async fn stop(self) {
        self.task.abort();
        let _ = self.task.await;
    }
}

fn record_drain_error(slot: &Arc<Mutex<Option<String>>>, cause: String) {
    eprintln!("declare capture aborted: {cause}");
    *slot.lock().expect("drain-error mutex poisoned") = Some(cause);
}

fn correlation_id_of(raw: &[u8]) -> Option<Uuid> {
    serde_json::from_slice::<CorrelationProbe>(raw)
        .ok()
        .map(|probe| probe.metadata.correlation_id)
}

#[derive(serde::Deserialize)]
struct CorrelationProbe {
    metadata: ProbeMetadata,
}

#[derive(serde::Deserialize)]
struct ProbeMetadata {
    correlation_id: Uuid,
}
