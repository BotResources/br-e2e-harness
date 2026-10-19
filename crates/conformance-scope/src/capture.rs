use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};

use br_core_integration::IntegrationCommand;
use br_core_scope::DeclareServiceScopes;
use br_scope_declaration_contract::declare_command_coords;
use br_test_harness::{CapturedMessage, CommandCapture, FabricTestNats};
use uuid::Uuid;

use crate::error::{ConformanceError, Result};
use crate::prerequisite::panic_message;

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

impl From<CapturedMessage> for CapturedDeclare {
    fn from(message: CapturedMessage) -> Self {
        Self {
            raw: message.payload,
            correlation_id: message.metadata.correlation_id,
        }
    }
}

pub struct DeclareCapture {
    inner: CommandCapture,
}

impl DeclareCapture {
    pub async fn start(harness: &FabricTestNats) -> Result<Self> {
        let coords = declare_command_coords()?;
        let inner = harness.capture_commands(&[&coords]).await;
        Ok(Self { inner })
    }

    pub fn count(&self) -> usize {
        self.inner.count()
    }

    pub fn first(&self) -> Option<CapturedDeclare> {
        self.inner.first().map(CapturedDeclare::from)
    }

    pub fn correlation_ids(&self) -> Vec<Uuid> {
        self.inner.correlation_ids()
    }

    pub fn try_count(&self) -> Result<usize> {
        catch_unwind(AssertUnwindSafe(|| self.count())).map_err(capture_read_failure)
    }

    pub fn try_first(&self) -> Result<Option<CapturedDeclare>> {
        catch_unwind(AssertUnwindSafe(|| self.first())).map_err(capture_read_failure)
    }

    pub fn try_correlation_ids(&self) -> Result<Vec<Uuid>> {
        catch_unwind(AssertUnwindSafe(|| self.correlation_ids())).map_err(capture_read_failure)
    }

    pub async fn stop(self) {
        self.inner.stop().await
    }
}

fn capture_read_failure(payload: Box<dyn Any + Send>) -> ConformanceError {
    ConformanceError::Jetstream(format!(
        "reading the declare capture: {}",
        panic_message(payload)
    ))
}
