//! Candle-based embedding provider using pure Rust ML framework.
//!
//! This module provides GPU-accelerated embeddings without Python dependency.
//! Uses Hugging Face Candle for inference on Metal (macOS) or CUDA (Linux).
//!
//! # Example
//!
//! ```no_run
//! use akidb_embedding::CandleEmbeddingProvider;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let provider = CandleEmbeddingProvider::new(
//!         "sentence-transformers/all-MiniLM-L6-v2"
//!     ).await?;
//!
//!     println!("Candle provider initialized");
//!     Ok(())
//! }
//! ```

use async_trait::async_trait;
use std::sync::Arc;

use crate::provider::EmbeddingProvider;
use crate::types::{
    BatchEmbeddingRequest, BatchEmbeddingResponse, EmbeddingError, EmbeddingResult, ModelInfo,
    Usage,
};

// Re-exports from Candle (will be used in Day 2-3)
use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use hf_hub::{api::sync::Api, Repo, RepoType};
use tokenizers::Tokenizer;

/// Candle embedding provider for GPU-accelerated inference.
///
/// This provider uses pure Rust (no Python) for embedding generation.
/// Supports Metal GPU (macOS), CUDA GPU (Linux), and CPU fallback.
///
/// # Architecture
///
/// - **Model**: BERT-based transformer (e.g., MiniLM)
/// - **Device**: Metal > CUDA > CPU (automatic selection)
/// - **Threading**: Thread-safe via Arc (future: multi-threading in Phase 2)
///
/// # Performance
///
/// - Single text: <20ms (Metal GPU)
/// - Batch of 8: <40ms (Metal GPU)
/// - Batch of 32: <100ms (Metal GPU)
///
/// # Example
///
/// ```no_run
/// use akidb_embedding::CandleEmbeddingProvider;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let provider = CandleEmbeddingProvider::new(
///         "sentence-transformers/all-MiniLM-L6-v2"
///     ).await?;
///
///     // Provider is ready for inference
///     Ok(())
/// }
/// ```
pub struct CandleEmbeddingProvider {
    /// BERT model (thread-safe via Arc)
    ///
    /// Loaded once at initialization, reused for all requests.
    /// Arc enables future multi-threading (Phase 2).
    model: Arc<BertModel>,

    /// Tokenizer (thread-safe via Arc)
    ///
    /// Uses Hugging Face tokenizers (Rust bindings).
    /// Handles text to token ID conversion.
    tokenizer: Arc<Tokenizer>,

    /// Device (Metal, CUDA, or CPU)
    ///
    /// Selected once during initialization:
    /// 1. Try Metal (macOS)
    /// 2. Try CUDA (Linux)
    /// 3. Fallback to CPU
    device: Device,

    /// Model name from Hugging Face Hub
    ///
    /// Example: "sentence-transformers/all-MiniLM-L6-v2"
    model_name: String,

    /// Embedding dimension
    ///
    /// - MiniLM: 384
    /// - BERT-base: 768
    /// - BGE-small: 384
    dimension: u32,
}

impl CandleEmbeddingProvider {
    /// Create new Candle embedding provider.
    ///
    /// Downloads model from Hugging Face Hub (if not cached) and loads into GPU/CPU.
    ///
    /// # Arguments
    ///
    /// * `model_name` - Name of the model on Hugging Face Hub
    ///   Examples:
    ///   - "sentence-transformers/all-MiniLM-L6-v2" (384-dim, 22M params) - Recommended
    ///   - "sentence-transformers/all-distilroberta-v1" (768-dim, 82M params)
    ///   - "BAAI/bge-small-en-v1.5" (384-dim, 33M params)
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Model not found on Hugging Face Hub (404)
    /// - Model download fails (network error)
    /// - GPU/CPU initialization fails
    /// - Model weights corrupted
    ///
    /// # Performance
    ///
    /// - First call: 5-30s (download + load)
    /// - Cached: 1-2s (load only)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use akidb_embedding::CandleEmbeddingProvider;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let provider = CandleEmbeddingProvider::new(
    ///         "sentence-transformers/all-MiniLM-L6-v2"
    ///     ).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn new(model_name: &str) -> EmbeddingResult<Self> {
        // 1. Select device (Metal > CUDA > CPU)
        let device = Self::select_device()?;

        // 2. Download files from Hugging Face Hub
        let api = Api::new().map_err(|e| {
            EmbeddingError::Internal(format!("HF Hub API initialization failed: {}", e))
        })?;

        let repo = api.repo(Repo::new(model_name.to_string(), RepoType::Model));

        eprintln!("📥 Downloading {} from Hugging Face Hub...", model_name);

        let config_path = repo.get("config.json").map_err(|e| {
            EmbeddingError::Internal(format!("Failed to download config.json: {}", e))
        })?;

        let weights_path = repo
            .get("model.safetensors")
            .or_else(|_| {
                eprintln!("⚠️  model.safetensors not found, trying pytorch_model.bin");
                repo.get("pytorch_model.bin")
            })
            .map_err(|e| {
                EmbeddingError::Internal(format!("Failed to download model weights: {}", e))
            })?;

        let tokenizer_path = repo.get("tokenizer.json").map_err(|e| {
            EmbeddingError::Internal(format!("Failed to download tokenizer.json: {}", e))
        })?;

        eprintln!("✅ Files downloaded (cached at ~/.cache/huggingface)");

        // 3. Parse config.json
        let config_json = std::fs::read_to_string(&config_path).map_err(|e| {
            EmbeddingError::Internal(format!("Failed to read config.json: {}", e))
        })?;

        let config_value: serde_json::Value =
            serde_json::from_str(&config_json).map_err(|e| {
                EmbeddingError::Internal(format!("Failed to parse config.json: {}", e))
            })?;

        // Build BERT Config struct
        let config = Config {
            vocab_size: config_value["vocab_size"].as_u64().unwrap() as usize,
            hidden_size: config_value["hidden_size"].as_u64().unwrap() as usize,
            num_hidden_layers: config_value["num_hidden_layers"].as_u64().unwrap() as usize,
            num_attention_heads: config_value["num_attention_heads"].as_u64().unwrap() as usize,
            intermediate_size: config_value["intermediate_size"].as_u64().unwrap() as usize,
            hidden_act: serde_json::from_value(config_value["hidden_act"].clone())
                .unwrap_or(candle_transformers::models::bert::HiddenAct::Gelu),
            max_position_embeddings: config_value["max_position_embeddings"]
                .as_u64()
                .unwrap() as usize,
            type_vocab_size: config_value
                .get("type_vocab_size")
                .and_then(|v| v.as_u64())
                .unwrap_or(2) as usize,
            layer_norm_eps: config_value
                .get("layer_norm_eps")
                .and_then(|v| v.as_f64())
                .unwrap_or(1e-12),
            hidden_dropout_prob: config_value
                .get("hidden_dropout_prob")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.1),
            classifier_dropout: config_value
                .get("classifier_dropout")
                .and_then(|v| v.as_f64()),
            initializer_range: config_value
                .get("initializer_range")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.02),
            position_embedding_type: serde_json::from_value(
                config_value
                    .get("position_embedding_type")
                    .cloned()
                    .unwrap_or(serde_json::Value::String("absolute".to_string())),
            )
            .unwrap_or(candle_transformers::models::bert::PositionEmbeddingType::Absolute),
            use_cache: config_value
                .get("use_cache")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            model_type: config_value
                .get("model_type")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            pad_token_id: config_value
                .get("pad_token_id")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize,
        };

        let dimension = config.hidden_size as u32;

        // 4. Load model weights
        eprintln!("📦 Loading model weights into {:?}...", device);
        use candle_core::DType;

        let vb = if weights_path
            .extension()
            .and_then(|s| s.to_str())
            == Some("safetensors")
        {
            unsafe {
                VarBuilder::from_mmaped_safetensors(&[weights_path], DType::F32, &device)
                    .map_err(|e| {
                        EmbeddingError::Internal(format!(
                            "Failed to load SafeTensors: {}",
                            e
                        ))
                    })?
            }
        } else {
            VarBuilder::from_pth(&weights_path, DType::F32, &device).map_err(|e| {
                EmbeddingError::Internal(format!("Failed to load PyTorch weights: {}", e))
            })?
        };

        let model = BertModel::load(vb, &config).map_err(|e| {
            EmbeddingError::Internal(format!("Failed to load BertModel: {}", e))
        })?;

        let model = Arc::new(model);

        // 5. Load tokenizer
        eprintln!("📝 Loading tokenizer...");
        let tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(|e| {
            EmbeddingError::Internal(format!("Failed to load tokenizer: {}", e))
        })?;

        let tokenizer = Arc::new(tokenizer);

        // Quick tokenizer test
        if let Ok(encoding) = tokenizer.encode("test", true) {
            eprintln!("✅ Tokenizer test: {} tokens", encoding.len());
        }

        eprintln!("✅ CandleEmbeddingProvider initialized successfully");
        eprintln!("   Model: {}", model_name);
        eprintln!("   Device: {:?}", device);
        eprintln!("   Dimension: {}", dimension);

        Ok(Self {
            model,
            tokenizer,
            device,
            model_name: model_name.to_string(),
            dimension,
        })
    }

    /// Generate embeddings for batch of texts (internal implementation).
    ///
    /// This is the core inference method. Called by `embed_batch()` (trait method).
    ///
    /// # Arguments
    ///
    /// * `texts` - Vector of input texts
    ///
    /// # Returns
    ///
    /// Vector of embeddings, one per input text.
    /// Each embedding is a vector of f32 values (dimension determined by model).
    ///
    /// # Performance
    ///
    /// - Single text: <20ms (Metal GPU)
    /// - Batch of 8: <40ms (Metal GPU)
    /// - Batch of 32: <100ms (Metal GPU)
    pub async fn embed_batch_internal(
        &self,
        texts: Vec<String>,
    ) -> EmbeddingResult<Vec<Vec<f32>>> {
        use candle_core::Tensor;

        // Validate input
        if texts.is_empty() {
            return Err(EmbeddingError::InvalidInput("Empty input".to_string()));
        }

        // 1. Tokenization
        let encodings: Vec<_> = texts
            .iter()
            .map(|text| {
                self.tokenizer
                    .encode(text.as_str(), true)
                    .map_err(|e| EmbeddingError::Internal(format!("Tokenization: {}", e)))
            })
            .collect::<Result<Vec<_>, _>>()?;

        const MAX_LENGTH: usize = 512;
        let batch_size = texts.len();

        let mut token_ids_batch = Vec::new();
        let mut attention_masks_batch = Vec::new();

        for encoding in encodings {
            let mut ids = encoding.get_ids().to_vec();
            let mut mask = encoding.get_attention_mask().to_vec();

            // Pad or truncate to MAX_LENGTH
            if ids.len() > MAX_LENGTH {
                ids.truncate(MAX_LENGTH);
                mask.truncate(MAX_LENGTH);
            } else {
                ids.resize(MAX_LENGTH, 0);
                mask.resize(MAX_LENGTH, 0);
            }

            token_ids_batch.push(ids);
            attention_masks_batch.push(mask);
        }

        // 2. Convert to tensors
        let token_ids_flat: Vec<u32> = token_ids_batch
            .iter()
            .flat_map(|ids| ids.iter().copied())
            .collect();

        let attention_mask_flat: Vec<u32> = attention_masks_batch
            .iter()
            .flat_map(|mask| mask.iter().copied())
            .collect();

        let token_ids_tensor = Tensor::from_vec(token_ids_flat, &[batch_size, MAX_LENGTH], &self.device)
            .map_err(|e| EmbeddingError::Internal(format!("Token tensor: {}", e)))?;

        let attention_mask_tensor =
            Tensor::from_vec(attention_mask_flat, &[batch_size, MAX_LENGTH], &self.device)
                .map_err(|e| EmbeddingError::Internal(format!("Mask tensor: {}", e)))?;

        // 3. BERT forward pass
        // Create token_type_ids (all zeros for single-sentence tasks)
        let token_type_ids = Tensor::zeros(&[batch_size, MAX_LENGTH], candle_core::DType::U32, &self.device)
            .map_err(|e| EmbeddingError::Internal(format!("Token type IDs: {}", e)))?;

        // model.forward() returns embeddings: (batch_size, seq_len, hidden_size)
        // Third parameter is position_ids (None = use default positions)
        let embeddings = self
            .model
            .forward(&token_ids_tensor, &token_type_ids, None)
            .map_err(|e| EmbeddingError::Internal(format!("Forward pass: {}", e)))?;

        // 4. Mean pooling
        // Expand attention mask to (batch_size, seq_len, 1) then broadcast to (batch_size, seq_len, hidden_size)
        let attention_mask_expanded = attention_mask_tensor
            .unsqueeze(2)
            .map_err(|e| EmbeddingError::Internal(format!("Unsqueeze mask: {}", e)))?
            .to_dtype(candle_core::DType::F32)
            .map_err(|e| EmbeddingError::Internal(format!("Mask to F32: {}", e)))?
            .broadcast_as(embeddings.shape())
            .map_err(|e| EmbeddingError::Internal(format!("Broadcast mask: {}", e)))?;

        // Multiply embeddings by mask (zero out padding tokens)
        let masked_embeddings = embeddings
            .mul(&attention_mask_expanded)
            .map_err(|e| EmbeddingError::Internal(format!("Mask embeddings: {}", e)))?;

        // Sum over sequence length (axis 1)
        let sum_embeddings = masked_embeddings
            .sum(1)
            .map_err(|e| EmbeddingError::Internal(format!("Sum embeddings: {}", e)))?;

        // Sum attention mask over sequence length
        let sum_mask = attention_mask_expanded
            .sum(1)
            .map_err(|e| EmbeddingError::Internal(format!("Sum mask: {}", e)))?
            .clamp(1e-9, f32::MAX)
            .map_err(|e| EmbeddingError::Internal(format!("Clamp mask: {}", e)))?;

        // Divide to get mean
        let mean_pooled = sum_embeddings
            .div(&sum_mask)
            .map_err(|e| EmbeddingError::Internal(format!("Mean pooling: {}", e)))?;

        // 5. L2 normalization
        let squared = mean_pooled
            .sqr()
            .map_err(|e| EmbeddingError::Internal(format!("Square: {}", e)))?;

        let sum_squared = squared
            .sum(1)
            .map_err(|e| EmbeddingError::Internal(format!("Sum squared: {}", e)))?;

        let l2_norm = sum_squared
            .sqrt()
            .map_err(|e| EmbeddingError::Internal(format!("Sqrt: {}", e)))?
            .unsqueeze(1)
            .map_err(|e| EmbeddingError::Internal(format!("Unsqueeze norm: {}", e)))?
            .clamp(1e-12, f32::MAX)
            .map_err(|e| EmbeddingError::Internal(format!("Clamp norm: {}", e)))?
            .broadcast_as(mean_pooled.shape())
            .map_err(|e| EmbeddingError::Internal(format!("Broadcast norm: {}", e)))?;

        let normalized = mean_pooled
            .div(&l2_norm)
            .map_err(|e| EmbeddingError::Internal(format!("Normalize: {}", e)))?;

        // 6. Convert to Vec<Vec<f32>>
        let normalized_cpu = normalized
            .to_device(&Device::Cpu)
            .map_err(|e| EmbeddingError::Internal(format!("To CPU: {}", e)))?;

        // Convert 2D tensor (batch_size, hidden_size) to Vec<Vec<f32>>
        let embeddings: Vec<Vec<f32>> = normalized_cpu
            .to_vec2()
            .map_err(|e| EmbeddingError::Internal(format!("To vec: {}", e)))?;

        Ok(embeddings)
    }

    /// Select device (Metal > CUDA > CPU priority).
    ///
    /// # Device Selection Logic
    ///
    /// 1. macOS: Try Metal GPU first
    /// 2. Linux/Windows: Try CUDA GPU first
    /// 3. Fallback: CPU (always works)
    ///
    /// # Returns
    ///
    /// Selected device (never fails, CPU is fallback)
    fn select_device() -> EmbeddingResult<Device> {
        // TEMPORARY: Use CPU due to Metal layer-norm limitation in Candle
        // TODO: Re-enable Metal when candle-transformers supports it fully
        // See: https://github.com/huggingface/candle/issues

        // Try Metal on macOS (DISABLED due to layer-norm issue)
        #[cfg(target_os = "macos")]
        {
            // if let Ok(device) = Device::new_metal(0) {
            //     eprintln!("✅ Using Metal GPU (macOS)");
            //     return Ok(device);
            // }
            eprintln!("⚠️  Using CPU (Metal has limited layer-norm support)");
        }

        // Try CUDA on Linux/Windows
        #[cfg(not(target_os = "macos"))]
        {
            if let Ok(device) = Device::new_cuda(0) {
                eprintln!("✅ Using CUDA GPU");
                return Ok(device);
            }
        }

        // Fallback to CPU (currently required for macOS)
        #[cfg(target_os = "macos")]
        eprintln!("   Using CPU for BERT inference");

        #[cfg(not(target_os = "macos"))]
        eprintln!("⚠️  Using CPU (GPU unavailable)");

        Ok(Device::Cpu)
    }
}

// EmbeddingProvider trait implementation
#[async_trait]
impl EmbeddingProvider for CandleEmbeddingProvider {
    /// Generate embeddings for a batch of text inputs.
    ///
    /// This is the public API method. Internally calls `embed_batch_internal()`.
    ///
    /// # Arguments
    ///
    /// * `request` - Batch embedding request with model and inputs
    ///
    /// # Returns
    ///
    /// Batch embedding response with embeddings and usage statistics
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Input is empty
    /// - Model inference fails
    /// - GPU/CPU error
    async fn embed_batch(
        &self,
        request: BatchEmbeddingRequest,
    ) -> EmbeddingResult<BatchEmbeddingResponse> {
        use std::time::Instant;

        // 1. Validate input
        if request.inputs.is_empty() {
            return Err(EmbeddingError::InvalidInput(
                "Empty input list".to_string(),
            ));
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

        // 4. Calculate token count (approximate)
        // Rough estimate: ~0.75 tokens per word
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

    /// Get model information (dimension, capabilities).
    ///
    /// # Returns
    ///
    /// Model info with name, dimension, and max tokens
    async fn model_info(&self) -> EmbeddingResult<ModelInfo> {
        Ok(ModelInfo {
            model: self.model_name.clone(),
            dimension: self.dimension,
            max_tokens: 512, // BERT standard max sequence length
        })
    }

    /// Health check for the embedding service.
    ///
    /// Verifies that the provider can generate embeddings.
    ///
    /// # Returns
    ///
    /// Ok(()) if healthy, error otherwise
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
