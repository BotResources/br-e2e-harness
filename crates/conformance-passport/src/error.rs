use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConformanceError {
    #[error("go toolchain unavailable: {0}")]
    GoUnavailable(String),
    #[error("building the identity-passport subject failed: {0}")]
    Build(String),
    #[error("nats jetstream error: {0}")]
    Jetstream(String),
    #[error("seeding a bearer token into the bearer_tokens bucket failed: {0}")]
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

impl From<br_test_harness::BearerSeedError> for ConformanceError {
    fn from(e: br_test_harness::BearerSeedError) -> Self {
        ConformanceError::Seed(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, ConformanceError>;
