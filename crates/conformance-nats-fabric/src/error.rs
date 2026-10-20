#[derive(thiserror::Error, Debug)]
pub enum ConformanceError {
    #[error(
        "the `{binary}` binary is not available: {detail}. Put it on PATH (the release asset \
         `{binary}-<os>-<arch>.tar.gz` of the conformance-nats-fabric GitHub release carries it) \
         or build it from conformance-subjects/nats-fabric outside the tests"
    )]
    AnchorBinaryUnavailable {
        binary: &'static str,
        detail: String,
    },
    #[error("running the nats-fabric anchor failed: {0}")]
    Run(String),
    #[error("anchor wire invalid: {0}")]
    Anchor(String),
    #[error("unknown check `{0}`: run with --help for the list of check ids")]
    UnknownCheck(String),
    #[error("check `{0}` is selected more than once: a repeated check would be counted twice")]
    DuplicateCheck(String),
    #[error("no check to run: {0}")]
    NoCheckToRun(&'static str),
    #[error("no NATS server to run against: {0}")]
    BrokerUnavailable(String),
    #[error("fabric error: {0}")]
    Fabric(#[from] br_util_nats_fabric::FabricError),
}

pub type Result<T> = std::result::Result<T, ConformanceError>;
