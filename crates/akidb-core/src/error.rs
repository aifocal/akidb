use thiserror::Error;

/// Unified error type shared across AkiDB crates.
#[derive(Debug, Error)]
pub enum Error {
    #[error("resource not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("not implemented: {0}")]
    NotImplemented(String),
    #[error("internal error: {0}")]
    Internal(String),
}

/// Convenience result alias binding to the shared error type.
pub type Result<T> = std::result::Result<T, Error>;
