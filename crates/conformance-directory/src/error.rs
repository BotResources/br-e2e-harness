use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConformanceError {
    #[error(
        "the `{binary}` binary is not available: {detail}. Put it on PATH (the release asset \
         `{binary}-<os>-<arch>.tar.gz` of the conformance-directory GitHub release carries it) \
         or build it from conformance-subjects/identity-directory outside the tests"
    )]
    AnchorBinaryUnavailable {
        binary: &'static str,
        detail: String,
    },
    #[error("running the identity-directory anchor failed: {0}")]
    Run(String),
    #[error("the anchor snapshot did not parse as a {{key, value}} transport: {0}")]
    Snapshot(String),
    #[error("the wire entry at {key} is not conformant to br_core_directory::{ty}: {cause}")]
    NonConformantWire {
        key: String,
        ty: &'static str,
        cause: String,
    },
    #[error("nats jetstream error: {0}")]
    Jetstream(String),
    #[error("directory kit error: {0}")]
    Directory(String),
    #[error("postgres error: {0}")]
    Postgres(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

pub type Result<T> = std::result::Result<T, ConformanceError>;
