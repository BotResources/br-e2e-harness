use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConformanceError {
    #[error(
        "the `{binary}` binary is not available: {detail}. Put it on PATH (the release asset \
         `{binary}-<os>-<arch>.tar.gz` of the conformance-passport GitHub release carries it) \
         or build it from conformance-subjects/identity-passport outside the tests"
    )]
    SubjectBinaryUnavailable {
        binary: &'static str,
        detail: String,
    },
    #[error("sealing/publishing a bearer into the PUBLISHED_LANGUAGE bucket failed: {0}")]
    Seed(String),
    #[error("calling GET /internal/passport failed: {0}")]
    Request(String),
    #[error("the returned X-Passport did not decode under br_core_auth::Passport: {0}")]
    NonConformantPassport(String),
    #[error("timed out waiting for {0}")]
    Timeout(String),
    #[error("polling /readyz failed: {0}")]
    Readyz(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

pub type Result<T> = std::result::Result<T, ConformanceError>;
