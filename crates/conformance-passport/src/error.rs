use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConformanceError {
    #[error("go toolchain unavailable: {0}")]
    GoUnavailable(String),
    #[error("building the identity-passport subject failed: {0}")]
    Build(String),
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
