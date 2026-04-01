use thiserror::Error;

#[derive(Debug, Error)]
pub enum PixmaError {
    #[error("no printer found on the network")]
    NotFound,

    #[error("BJNP protocol error: {0}")]
    Protocol(String),

    #[error("invalid BJNP packet: {0}")]
    InvalidPacket(String),

    #[error("scanner busy")]
    Busy,

    #[error("scan failed: {0}")]
    ScanFailed(String),

    #[error("network error: {0}")]
    Io(#[from] std::io::Error),

    #[error("timeout waiting for response")]
    Timeout,
}
