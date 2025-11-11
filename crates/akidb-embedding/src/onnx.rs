//! ONNX Runtime embedding provider.
//!
//! Provides universal GPU support (Metal, CUDA, DirectML) for text embedding generation
//! using ONNX Runtime with BERT-based transformer models.

use crate::{
    BatchEmbeddingRequest, BatchEmbeddingResponse, EmbeddingError, EmbeddingProvider,
    EmbeddingResult, ModelInfo, Usage,
};
use async_trait::async_trait;
use ndarray::Array2;
use ort::{session::Session, GraphOptimizationLevel};
use std::sync::Arc;
use tokenizers::Tokenizer;

/// ONNX Runtime embedding provider.
///
/// Uses ONNX Runtime for universal GPU support (Metal, CUDA, DirectML) with
/// BERT-based transformer models for text embedding generation.
pub struct OnnxEmbeddingProvider {
    /// ONNX Runtime session (contains model)
    session: Arc<Session>,

    /// Tokenizer for text preprocessing
    tokenizer: Tokenizer,

    /// Model name (for metadata)
    model_name: String,

    /// Embedding dimension
    dimension: u32,
}

impl OnnxEmbeddingProvider {
    /// Create new ONNX embedding provider.
    ///
    /// # Arguments
    ///
    /// * `model_path` - Path to ONNX model file (.onnx)
    /// * `model_name` - Model name (e.g., "sentence-transformers/all-MiniLM-L6-v2")
    pub async fn new(model_path: &str, model_name: &str) -> EmbeddingResult<Self> {
        eprintln!("\n🔧 Initializing ONNX Runtime provider...");

        // 1. Create session with model
        eprintln!("📦 Loading ONNX model from: {}", model_path);

        let session = Session::builder()
            .map_err(|e| EmbeddingError::Internal(format!("Failed to create session builder: {}", e)))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| EmbeddingError::Internal(format!("Failed to set optimization level: {}", e)))?
            .with_intra_threads(4)
            .map_err(|e| EmbeddingError::Internal(format!("Failed to set threads: {}", e)))?
            .commit_from_file(model_path)
            .map_err(|e| EmbeddingError::Internal(format!("Failed to load model: {}", e)))?;

        eprintln!("✅ ONNX model loaded successfully");

        // 2. Load tokenizer from Hugging Face Hub or cache
        eprintln!("📝 Loading tokenizer for: {}", model_name);
        let tokenizer = Self::load_tokenizer(model_name).await?;
        eprintln!("✅ Tokenizer loaded successfully");

        // 3. Determine dimension from model output metadata
        let dimension = Self::get_model_dimension(&session)?;

        eprintln!(
            "✅ OnnxEmbeddingProvider initialized\n   Model: {}\n   Dimension: {}",
            model_name, dimension
        );

        Ok(Self {
            session: Arc::new(session),
            tokenizer,
            model_name: model_name.to_string(),
            dimension,
        })
    }

    /// Load tokenizer from Hugging Face Hub or cache.
    async fn load_tokenizer(model_name: &str) -> EmbeddingResult<Tokenizer> {
        use hf_hub::api::tokio::Api;

        let api = Api::new()
            .map_err(|e| EmbeddingError::Internal(format!("Failed to create HF Hub API: {}", e)))?;

        let repo = api.model(model_name.to_string());

        let tokenizer_path = repo
            .get("tokenizer.json")
            .await
            .map_err(|e| EmbeddingError::Internal(format!("Failed to download tokenizer: {}", e)))?;

        Tokenizer::from_file(tokenizer_path)
            .map_err(|e| EmbeddingError::Internal(format!("Failed to load tokenizer: {}", e)))
    }

    /// Get model output dimension from ONNX metadata.
    fn get_model_dimension(session: &Session) -> EmbeddingResult<u32> {
        // Get output metadata (last_hidden_state shape: [batch_size, seq_len, hidden_size])
        let outputs = session.outputs.clone();

        if outputs.is_empty() {
            return Err(EmbeddingError::Internal("Model has no outputs".to_string()));
        }

        // First output should be last_hidden_state with shape [batch, seq, hidden]
        let output_shape = &outputs[0].dimensions;

        if output_shape.len() < 3 {
            return Err(EmbeddingError::Internal(format!(
                "Invalid output shape: expected 3 dimensions, got {}",
                output_shape.len()
            )));
        }

        // Last dimension is hidden_size (embedding dimension)
        let dimension = output_shape[2];

        match dimension {
            Some(dim) if dim > 0 => Ok(dim as u32),
            _ => Err(EmbeddingError::Internal(
                "Could not determine embedding dimension from model".to_string(),
            )),
        }
    }

    /// Generate embeddings (internal implementation).
    ///
    /// Performs:
    /// 1. Tokenization with padding/truncation
    /// 2. ONNX Runtime inference
    /// 3. Mean pooling with attention mask
    /// 4. L2 normalization
    pub async fn embed_batch_internal(
        &self,
        texts: Vec<String>,
    ) -> EmbeddingResult<Vec<Vec<f32>>> {
        // 1. Validate input
        if texts.is_empty() {
            return Err(EmbeddingError::InvalidInput("Empty input list".to_string()));
        }

        // 2. Tokenize inputs
        let encodings: Vec<_> = texts
            .iter()
            .map(|text| self.tokenizer.encode(text.as_str(), true))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| EmbeddingError::Internal(format!("Tokenization failed: {}", e)))?;

        const MAX_LENGTH: usize = 512;
        let batch_size = texts.len();

        // 3. Pad/truncate to fixed length
        let mut input_ids_vec = Vec::with_capacity(batch_size * MAX_LENGTH);
        let mut attention_mask_vec = Vec::with_capacity(batch_size * MAX_LENGTH);

        for encoding in encodings {
            let mut ids = encoding.get_ids().to_vec();
            let mut mask = encoding.get_attention_mask().to_vec();

            if ids.len() > MAX_LENGTH {
                ids.truncate(MAX_LENGTH);
                mask.truncate(MAX_LENGTH);
            } else {
                ids.resize(MAX_LENGTH, 0);
                mask.resize(MAX_LENGTH, 0);
            }

            input_ids_vec.extend(ids.iter().map(|&x| x as i64));
            attention_mask_vec.extend(mask.iter().map(|&x| x as i64));
        }

        // 4. Create input tensors
        let input_ids_shape = vec![batch_size, MAX_LENGTH];
        let input_ids_array = Array2::from_shape_vec(
            (batch_size, MAX_LENGTH),
            input_ids_vec,
        )
        .map_err(|e| EmbeddingError::Internal(format!("Failed to create input tensor: {}", e)))?;

        let attention_mask_array = Array2::from_shape_vec(
            (batch_size, MAX_LENGTH),
            attention_mask_vec.clone(),
        )
        .map_err(|e| EmbeddingError::Internal(format!("Failed to create mask tensor: {}", e)))?;

        // 5. Run ONNX inference
        let outputs = self
            .session
            .run(vec![
                ort::Value::from_array(input_ids_array.view())
                    .map_err(|e| EmbeddingError::Internal(format!("Failed to create input tensor: {}", e)))?,
                ort::Value::from_array(attention_mask_array.view())
                    .map_err(|e| EmbeddingError::Internal(format!("Failed to create mask tensor: {}", e)))?,
            ])
            .map_err(|e| EmbeddingError::Internal(format!("ONNX inference failed: {}", e)))?;

        // 6. Extract last_hidden_state output
        let last_hidden_state = outputs[0]
            .try_extract_raw_tensor::<f32>()
            .map_err(|e| EmbeddingError::Internal(format!("Failed to extract output: {}", e)))?;

        let hidden_data = last_hidden_state.view();
        let hidden_shape = hidden_data.shape();

        // Shape should be [batch_size, seq_len, hidden_size]
        if hidden_shape.len() != 3 {
            return Err(EmbeddingError::Internal(format!(
                "Unexpected output shape: {:?}",
                hidden_shape
            )));
        }

        let hidden_size = hidden_shape[2];

        // 7. Mean pooling with attention mask
        let mut embeddings = Vec::with_capacity(batch_size);

        for i in 0..batch_size {
            let mut pooled = vec![0.0f32; hidden_size];
            let mut sum_mask = 0.0f32;

            for j in 0..MAX_LENGTH {
                let mask_val = attention_mask_vec[i * MAX_LENGTH + j] as f32;
                sum_mask += mask_val;

                for k in 0..hidden_size {
                    let hidden_val = hidden_data[[i, j, k]];
                    pooled[k] += hidden_val * mask_val;
                }
            }

            // Divide by sum of mask
            if sum_mask > 0.0 {
                for val in &mut pooled {
                    *val /= sum_mask;
                }
            }

            // 8. L2 normalization
            let norm: f32 = pooled.iter().map(|x| x * x).sum::<f32>().sqrt();
            let norm = norm.max(1e-12); // Prevent division by zero

            for val in &mut pooled {
                *val /= norm;
            }

            embeddings.push(pooled);
        }

        Ok(embeddings)
    }
}

#[async_trait]
impl EmbeddingProvider for OnnxEmbeddingProvider {
    async fn embed_batch(
        &self,
        request: BatchEmbeddingRequest,
    ) -> EmbeddingResult<BatchEmbeddingResponse> {
        use std::time::Instant;

        // 1. Validate input
        if request.inputs.is_empty() {
            return Err(EmbeddingError::InvalidInput("Empty input list".to_string()));
        }

        if request.inputs.len() > 32 {
            return Err(EmbeddingError::InvalidInput(format!(
                "Batch size {} exceeds maximum of 32",
                request.inputs.len()
            )));
        }

        // Check for empty strings
        for (i, input) in request.inputs.iter().enumerate() {
            if input.trim().is_empty() {
                return Err(EmbeddingError::InvalidInput(format!(
                    "Input at index {} is empty or whitespace",
                    i
                )));
            }
        }

        // 2. Measure duration
        let start = Instant::now();

        // 3. Generate embeddings
        let embeddings = self.embed_batch_internal(request.inputs.clone()).await?;

        let duration_ms = start.elapsed().as_millis() as u64;

        // 4. Calculate token count (approximate: 0.75 tokens per word)
        let total_tokens: usize = request
            .inputs
            .iter()
            .map(|text| {
                let words = text.split_whitespace().count();
                ((words as f32) * 0.75) as usize
            })
            .sum();

        // 5. Build response
        Ok(BatchEmbeddingResponse {
            model: request.model,
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
            max_tokens: 512, // BERT standard max sequence length
        })
    }

    async fn health_check(&self) -> EmbeddingResult<()> {
        // Generate a test embedding to verify the provider is functional
        let test_embedding = self
            .embed_batch_internal(vec!["health check".to_string()])
            .await?;

        // Verify output is not empty
        if test_embedding.is_empty() {
            return Err(EmbeddingError::ServiceUnavailable(
                "Health check failed: no embeddings generated".to_string(),
            ));
        }

        // Verify correct dimension
        if test_embedding[0].len() != self.dimension as usize {
            return Err(EmbeddingError::ServiceUnavailable(format!(
                "Health check failed: wrong dimension (expected {}, got {})",
                self.dimension,
                test_embedding[0].len()
            )));
        }

        // Verify L2 normalized (norm should be approximately 1.0)
        let norm: f32 = test_embedding[0]
            .iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt();
        if (norm - 1.0).abs() > 0.1 {
            return Err(EmbeddingError::ServiceUnavailable(format!(
                "Health check failed: embeddings not normalized (norm={})",
                norm
            )));
        }

        Ok(())
    }
}
