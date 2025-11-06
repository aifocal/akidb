use thiserror::Error;

/// Canonical error type for core metadata operations.
#[derive(Debug, Error)]
pub enum CoreError {
    /// Entity was not found in the metadata store.
    #[error("{entity} `{id}` was not found")]
    NotFound {
        /// Entity type name (e.g. `"tenant"`).
        entity: &'static str,
        /// Identifier of the missing entity.
        id: String,
    },

    /// Entity already exists and cannot be created again.
    #[error("{entity} `{id}` already exists")]
    AlreadyExists {
        /// Entity type name (e.g. `"tenant"`).
        entity: &'static str,
        /// Identifier that conflicts.
        id: String,
    },

    /// Resource quotas prohibit the attempted operation.
    #[error("quota exceeded: {message}")]
    QuotaExceeded {
        /// Human-readable quota violation message.
        message: String,
    },

    /// Operation violates current state machine rules.
    #[error("invalid state: {message}")]
    InvalidState {
        /// Human-readable explanation of the invalid state.
        message: String,
    },

    /// Unexpected internal error occurred.
    #[error("internal error: {message}")]
    Internal {
        /// Human-readable details for debugging purposes.
        message: String,
    },
}

impl CoreError {
    /// Creates a `NotFound` variant.
    #[must_use]
    pub fn not_found(entity: &'static str, id: impl Into<String>) -> Self {
        Self::NotFound {
            entity,
            id: id.into(),
        }
    }

    /// Creates an `AlreadyExists` variant.
    #[must_use]
    pub fn already_exists(entity: &'static str, id: impl Into<String>) -> Self {
        Self::AlreadyExists {
            entity,
            id: id.into(),
        }
    }

    /// Creates an `InvalidState` variant.
    #[must_use]
    pub fn invalid_state(message: impl Into<String>) -> Self {
        Self::InvalidState {
            message: message.into(),
        }
    }

    /// Creates an `Internal` variant.
    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }
}

/// Convenient result alias for core operations.
pub type CoreResult<T> = Result<T, CoreError>;
