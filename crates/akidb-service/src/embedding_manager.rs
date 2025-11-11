//! Embedding Manager - Service layer for embedding generation
//!
//! # Bug Fix #5: Feature-gated MLX Support
//!
//! This module requires the "mlx" feature to be enabled (default).
//! It will not compile without MLX support since it directly uses MlxEmbeddingProvider.

use akidb_embedding::{BatchEmbeddingRequest, EmbeddingProvider, MlxEmbeddingProvider, ModelInfo};
use std::sync::Arc;

/// Manages embedding generation using the MLX provider
pub struct EmbeddingManager {
    provider: Arc<MlxEmbeddingProvider>,
    model_name: String,
    dimension: u32,
}

impl EmbeddingManager {
    /// Create a new EmbeddingManager with the specified model
    ///
    /// # Arguments
    ///
    /// * `model_name` - Name of the embedding model (e.g., "qwen3-0.6b-4bit")
    ///
    /// # Errors
    ///
    /// Returns error if model initialization fails
    ///
    /// # Bug Fix (Bug #4)
    ///
    /// Changed from sync to async to avoid runtime panics when called outside Tokio runtime.
    /// The old implementation used `block_in_place` + `Handle::current().block_on()` which
    /// panics in unit tests and CLI tools.
    pub async fn new(model_name: &str) -> Result<Self, String> {
        let provider = MlxEmbeddingProvider::new(model_name)
            .map_err(|e| format!("Failed to initialize MLX provider: {}", e))?;

        // Get model info asynchronously (no more runtime panics!)
        let dimension = provider
            .model_info()
            .await
            .map_err(|e| format!("Failed to get model info: {}", e))?
            .dimension;

        let model_name_owned = model_name.to_string();

        Ok(Self {
            provider: Arc::new(provider),
            model_name: model_name_owned,
            dimension,
        })
    }

    /// Generate embeddings for a list of texts
    ///
    /// # Arguments
    ///
    /// * `texts` - List of input texts to embed
    ///
    /// # Returns
    ///
    /// Vector of embedding vectors (each is Vec<f32>)
    ///
    /// # Errors
    ///
    /// Returns error if embedding generation fails
    pub async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, String> {
        if texts.is_empty() {
            return Err("Cannot embed empty text list".to_string());
        }

        let request = BatchEmbeddingRequest {
            model: self.model_name.clone(),
            inputs: texts,
            normalize: true,
        };

        let response = self
            .provider
            .embed_batch(request)
            .await
            .map_err(|e| format!("Embedding failed: {}", e))?;

        Ok(response.embeddings)
    }

    /// Get model information
    ///
    /// # Returns
    ///
    /// Model metadata (name, dimension, max_tokens)
    pub async fn model_info(&self) -> Result<ModelInfo, String> {
        self.provider
            .model_info()
            .await
            .map_err(|e| format!("Failed to get model info: {}", e))
    }

    /// Validate a user-provided vector
    ///
    /// # Arguments
    ///
    /// * `vector` - User-provided embedding vector
    /// * `expected_dim` - Expected dimension from collection metadata
    ///
    /// # Errors
    ///
    /// Returns error if vector dimension doesn't match expected
    pub fn validate_vector(&self, vector: &[f32], expected_dim: u32) -> Result<(), String> {
        if vector.len() != expected_dim as usize {
            return Err(format!(
                "Vector dimension mismatch: got {}, expected {}",
                vector.len(),
                expected_dim
            ));
        }

        // Optionally warn if vector is not normalized
        let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if (norm - 1.0).abs() > 0.01 {
            tracing::warn!(
                "User-provided vector not L2 normalized: norm = {:.4}, expected ~1.0",
                norm
            );
        }

        Ok(())
    }

    /// Get the model name
    pub fn model_name(&self) -> &str {
        &self.model_name
    }

    /// Get the embedding dimension
    pub fn dimension(&self) -> u32 {
        self.dimension
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_embedding_manager_creation() {
        let result = EmbeddingManager::new("qwen3-0.6b-4bit").await;

        if let Ok(manager) = result {
            assert_eq!(manager.model_name(), "qwen3-0.6b-4bit");
            assert_eq!(manager.dimension(), 1024);
        } else {
            // Skip test if Python environment not available
            println!("Skipping test: Python environment not available");
        }
    }

    #[tokio::test]
    async fn test_embedding_generation() {
        let manager = match EmbeddingManager::new("qwen3-0.6b-4bit").await {
            Ok(m) => m,
            Err(_) => {
                println!("Skipping test: Python environment not available");
                return;
            }
        };

        let texts = vec!["Hello world".to_string(), "Machine learning".to_string()];
        let result = manager.embed(texts).await;

        assert!(result.is_ok());
        let embeddings = result.unwrap();
        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), 1024);
        assert_eq!(embeddings[1].len(), 1024);
    }

    #[tokio::test]
    async fn test_vector_validation() {
        let manager = match EmbeddingManager::new("qwen3-0.6b-4bit").await {
            Ok(m) => m,
            Err(_) => {
                println!("Skipping test: Python environment not available");
                return;
            }
        };

        // Valid vector
        let valid_vector: Vec<f32> = vec![0.001; 1024];
        assert!(manager.validate_vector(&valid_vector, 1024).is_ok());

        // Invalid dimension
        let invalid_vector: Vec<f32> = vec![0.001; 512];
        assert!(manager.validate_vector(&invalid_vector, 1024).is_err());
    }

    #[tokio::test]
    async fn test_empty_text_list() {
        let manager = match EmbeddingManager::new("qwen3-0.6b-4bit").await {
            Ok(m) => m,
            Err(_) => {
                println!("Skipping test: Python environment not available");
                return;
            }
        };

        let result = manager.embed(vec![]).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }
}
