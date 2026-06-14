use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_nats::jetstream::{self, consumer};
use br_core_integration::IntegrationEvent;
use br_core_scope::{ScopeDeclarationError, ServiceScopesAccepted, ServiceScopesRejected};
use futures_util::StreamExt as _;
use uuid::Uuid;

use crate::error::{ConformanceError, Result};
use crate::subjects::{accepted_event_subject, rejected_event_subject};

#[derive(Clone)]
pub struct CapturedConfirmation {
    pub subject: String,
    pub raw: Vec<u8>,
    pub correlation_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Accepted { service: String },
    Rejected { reason: ScopeDeclarationError },
}

impl CapturedConfirmation {
    pub fn decode(&self, accepted_subject: &str, rejected_subject: &str) -> Result<Verdict> {
        if self.subject == accepted_subject {
            let event: IntegrationEvent<ServiceScopesAccepted> = serde_json::from_slice(&self.raw)
                .map_err(|e| {
                    ConformanceError::NonConformantConfirmation(format!("accepted: {e}"))
                })?;
            Ok(Verdict::Accepted {
                service: event.payload.service.as_str().to_string(),
            })
        } else if self.subject == rejected_subject {
            let event: IntegrationEvent<ServiceScopesRejected> = serde_json::from_slice(&self.raw)
                .map_err(|e| {
                    ConformanceError::NonConformantConfirmation(format!("rejected: {e}"))
                })?;
            Ok(Verdict::Rejected {
                reason: event.payload.reason,
            })
        } else {
            Err(ConformanceError::NonConformantConfirmation(format!(
                "confirmation on unexpected subject {:?}",
                self.subject
            )))
        }
    }
}

pub struct ConfirmationCapture {
    captured: Arc<Mutex<Vec<CapturedConfirmation>>>,
    drain_error: Arc<Mutex<Option<String>>>,
    task: tokio::task::JoinHandle<()>,
    accepted_subject: String,
    rejected_subject: String,
}

impl ConfirmationCapture {
    pub async fn start(js: &jetstream::Context, stream_name: &str) -> Result<Self> {
        let accepted = accepted_event_subject()?;
        let rejected = rejected_event_subject()?;
        let stream = js
            .get_stream(stream_name)
            .await
            .map_err(|e| ConformanceError::Jetstream(format!("get stream '{stream_name}': {e}")))?;
        let consumer = stream
            .create_consumer(consumer::pull::Config {
                deliver_policy: consumer::DeliverPolicy::New,
                ack_policy: consumer::AckPolicy::None,
                filter_subjects: vec![accepted.clone(), rejected.clone()],
                inactive_threshold: Duration::from_secs(300),
                ..Default::default()
            })
            .await
            .map_err(|e| {
                ConformanceError::Jetstream(format!("create confirmation consumer: {e}"))
            })?;

        let captured = Arc::new(Mutex::new(Vec::new()));
        let drain_error = Arc::new(Mutex::new(None));
        let sink = captured.clone();
        let error_sink = drain_error.clone();
        let task = tokio::spawn(async move {
            let mut messages = match consumer.messages().await {
                Ok(messages) => messages,
                Err(e) => {
                    record_drain_error(
                        &error_sink,
                        format!("opening the confirmation stream: {e}"),
                    );
                    return;
                }
            };
            while let Some(result) = messages.next().await {
                let message = match result {
                    Ok(message) => message,
                    Err(e) => {
                        record_drain_error(
                            &error_sink,
                            format!("draining the confirmation stream: {e}"),
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
                    .push(CapturedConfirmation {
                        subject: message.subject.to_string(),
                        raw,
                        correlation_id,
                    });
            }
        });

        Ok(Self {
            captured,
            drain_error,
            task,
            accepted_subject: accepted,
            rejected_subject: rejected,
        })
    }

    pub fn confirmation_for(&self, correlation_id: Uuid) -> Option<CapturedConfirmation> {
        self.assert_draining();
        self.captured
            .lock()
            .expect("capture mutex poisoned")
            .iter()
            .find(|c| c.correlation_id == correlation_id)
            .cloned()
    }

    pub fn verdict_for(&self, correlation_id: Uuid) -> Option<Result<Verdict>> {
        self.confirmation_for(correlation_id)
            .map(|c| c.decode(&self.accepted_subject, &self.rejected_subject))
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
            panic!("confirmation capture drain failed: {cause}");
        }
    }

    pub async fn stop(self) {
        self.task.abort();
        let _ = self.task.await;
    }
}

fn record_drain_error(slot: &Arc<Mutex<Option<String>>>, cause: String) {
    eprintln!("confirmation capture aborted: {cause}");
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
