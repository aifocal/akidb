// Python bridge embedding provider using ONNX Runtime with CoreML EP
//
// This provider wraps a Python subprocess running onnx_server.py which uses:
// - ONNX Runtime with CoreML Execution Provider
// - HuggingFace transformers for tokenization
//
// Performance: ~10ms P95 on Apple Silicon (validated Day 2)
// Protocol: JSON-RPC over stdin/stdout

use crate::provider::EmbeddingProvider;
use crate::types::{BatchEmbeddingRequest, BatchEmbeddingResponse, EmbeddingError, EmbeddingResult, ModelInfo, Usage};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Arc;
use tokio::sync::Mutex;

/// JSON-RPC request format
#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    method: String,
    params: serde_json::Value,
}

/// JSON-RPC response format
#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    status: String,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    embeddings: Option<Vec<Vec<f32>>>,
    #[serde(default)]
    dimension: Option<u32>,
    #[serde(default)]
    #[allow(dead_code)] // Reserved for future batch metrics
    count: Option<usize>,
}

/// Python bridge embedding provider
pub struct PythonBridgeProvider {
    /// Python subprocess handle
    process: Arc<Mutex<Child>>,
    /// Stdin pipe for sending requests
    stdin: Arc<Mutex<ChildStdin>>,
    /// Stdout pipe for receiving responses
    stdout: Arc<Mutex<BufReader<ChildStdout>>>,
    /// Embedding dimension (cached after first load)
    dimension: Arc<Mutex<Option<u32>>>,
}

impl PythonBridgeProvider {
    /// Create a new Python bridge provider
    ///
    /// # Arguments
    /// * `model_name` - HuggingFace model name (e.g., "sentence-transformers/all-MiniLM-L6-v2")
    /// * `python_path` - Optional path to Python executable (defaults to "python3")
    ///
    /// # Returns
    /// Initialized provider with Python subprocess running
    pub async fn new(model_name: &str, python_path: Option<&str>) -> EmbeddingResult<Self> {
        let python_exe = python_path.unwrap_or("python3");

        // Find onnx_server.py (relative to this crate)
        let server_script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("python")
            .join("onnx_server.py");

        if !server_script.exists() {
            return Err(EmbeddingError::ModelNotFound(format!(
                "Python server script not found: {}",
                server_script.display()
            )));
        }

        // Spawn Python subprocess
        let mut process = Command::new(python_exe)
            .arg(server_script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()) // Show Python logs in our stderr
            .spawn()
            .map_err(|e| {
                EmbeddingError::Internal(format!("Failed to spawn Python process: {}", e))
            })?;

        // Get stdin/stdout handles
        let stdin = process.stdin.take().ok_or_else(|| {
            EmbeddingError::Internal("Failed to get stdin handle".to_string())
        })?;

        let stdout = process.stdout.take().ok_or_else(|| {
            EmbeddingError::Internal("Failed to get stdout handle".to_string())
        })?;

        let mut provider = Self {
            process: Arc::new(Mutex::new(process)),
            stdin: Arc::new(Mutex::new(stdin)),
            stdout: Arc::new(Mutex::new(BufReader::new(stdout))),
            dimension: Arc::new(Mutex::new(None)),
        };

        // Test connection with ping
        provider.ping().await?;

        // Load the model
        provider.load_model(model_name).await?;

        // Warmup: perform a test embedding to load the model into memory
        // This eliminates first-request slowness in production
        provider.warmup(model_name).await?;

        Ok(provider)
    }

    /// Warmup the model by performing a test embedding
    ///
    /// This loads the model into memory and initializes all acceleration paths (MPS/CoreML).
    /// Eliminates the 300-400ms first-request penalty seen in benchmarks.
    async fn warmup(&self, model_name: &str) -> EmbeddingResult<()> {
        use crate::BatchEmbeddingRequest;

        let warmup_req = BatchEmbeddingRequest {
            model: model_name.to_string(),
            inputs: vec!["warmup".to_string()],
            normalize: true,
        };

        // Perform warmup embedding (result discarded)
        self.embed_batch(warmup_req).await?;

        Ok(())
    }

    /// Send a ping request to check if server is alive
    async fn ping(&mut self) -> EmbeddingResult<()> {
        let request = JsonRpcRequest {
            method: "ping".to_string(),
            params: serde_json::json!({}),
        };

        let response = self.send_request(&request).await?;

        if response.status != "ok" {
            return Err(EmbeddingError::Internal(format!(
                "Ping failed: {:?}",
                response.message
            )));
        }

        Ok(())
    }

    /// Load a model in the Python subprocess
    async fn load_model(&mut self, model_name: &str) -> EmbeddingResult<()> {
        let request = JsonRpcRequest {
            method: "load_model".to_string(),
            params: serde_json::json!({
                "model": model_name,
                "cache_dir": "~/.cache/akidb/models"
            }),
        };

        let response = self.send_request(&request).await?;

        if response.status != "ok" {
            return Err(EmbeddingError::ModelNotFound(format!(
                "Failed to load model: {:?}",
                response.message
            )));
        }

        // Cache dimension
        if let Some(dim) = response.dimension {
            *self.dimension.lock().await = Some(dim);
        }

        Ok(())
    }

    /// Send a JSON-RPC request and receive response
    ///
    /// Thread-safe: acquires locks on stdin/stdout
    async fn send_request(&self, request: &JsonRpcRequest) -> EmbeddingResult<JsonRpcResponse> {
        // Serialize request to JSON (single line)
        let mut json = serde_json::to_string(request).map_err(|e| {
            EmbeddingError::Internal(format!("Failed to serialize request: {}", e))
        })?;
        json.push('\n');

        // Send request
        {
            let mut stdin = self.stdin.lock().await;
            stdin.write_all(json.as_bytes()).map_err(|e| {
                EmbeddingError::Internal(format!("Failed to write to Python subprocess: {}", e))
            })?;
            stdin.flush().map_err(|e| {
                EmbeddingError::Internal(format!("Failed to flush stdin: {}", e))
            })?;
        }

        // Read response
        let response_line = {
            let mut stdout = self.stdout.lock().await;
            let mut line = String::new();
            stdout.read_line(&mut line).map_err(|e| {
                EmbeddingError::Internal(format!("Failed to read from Python subprocess: {}", e))
            })?;
            line
        };

        // Parse response
        let response: JsonRpcResponse = serde_json::from_str(&response_line).map_err(|e| {
            EmbeddingError::Internal(format!("Failed to parse response: {}", e))
        })?;

        Ok(response)
    }
}

#[async_trait]
impl EmbeddingProvider for PythonBridgeProvider {
    async fn embed_batch(
        &self,
        request: BatchEmbeddingRequest,
    ) -> EmbeddingResult<BatchEmbeddingResponse> {
        let json_request = JsonRpcRequest {
            method: "embed_batch".to_string(),
            params: serde_json::json!({
                "model": request.model,
                "inputs": request.inputs,
                "normalize": request.normalize
            }),
        };

        let response = self.send_request(&json_request).await?;

        if response.status != "ok" {
            return Err(EmbeddingError::Internal(format!(
                "Embedding failed: {:?}",
                response.message
            )));
        }

        let embeddings = response.embeddings.ok_or_else(|| {
            EmbeddingError::Internal("Missing embeddings in response".to_string())
        })?;

        // Calculate usage
        let num_inputs = request.inputs.len();
        let total_tokens = num_inputs * 512; // Approximate (max_length)

        Ok(BatchEmbeddingResponse {
            embeddings,
            model: request.model,
            usage: Usage {
                total_tokens,
                duration_ms: 0, // TODO: measure actual duration
            },
        })
    }

    async fn model_info(&self) -> EmbeddingResult<ModelInfo> {
        let dimension = self.dimension
            .lock()
            .await
            .ok_or_else(|| EmbeddingError::Internal("Model not loaded".to_string()))?;

        Ok(ModelInfo {
            model: "python-bridge-onnx-coreml".to_string(),
            dimension,
            max_tokens: 512,
        })
    }

    async fn health_check(&self) -> EmbeddingResult<()> {
        // Check if dimension is initialized (model loaded)
        if self.dimension.lock().await.is_none() {
            return Err(EmbeddingError::ServiceUnavailable(
                "Model not loaded".to_string()
            ));
        }

        // Ping the Python subprocess
        let request = JsonRpcRequest {
            method: "ping".to_string(),
            params: serde_json::json!({}),
        };

        let response = self.send_request(&request).await?;

        if response.status != "ok" {
            return Err(EmbeddingError::ServiceUnavailable(format!(
                "Health check failed: {:?}",
                response.message
            )));
        }

        Ok(())
    }
}

impl Drop for PythonBridgeProvider {
    fn drop(&mut self) {
        // Kill Python subprocess on drop
        if let Ok(mut process) = self.process.try_lock() {
            let _ = process.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_python_bridge_basic() {
        // This test requires:
        // 1. Python 3 installed
        // 2. pip install onnxruntime transformers
        // 3. ONNX model converted and available

        // Skip if Python dependencies not available
        let provider = PythonBridgeProvider::new(
            "sentence-transformers/all-MiniLM-L6-v2",
            None,
        )
        .await;

        if provider.is_err() {
            println!("Skipping test: Python bridge not available");
            return;
        }

        let provider = provider.unwrap();

        let request = BatchEmbeddingRequest {
            model: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
            inputs: vec!["Hello, world!".to_string()],
            normalize: true,
        };

        let response = provider.embed_batch(request).await.unwrap();

        assert_eq!(response.embeddings.len(), 1);
        assert_eq!(response.embeddings[0].len(), 384);

        // Check L2 normalization
        let embedding = &response.embeddings[0];
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01, "Embedding should be normalized");
    }
}
