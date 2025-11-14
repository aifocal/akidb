//! Embedding Manager - Service layer for embedding generation
//!
//! Supports multiple embedding providers (Python-bridge, Mock)
//! configured via the service Config struct.
//!
//! Note: MLX provider has been deprecated in favor of Python-bridge with ONNX Runtime.

use akidb_embedding::{
    BatchEmbeddingRequest, EmbeddingProvider, MockEmbeddingProvider, ModelInfo,
    PythonBridgeProvider,
};
use std::sync::Arc;

/// Manages embedding generation using configured provider
pub struct EmbeddingManager {
    provider: Arc<dyn EmbeddingProvider + Send + Sync>,
    model_name: String,
    dimension: u32,
}

impl EmbeddingManager {
    /// Create EmbeddingManager from configuration
    ///
    /// This is the recommended way to create an EmbeddingManager.
    /// Supports multiple providers: "python-bridge", "mock"
    ///
    /// # Arguments
    ///
    /// * `provider_type` - Provider type: "python-bridge", "mock"
    /// * `model_name` - Model name (e.g., "sentence-transformers/all-MiniLM-L6-v2")
    /// * `python_path` - Optional Python executable path (for python-bridge provider)
    ///
    /// # Returns
    ///
    /// Initialized EmbeddingManager with configured provider
    ///
    /// # Errors
    ///
    /// Returns error if provider initialization fails or provider type is unknown
    pub async fn from_config(
        provider_type: &str,
        model_name: &str,
        python_path: Option<&str>,
    ) -> Result<Self, String> {
        tracing::info!(
            provider = %provider_type,
            model = %model_name,
            "Initializing embedding manager"
        );

        // Select provider based on config
        let provider: Arc<dyn EmbeddingProvider + Send + Sync> = match provider_type {
            "python-bridge" => Arc::new(
                PythonBridgeProvider::new(model_name, python_path)
                    .await
                    .map_err(|e| format!("Failed to initialize Python bridge provider: {}", e))?,
            ),
            "mock" => Arc::new(MockEmbeddingProvider::new()),
            "mlx" => {
                return Err(
                    "MLX provider has been deprecated. Use 'python-bridge' with ONNX Runtime instead."
                        .to_string(),
                );
            }
            _ => {
                return Err(format!(
                    "Unknown provider type: '{}'. Supported: python-bridge, mock",
                    provider_type
                ))
            }
        };

        // Get model info
        let model_info = provider
            .model_info()
            .await
            .map_err(|e| format!("Failed to get model info: {}", e))?;

        tracing::info!(
            provider = %provider_type,
            model = %model_name,
            dimension = %model_info.dimension,
            "Embedding manager initialized successfully"
        );

        Ok(Self {
            provider,
            model_name: model_name.to_string(),
            dimension: model_info.dimension,
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
        // Use mock provider for reliable testing
        let result = EmbeddingManager::from_config(
            "mock",
            "mock-embed-512",  // Mock provider model name
            None,
        )
        .await;

        assert!(result.is_ok());
        let manager = result.unwrap();
        assert_eq!(manager.model_name(), "mock-embed-512");
        assert_eq!(manager.dimension(), 512); // Mock provider default dimension
    }

    #[tokio::test]
    async fn test_embedding_generation() {
        let manager = EmbeddingManager::from_config(
            "mock",
            "mock-embed-512",  // Mock provider model name
            None,
        )
        .await
        .unwrap();

        let texts = vec!["Hello world".to_string(), "Machine learning".to_string()];
        let result = manager.embed(texts).await;

        assert!(result.is_ok(), "Embedding generation failed: {:?}", result.err());
        let embeddings = result.unwrap();
        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), 512); // Mock provider dimension
        assert_eq!(embeddings[1].len(), 512);
    }

    #[tokio::test]
    async fn test_vector_validation() {
        let manager = EmbeddingManager::from_config(
            "mock",
            "mock-embed-512",  // Mock provider model name
            None,
        )
        .await
        .unwrap();

        // Valid vector (512 dims for mock provider)
        let valid_vector: Vec<f32> = vec![0.001; 512];
        assert!(manager.validate_vector(&valid_vector, 512).is_ok());

        // Invalid dimension
        let invalid_vector: Vec<f32> = vec![0.001; 256];
        assert!(manager.validate_vector(&invalid_vector, 512).is_err());
    }

    #[tokio::test]
    async fn test_empty_text_list() {
        let manager = EmbeddingManager::from_config(
            "mock",
            "mock-embed-512",  // Mock provider model name
            None,
        )
        .await
        .unwrap();

        let result = manager.embed(vec![]).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }
}
