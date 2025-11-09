use async_trait::async_trait;

use crate::types::{BatchEmbeddingRequest, BatchEmbeddingResponse, EmbeddingResult, ModelInfo};

/// Trait for embedding model providers.
///
/// Implementations can use different backends (MLX, ONNX, etc.)
/// while providing a consistent interface for embedding generation.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Generate embeddings for a batch of text inputs.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The model is not found or not loaded
    /// - Input validation fails (empty, too long)
    /// - Internal embedding generation fails
    async fn embed_batch(
        &self,
        request: BatchEmbeddingRequest,
    ) -> EmbeddingResult<BatchEmbeddingResponse>;

    /// Get model information (dimension, capabilities).
    ///
    /// # Errors
    ///
    /// Returns an error if the model is not found.
    async fn model_info(&self) -> EmbeddingResult<ModelInfo>;

    /// Health check for the embedding service.
    ///
    /// # Errors
    ///
    /// Returns an error if the service is unhealthy or models are not loaded.
    async fn health_check(&self) -> EmbeddingResult<()>;
}
