use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConformanceError {
    #[error("go toolchain unavailable: {0}")]
    GoUnavailable(String),
    #[error("building the identity-directory anchor failed: {0}")]
    Build(String),
    #[error("running the identity-directory anchor failed: {0}")]
    Run(String),
    #[error("the anchor snapshot did not parse as a {{key, value}} transport: {0}")]
    Snapshot(String),
    #[error(
        "the Go-frozen value at {key} did not deserialize through br_core_directory::{ty}: {cause}"
    )]
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
