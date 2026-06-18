#[derive(thiserror::Error, Debug)]
pub enum ConformanceError {
    #[error("go toolchain unavailable: {0}")]
    GoUnavailable(String),
    #[error("anchor build failed: {0}")]
    Build(String),
    #[error("anchor wire invalid: {0}")]
    Anchor(String),
    #[error("fabric error: {0}")]
    Fabric(#[from] br_util_nats_fabric::FabricError),
}

pub type Result<T> = std::result::Result<T, ConformanceError>;
