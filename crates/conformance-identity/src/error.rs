use br_util_nats_fabric::CoordError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConformanceError {
    #[error(
        "the `{binary}` binary is not available: {detail}. Put it on an absolute PATH entry (the \
         release asset `identity-acceptor-<os>-<arch>.tar.gz` of the conformance-identity GitHub \
         release carries the acceptor) or build it from conformance-subjects/identity-acceptor \
         outside the tests"
    )]
    SubjectBinaryUnavailable { binary: String, detail: String },
    #[error("deriving the contract coordinates failed: {0}")]
    Coord(#[from] CoordError),
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
