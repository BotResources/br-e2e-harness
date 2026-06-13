use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConformanceError {
    #[error("go toolchain unavailable: {0}")]
    GoUnavailable(String),
    #[error("building the scope-service subject failed: {0}")]
    Build(String),
    #[error("nats jetstream error: {0}")]
    Jetstream(String),
    #[error("publishing a confirmation failed: {0}")]
    Publish(String),
    #[error(
        "the declare command did not deserialize into IntegrationCommand<DeclareServiceScopes>: {0}"
    )]
    NonConformantDeclare(String),
    #[error("timed out waiting for {0}")]
    Timeout(String),
}

pub type Result<T> = std::result::Result<T, ConformanceError>;
