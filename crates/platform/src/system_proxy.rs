use std::net::SocketAddr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SystemProxyError {
    #[error("operation not supported on this platform")]
    Unsupported,
    #[error("I/O error while configuring the system proxy: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, SystemProxyError>;

/// Abstraction over "make the OS route HTTP(S) traffic through the local proxy".
/// Implementations must be idempotent and must restore cleanly via `disable`.
pub trait SystemProxy: Send + Sync {
    fn enable(&self, addr: SocketAddr) -> Result<()>;
    fn disable(&self) -> Result<()>;
}
