use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

use async_trait::async_trait;

use crate::provider::EmbeddingProvider;
use crate::types::{BatchEmbeddingRequest, BatchEmbeddingResponse, EmbeddingError, EmbeddingResult, ModelInfo, Usage};

/// Mock embedding provider for testing.
///
/// Generates deterministic embeddings based on input hash, allowing
/// integration tests to run without ML dependencies.
pub struct MockEmbeddingProvider {
    model: String,
    dimension: u32,
    latency_ms: u64,
}

impl MockEmbeddingProvider {
    /// Default model name for mock provider.
    pub const DEFAULT_MODEL: &'static str = "mock-embed-512";
    /// Default dimension (512).
    pub const DEFAULT_DIMENSION: u32 = 512;
    /// Default latency simulation (20ms).
    pub const DEFAULT_LATENCY_MS: u64 = 20;

    /// Creates a new mock provider with default parameters.
    #[must_use]
    pub fn new() -> Self {
        Self {
            model: Self::DEFAULT_MODEL.to_string(),
            dimension: Self::DEFAULT_DIMENSION,
            latency_ms: Self::DEFAULT_LATENCY_MS,
        }
    }

    /// Creates a mock provider with custom dimension.
    #[must_use]
    pub fn with_dimension(dimension: u32) -> Self {
        Self {
            model: format!("mock-embed-{dimension}"),
            dimension,
            latency_ms: Self::DEFAULT_LATENCY_MS,
        }
    }

    /// Creates a mock provider with custom model and dimension.
    #[must_use]
    pub fn with_model(model: impl Into<String>, dimension: u32) -> Self {
        Self {
            model: model.into(),
            dimension,
            latency_ms: Self::DEFAULT_LATENCY_MS,
        }
    }

    /// Sets the simulated latency.
    #[must_use]
    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = latency_ms;
        self
    }

    /// Generate a deterministic embedding for a given text input.
    ///
    /// Uses the hash of the input string to seed a deterministic vector.
    /// The vector is L2 normalized if requested.
    fn generate_embedding(&self, text: &str, normalize: bool) -> Vec<f32> {
        // Hash the input to get a deterministic seed
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        let seed = hasher.finish();

        // Generate deterministic vector based on seed
        let mut embedding = Vec::with_capacity(self.dimension as usize);
        let mut state = seed;

        for i in 0..self.dimension {
            // Simple LCG (Linear Congruential Generator) for deterministic values
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let value = ((state >> 16) as f32) / 32768.0 - 1.0; // Range: [-1, 1]

            // Add position-dependent variation
            let position_factor = (i as f32 / self.dimension as f32) * 0.1;
            embedding.push(value * (1.0 + position_factor));
        }

        // L2 normalize if requested
        if normalize {
            let magnitude = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            if magnitude > 0.0 {
                for value in &mut embedding {
                    *value /= magnitude;
                }
            }
        }

        embedding
    }

    /// Estimate token count (simple word count for mock).
    fn estimate_tokens(text: &str) -> usize {
        text.split_whitespace().count().max(1)
    }
}

impl Default for MockEmbeddingProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EmbeddingProvider for MockEmbeddingProvider {
    async fn embed_batch(&self, request: BatchEmbeddingRequest) -> EmbeddingResult<BatchEmbeddingResponse> {
        let start = Instant::now();

        // Validate inputs
        if request.inputs.is_empty() {
            return Err(EmbeddingError::InvalidInput("empty input batch".to_string()));
        }

        // Validate model matches
        if request.model != self.model {
            return Err(EmbeddingError::ModelNotFound(format!(
                "expected model '{}', got '{}'",
                self.model, request.model
            )));
        }

        // Simulate latency
        if self.latency_ms > 0 {
            tokio::time::sleep(Duration::from_millis(self.latency_ms)).await;
        }

        // Generate embeddings
        let embeddings: Vec<Vec<f32>> = request
            .inputs
            .iter()
            .map(|text| self.generate_embedding(text, request.normalize))
            .collect();

        // Calculate usage
        let total_tokens = request.inputs.iter().map(|text| Self::estimate_tokens(text)).sum();
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(BatchEmbeddingResponse {
            model: self.model.clone(),
            embeddings,
            usage: Usage {
                total_tokens,
                duration_ms,
            },
        })
    }

    async fn model_info(&self) -> EmbeddingResult<ModelInfo> {
        Ok(ModelInfo {
            model: self.model.clone(),
            dimension: self.dimension,
            max_tokens: 8192, // Mock max tokens
        })
    }

    async fn health_check(&self) -> EmbeddingResult<()> {
        // Mock provider is always healthy
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_provider_deterministic() {
        let provider = MockEmbeddingProvider::new();
        let request = BatchEmbeddingRequest {
            model: "mock-embed-512".to_string(),
            inputs: vec!["hello world".to_string()],
            normalize: false,
        };

        let response1 = provider.embed_batch(request.clone()).await.unwrap();
        let response2 = provider.embed_batch(request).await.unwrap();

        assert_eq!(response1.embeddings, response2.embeddings);
    }

    #[tokio::test]
    async fn test_mock_provider_dimension() {
        let provider = MockEmbeddingProvider::with_dimension(128);
        let request = BatchEmbeddingRequest {
            model: "mock-embed-128".to_string(),
            inputs: vec!["test".to_string()],
            normalize: false,
        };

        let response = provider.embed_batch(request).await.unwrap();
        assert_eq!(response.embeddings[0].len(), 128);
    }

    #[tokio::test]
    async fn test_mock_provider_normalize() {
        let provider = MockEmbeddingProvider::new();
        let request = BatchEmbeddingRequest {
            model: "mock-embed-512".to_string(),
            inputs: vec!["normalize me".to_string()],
            normalize: true,
        };

        let response = provider.embed_batch(request).await.unwrap();
        let embedding = &response.embeddings[0];

        // Check L2 norm is approximately 1.0
        let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 1e-5);
    }

    #[tokio::test]
    async fn test_mock_provider_health_check() {
        let provider = MockEmbeddingProvider::new();
        assert!(provider.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_mock_provider_model_info() {
        let provider = MockEmbeddingProvider::with_dimension(256);
        let info = provider.model_info().await.unwrap();
        assert_eq!(info.dimension, 256);
        assert_eq!(info.model, "mock-embed-256");
    }
}
