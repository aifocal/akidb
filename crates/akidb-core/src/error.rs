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

    /// I/O error occurred during file or network operations.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Serialization error occurred.
    #[error("serialization error: {0}")]
    SerializationError(String),

    /// Deserialization error occurred.
    #[error("deserialization error: {0}")]
    DeserializationError(String),

    /// Storage backend error.
    #[error("storage error: {0}")]
    StorageError(String),

    /// Validation error for input data.
    #[error("validation error: {0}")]
    ValidationError(String),
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

impl From<serde_json::Error> for CoreError {
    fn from(err: serde_json::Error) -> Self {
        if err.is_eof() || err.is_syntax() {
            Self::DeserializationError(err.to_string())
        } else {
            Self::SerializationError(err.to_string())
        }
    }
}

/// Convenient result alias for core operations.
pub type CoreResult<T> = Result<T, CoreError>;
