use br_core_integration::SubjectError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConformanceError {
    #[error("go toolchain unavailable: {0}")]
    GoUnavailable(String),
    #[error("building the identity-acceptor subject failed: {0}")]
    Build(String),
    #[error("deriving a contract subject failed: {0}")]
    Subject(#[from] SubjectError),
    #[error("nats jetstream error: {0}")]
    Jetstream(String),
    #[error("publishing a declare command failed: {0}")]
    Publish(String),
    #[error("a confirmation did not deserialize into IntegrationEvent<ServiceScopes…>: {0}")]
    NonConformantConfirmation(String),
    #[error("timed out waiting for {0}")]
    Timeout(String),
    #[error("polling /readyz failed: {0}")]
    Readyz(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("DeclarationOutcome gained an unknown variant — the oracle must be updated to map it")]
    OracleOutcomeUnknown,
}

pub type Result<T> = std::result::Result<T, ConformanceError>;
