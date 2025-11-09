//! MLX-powered embedding provider using PyO3 bridge to Python.
//!
//! This module provides Apple Silicon-accelerated embeddings through the MLX framework.
//! The Rust side uses PyO3 to call Python code that runs MLX inference.

use async_trait::async_trait;
use parking_lot::Mutex;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::path::PathBuf;
use std::sync::Arc;

use crate::provider::EmbeddingProvider;
use crate::types::{
    BatchEmbeddingRequest, BatchEmbeddingResponse, EmbeddingError, EmbeddingResult, ModelInfo,
    Usage,
};

/// MLX embedding provider powered by Apple Silicon.
///
/// This provider uses PyO3 to bridge to Python code that runs MLX inference.
/// Thread-safe and async-compatible through interior mutability.
///
/// **Concurrency Note**: MLX inference is inherently single-threaded due to Python's GIL.
/// Concurrent requests will receive ServiceUnavailable errors (HTTP 503).
/// This is expected behavior for on-device inference - clients should retry with backoff.
pub struct MlxEmbeddingProvider {
    /// Python EmbeddingService instance (wrapped in Arc<Mutex> for thread safety)
    py_service: Arc<Mutex<Py<PyAny>>>,
    /// Model name (e.g., "qwen3-0.6b-4bit")
    model_name: String,
    /// Output dimension
    dimension: u32,
}

impl MlxEmbeddingProvider {
    /// Create a new MLX embedding provider.
    ///
    /// # Arguments
    ///
    /// * `model_name` - Name of the embedding model (e.g., "qwen3-0.6b-4bit")
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Python interpreter initialization fails
    /// - Python module import fails
    /// - Model initialization fails
    pub fn new(model_name: &str) -> EmbeddingResult<Self> {
        // Initialize Python interpreter (auto-initialize feature handles this)
        Python::with_gil(|py| {
            // Add Python module directory to sys.path
            let sys = py
                .import_bound("sys")
                .map_err(|e| EmbeddingError::Internal(format!("Failed to import sys: {e}")))?;
            let path = sys
                .getattr("path")
                .map_err(|e| EmbeddingError::Internal(format!("Failed to get sys.path: {e}")))?;
            let path: &pyo3::Bound<'_, PyList> = path
                .downcast()
                .map_err(|e| EmbeddingError::Internal(format!("Failed to downcast sys.path: {e}")))?;

            // Find the Python module directory (relative to this crate)
            let module_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("python");
            path.insert(0, module_path.to_str().ok_or_else(|| {
                EmbeddingError::Internal("Invalid module path".to_string())
            })?)
            .map_err(|e| {
                EmbeddingError::Internal(format!("Failed to add module to sys.path: {e}"))
            })?;

            // Import the akidb_mlx module
            let akidb_mlx = py.import_bound("akidb_mlx").map_err(|e| {
                EmbeddingError::Internal(format!(
                    "Failed to import akidb_mlx module: {e}\nMake sure Python dependencies are installed: pip install -r python/requirements.txt"
                ))
            })?;

            // Create EmbeddingService instance
            let service_class = akidb_mlx.getattr("EmbeddingService").map_err(|e| {
                EmbeddingError::Internal(format!("Failed to get EmbeddingService class: {e}"))
            })?;

            let service = service_class.call1((model_name,)).map_err(|e| {
                EmbeddingError::Internal(format!("Failed to create EmbeddingService: {e}"))
            })?;

            // Get model info to determine dimension
            let model_info = service.call_method0("get_model_info").map_err(|e| {
                EmbeddingError::Internal(format!("Failed to get model info: {e}"))
            })?;

            let info_dict = model_info.downcast::<PyDict>().map_err(|e| {
                EmbeddingError::Internal(format!("Invalid model info format: {e}"))
            })?;

            let dimension: u32 = info_dict
                .get_item("dimension")
                .and_then(|opt| opt.ok_or_else(|| pyo3::PyErr::new::<pyo3::exceptions::PyKeyError, _>("dimension not found")))
                .and_then(|dim| dim.extract::<u32>())
                .map_err(|e| {
                    EmbeddingError::Internal(format!("Failed to get dimension from model info: {e}"))
                })?;

            println!("[MlxEmbeddingProvider] Initialized with model: {model_name}, dimension: {dimension}");

            Ok(Self {
                py_service: Arc::new(Mutex::new(service.into())),
                model_name: model_name.to_string(),
                dimension,
            })
        })
    }

    /// Call the Python embedding service to generate embeddings.
    ///
    /// Uses `try_lock()` to ensure only one Python call at a time due to GIL constraints.
    /// Returns ServiceUnavailable if another request is currently processing.
    fn call_python_embed(&self, texts: Vec<String>) -> EmbeddingResult<Vec<Vec<f32>>> {
        Python::with_gil(|py| {
            // Try to acquire lock - fails fast if another request is processing
            let service = self.py_service.try_lock().ok_or_else(|| {
                EmbeddingError::ServiceUnavailable(
                    "Embedding model is currently processing another request. Please retry with exponential backoff.".to_string()
                )
            })?;

            // Call the embed method
            let result = service
                .bind(py)
                .call_method1("embed", (texts,))
                .map_err(|e| EmbeddingError::Internal(format!("Python embed() failed: {e}")))?;

            // Extract the embeddings (list of lists of floats)
            let embeddings: Vec<Vec<f32>> = result.extract().map_err(|e| {
                EmbeddingError::Internal(format!("Failed to extract embeddings: {e}"))
            })?;

            Ok(embeddings)
        })
    }
}

#[async_trait]
impl EmbeddingProvider for MlxEmbeddingProvider {
    async fn embed_batch(
        &self,
        request: BatchEmbeddingRequest,
    ) -> EmbeddingResult<BatchEmbeddingResponse> {
        // Validate inputs
        if request.inputs.is_empty() {
            return Err(EmbeddingError::InvalidInput(
                "Inputs cannot be empty".to_string(),
            ));
        }

        // Record start time
        let start = std::time::Instant::now();

        // Call Python (blocking, so run in blocking thread pool)
        // Uses try_lock() inside to fail fast if model is busy
        let texts = request.inputs.clone();
        let embeddings = tokio::task::spawn_blocking({
            let provider = self.clone_for_blocking();
            move || provider.call_python_embed(texts)
        })
        .await
        .map_err(|e| EmbeddingError::Internal(format!("Task join error: {e}")))??;

        // Calculate duration
        let duration_ms = start.elapsed().as_millis() as u64;

        // Estimate token count (rough approximation: 1 token ~= 4 characters)
        let total_tokens: usize = request
            .inputs
            .iter()
            .map(|s| s.len() / 4)
            .sum::<usize>()
            .max(request.inputs.len()); // At least 1 token per input

        Ok(BatchEmbeddingResponse {
            model: self.model_name.clone(),
            embeddings,
            usage: Usage {
                total_tokens,
                duration_ms,
            },
        })
    }

    async fn model_info(&self) -> EmbeddingResult<ModelInfo> {
        Ok(ModelInfo {
            model: self.model_name.clone(),
            dimension: self.dimension,
            max_tokens: 512, // Default for now, will be configurable later
        })
    }

    async fn health_check(&self) -> EmbeddingResult<()> {
        // Try a simple Python call to verify the service is working
        // Uses blocking lock since health checks are not in hot path
        tokio::task::spawn_blocking({
            let provider = self.clone_for_blocking();
            move || {
                Python::with_gil(|py| {
                    let service = provider.py_service.lock();

                    service
                        .bind(py)
                        .call_method0("get_model_info")
                        .map_err(|e| {
                            EmbeddingError::ServiceUnavailable(format!("Health check failed: {e}"))
                        })?;

                    Ok(())
                })
            }
        })
        .await
        .map_err(|e| EmbeddingError::Internal(format!("Task join error: {e}")))?
    }
}

impl MlxEmbeddingProvider {
    /// Helper method to clone data needed for blocking tasks.
    fn clone_for_blocking(&self) -> Self {
        Self {
            py_service: Arc::clone(&self.py_service),
            model_name: self.model_name.clone(),
            dimension: self.dimension,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mlx_provider_initialization() {
        // This test requires Python dependencies to be installed
        // Skip if not available
        let result = MlxEmbeddingProvider::new("qwen3-0.6b-4bit");

        // Just verify it doesn't panic; actual functionality tested in integration tests
        if let Ok(provider) = result {
            assert_eq!(provider.model_name, "qwen3-0.6b-4bit");
            assert_eq!(provider.dimension, 1024); // Qwen3-0.6B has 1024-dim embeddings
        }
    }

    #[tokio::test]
    async fn test_mlx_provider_model_info() {
        let provider = match MlxEmbeddingProvider::new("qwen3-0.6b-4bit") {
            Ok(p) => p,
            Err(_) => {
                println!("Skipping test: Python environment not available");
                return;
            }
        };

        let info = provider.model_info().await.unwrap();
        assert_eq!(info.model, "qwen3-0.6b-4bit");
        assert_eq!(info.dimension, 1024); // Qwen3-0.6B has 1024-dim embeddings
    }

    #[tokio::test]
    async fn test_mlx_provider_health_check() {
        let provider = match MlxEmbeddingProvider::new("qwen3-0.6b-4bit") {
            Ok(p) => p,
            Err(_) => {
                println!("Skipping test: Python environment not available");
                return;
            }
        };

        assert!(provider.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_mlx_provider_embed_batch() {
        let provider = match MlxEmbeddingProvider::new("qwen3-0.6b-4bit") {
            Ok(p) => p,
            Err(_) => {
                println!("Skipping test: Python environment not available");
                return;
            }
        };

        let request = BatchEmbeddingRequest {
            model: "qwen3-0.6b-4bit".to_string(),
            inputs: vec!["hello world".to_string(), "test embedding".to_string()],
            normalize: true,
        };

        let response = provider.embed_batch(request).await.unwrap();

        assert_eq!(response.model, "qwen3-0.6b-4bit");
        assert_eq!(response.embeddings.len(), 2);
        assert_eq!(response.embeddings[0].len(), 1024); // Qwen3-0.6B has 1024-dim embeddings
        assert_eq!(response.embeddings[1].len(), 1024);
    }
}
