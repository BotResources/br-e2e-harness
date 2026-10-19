use br_core_integration::CoordError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConformanceError {
    #[error(
        "the `{binary}` binary is not available: {detail}. Put it on PATH (the release asset \
         `{binary}-<os>-<arch>.tar.gz` of the conformance-scope GitHub release carries it) \
         or build it from conformance-subjects/scope-service outside the tests"
    )]
    SubjectBinaryUnavailable {
        binary: &'static str,
        detail: String,
    },
    #[error("deriving a contract subject failed: {0}")]
    Coord(#[from] CoordError),
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
    #[error("polling /readyz failed: {0}")]
    Readyz(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

pub type Result<T> = std::result::Result<T, ConformanceError>;
