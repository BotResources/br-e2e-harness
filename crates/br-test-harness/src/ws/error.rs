#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum WsError {
    #[error("ws: timed out waiting for a `next` push")]
    Timeout,
    #[error("ws: socket closed before a `next` push")]
    Closed,
    #[error("ws: subscription completed before any push")]
    Completed,
    #[error("ws: subscription error frame: {0}")]
    ErrorFrame(String),
    #[error("ws: {0}")]
    Transport(String),
}
