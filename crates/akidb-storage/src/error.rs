/// Storage crate re-exports the shared error type for convenience.
pub type Error = akidb_core::Error;

/// Result alias bound to the shared error type.
pub type Result<T> = std::result::Result<T, Error>;
