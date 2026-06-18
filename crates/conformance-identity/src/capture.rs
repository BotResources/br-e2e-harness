use br_core_integration::IntegrationEvent;
use br_core_scope::{ScopeDeclarationError, ServiceScopesAccepted, ServiceScopesRejected};
use br_scope_declaration_contract::{accepted_event_coords, rejected_event_coords};
use br_test_harness::{CapturedMessage, EventCapture, FabricTestNats};
use br_util_nats_fabric::event_subject;
use uuid::Uuid;

use crate::error::{ConformanceError, Result};

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

impl From<CapturedMessage> for CapturedConfirmation {
    fn from(message: CapturedMessage) -> Self {
        Self {
            subject: message.subject,
            correlation_id: message.metadata.correlation_id,
            raw: message.payload,
        }
    }
}

pub struct ConfirmationCapture {
    inner: EventCapture,
    accepted_subject: String,
    rejected_subject: String,
}

impl ConfirmationCapture {
    pub async fn start(harness: &FabricTestNats) -> Result<Self> {
        let accepted_coords = accepted_event_coords()?;
        let rejected_coords = rejected_event_coords()?;
        let inner = harness
            .capture_events(&[&accepted_coords, &rejected_coords])
            .await;
        Ok(Self {
            inner,
            accepted_subject: event_subject(&accepted_coords),
            rejected_subject: event_subject(&rejected_coords),
        })
    }

    pub fn confirmation_for(&self, correlation_id: Uuid) -> Option<CapturedConfirmation> {
        self.inner
            .for_correlation(correlation_id)
            .into_iter()
            .next()
            .map(CapturedConfirmation::from)
    }

    pub fn verdict_for(&self, correlation_id: Uuid) -> Option<Result<Verdict>> {
        self.confirmation_for(correlation_id)
            .map(|c| c.decode(&self.accepted_subject, &self.rejected_subject))
    }

    pub fn count(&self) -> usize {
        self.inner.count()
    }

    pub async fn stop(self) {
        self.inner.stop().await
    }
}
