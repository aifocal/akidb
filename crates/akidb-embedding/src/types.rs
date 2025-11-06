use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error type for embedding operations.
#[derive(Debug, Error)]
pub enum EmbeddingError {
    /// Model not found or not loaded.
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    /// Invalid input (empty, too long, etc.).
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Service unavailable or unhealthy.
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    /// Internal error during embedding generation.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type for embedding operations.
pub type EmbeddingResult<T> = Result<T, EmbeddingError>;

/// Request for batch embedding generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchEmbeddingRequest {
    /// Model identifier (e.g., "qwen3-embed-8b").
    pub model: String,
    /// Text inputs to embed.
    pub inputs: Vec<String>,
    /// Whether to L2 normalize the output vectors.
    #[serde(default)]
    pub normalize: bool,
}

/// Response from batch embedding generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchEmbeddingResponse {
    /// Model identifier that generated the embeddings.
    pub model: String,
    /// Generated embeddings (one per input).
    pub embeddings: Vec<Vec<f32>>,
    /// Usage statistics.
    pub usage: Usage,
}

/// Usage statistics for embedding requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    /// Total number of tokens processed.
    pub total_tokens: usize,
    /// Duration in milliseconds.
    pub duration_ms: u64,
}

/// Model information and capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model identifier.
    pub model: String,
    /// Output dimension of embeddings.
    pub dimension: u32,
    /// Maximum input tokens supported.
    pub max_tokens: usize,
}
