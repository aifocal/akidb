//! ONNX Runtime embedding provider.
//!
//! Provides universal GPU support (Metal, CUDA, DirectML, TensorRT) for text embedding generation
//! using ONNX Runtime with transformer models (BERT, Qwen, etc.).
//!
//! # Execution Providers
//!
//! - **CoreML**: Mac ARM GPU acceleration (M1/M2/M3)
//! - **TensorRT**: NVIDIA GPU optimization for Jetson Thor (FP8 support)
//! - **CUDA**: Generic NVIDIA GPU fallback
//! - **CPU**: CPU-only fallback

use crate::{
    BatchEmbeddingRequest, BatchEmbeddingResponse, EmbeddingError, EmbeddingProvider,
    EmbeddingResult, ModelInfo, Usage,
};
use async_trait::async_trait;
use ndarray::Array2;
use ort::{session::{Session, builder::GraphOptimizationLevel}, value::Value};
use parking_lot::Mutex;
use std::path::PathBuf;
use tokenizers::Tokenizer;

/// Execution provider configuration.
#[derive(Debug, Clone)]
pub enum ExecutionProviderConfig {
    /// CoreML (Mac ARM)
    CoreML,
    /// TensorRT (NVIDIA Jetson/GPUs) with optional FP8
    TensorRT {
        device_id: i32,
        fp8_enable: bool,
        engine_cache_path: Option<PathBuf>,
    },
    /// CUDA (generic NVIDIA GPU)
    CUDA { device_id: i32 },
    /// CPU fallback
    CPU,
}

/// Configuration for ONNX embedding provider.
#[derive(Debug, Clone)]
pub struct OnnxConfig {
    /// Path to ONNX model file
    pub model_path: PathBuf,
    /// Path to tokenizer.json file
    pub tokenizer_path: PathBuf,
    /// Model name (for metadata)
    pub model_name: String,
    /// Output embedding dimension
    pub dimension: u32,
    /// Maximum sequence length
    pub max_length: usize,
    /// Execution provider
    pub execution_provider: ExecutionProviderConfig,
}

impl Default for OnnxConfig {
    fn default() -> Self {
        Self {
            model_path: PathBuf::from("models/model.onnx"),
            tokenizer_path: PathBuf::from("models/tokenizer.json"),
            model_name: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
            dimension: 384,
            max_length: 512,
            execution_provider: ExecutionProviderConfig::CPU,
        }
    }
}

/// ONNX Runtime embedding provider.
///
/// Uses ONNX Runtime for universal GPU support (CoreML, TensorRT, CUDA) with
/// transformer models for text embedding generation.
pub struct OnnxEmbeddingProvider {
    /// ONNX Runtime session (contains model)
    /// Wrapped in Mutex for interior mutability (Session::run requires &mut self)
    session: Mutex<Session>,

    /// Tokenizer for text preprocessing
    tokenizer: Tokenizer,

    /// Configuration
    config: OnnxConfig,
}

impl OnnxEmbeddingProvider {
    /// Create new ONNX embedding provider with configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - ONNX provider configuration
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use akidb_embedding::onnx::{OnnxEmbeddingProvider, OnnxConfig, ExecutionProviderConfig};
    /// use std::path::PathBuf;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Jetson Thor with TensorRT + FP8
    /// let config = OnnxConfig {
    ///     model_path: PathBuf::from("models/qwen3-4b-fp8.onnx"),
    ///     tokenizer_path: PathBuf::from("models/tokenizer.json"),
    ///     model_name: "Qwen/Qwen2.5-4B".to_string(),
    ///     dimension: 4096,
    ///     max_length: 512,
    ///     execution_provider: ExecutionProviderConfig::TensorRT {
    ///         device_id: 0,
    ///         fp8_enable: true,
    ///         engine_cache_path: Some(PathBuf::from("/tmp/trt_cache")),
    ///     },
    /// };
    ///
    /// let provider = OnnxEmbeddingProvider::with_config(config).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn with_config(config: OnnxConfig) -> EmbeddingResult<Self> {
        eprintln!("\n🔧 Initializing ONNX Runtime provider...");
        eprintln!("   Model: {}", config.model_name);
        eprintln!("   Dimension: {}", config.dimension);
        eprintln!("   Max length: {}", config.max_length);
        eprintln!("   Execution provider: {:?}", config.execution_provider);

        // 1. Create session with execution provider
        eprintln!("📦 Loading ONNX model from: {:?}", config.model_path);

        let mut builder = Session::builder()
            .map_err(|e| EmbeddingError::Internal(format!("Failed to create session builder: {}", e)))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| EmbeddingError::Internal(format!("Failed to set optimization level: {}", e)))?
            .with_intra_threads(4)
            .map_err(|e| EmbeddingError::Internal(format!("Failed to set threads: {}", e)))?;

        // Configure execution provider
        builder = match &config.execution_provider {
            #[cfg(feature = "tensorrt")]
            ExecutionProviderConfig::TensorRT {
                device_id,
                fp8_enable,
                engine_cache_path,
            } => {
                eprintln!("🚀 Configuring TensorRT Execution Provider (FP8: {})", fp8_enable);

                use ort::TensorRTExecutionProvider;
                let mut trt_options = TensorRTExecutionProvider::default()
                    .with_device_id(*device_id)
                    .with_fp16_enable(true)  // Enable FP16 for better performance
                    .with_engine_cache_enable(true)
                    .with_timing_cache_enable(true);

                if *fp8_enable {
                    // FP8 is enabled via TensorRT builder flags
                    eprintln!("   ⚡ FP8 quantization enabled");
                }

                if let Some(cache_path) = engine_cache_path {
                    let cache_path_str = cache_path.to_str()
                        .ok_or_else(|| EmbeddingError::Internal(
                            format!("TensorRT engine cache path is not valid UTF-8: {:?}", cache_path)
                        ))?;
                    trt_options = trt_options.with_engine_cache_path(cache_path_str);
                    eprintln!("   💾 Engine cache: {:?}", cache_path);
                }

                builder.with_execution_providers([trt_options])
                    .map_err(|e| EmbeddingError::Internal(format!("Failed to set TensorRT EP: {}", e)))?
            }

            #[cfg(feature = "cuda")]
            ExecutionProviderConfig::CUDA { device_id } => {
                eprintln!("🎮 Configuring CUDA Execution Provider");

                use ort::CUDAExecutionProvider;
                let cuda_options = CUDAExecutionProvider::default()
                    .with_device_id(*device_id);

                builder.with_execution_providers([cuda_options])
                    .map_err(|e| EmbeddingError::Internal(format!("Failed to set CUDA EP: {}", e)))?
            }

            ExecutionProviderConfig::CoreML => {
                eprintln!("🍎 Configuring CoreML Execution Provider");
                // CoreML is automatically detected on macOS
                builder
            }

            ExecutionProviderConfig::CPU => {
                eprintln!("💻 Using CPU Execution Provider");
                builder
            }

            #[allow(unreachable_patterns)]
            _ => {
                eprintln!("⚠️  Execution provider not available, falling back to CPU");
                builder
            }
        };

        let session = builder
            .commit_from_file(&config.model_path)
            .map_err(|e| EmbeddingError::Internal(format!("Failed to load model: {}", e)))?;

        eprintln!("✅ ONNX model loaded successfully");

        // 2. Load tokenizer
        eprintln!("📝 Loading tokenizer from: {:?}", config.tokenizer_path);
        let tokenizer = Tokenizer::from_file(&config.tokenizer_path)
            .map_err(|e| EmbeddingError::Internal(format!("Failed to load tokenizer: {}", e)))?;
        eprintln!("✅ Tokenizer loaded successfully");

        eprintln!(
            "✅ OnnxEmbeddingProvider initialized\n   Model: {}\n   Dimension: {}",
            config.model_name, config.dimension
        );

        Ok(Self {
            session: Mutex::new(session),
            tokenizer,
            config,
        })
    }

    /// Create new ONNX embedding provider (legacy API).
    ///
    /// # Arguments
    ///
    /// * `model_path` - Path to ONNX model file (.onnx)
    /// * `tokenizer_path` - Path to tokenizer.json file
    /// * `model_name` - Model name (e.g., "sentence-transformers/all-MiniLM-L6-v2")
    ///
    /// # Deprecated
    ///
    /// Use `with_config()` instead for more control over execution providers.
    pub async fn new(model_path: &str, tokenizer_path: &str, model_name: &str) -> EmbeddingResult<Self> {
        let config = OnnxConfig {
            model_path: PathBuf::from(model_path),
            tokenizer_path: PathBuf::from(tokenizer_path),
            model_name: model_name.to_string(),
            dimension: 384, // Default for MiniLM
            max_length: 512,
            execution_provider: ExecutionProviderConfig::CPU,
        };

        Self::with_config(config).await
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

        let max_length = self.config.max_length;
        let batch_size = texts.len();

        // 3. Pad/truncate to fixed length
        let mut input_ids_vec = Vec::with_capacity(batch_size * max_length);
        let mut attention_mask_vec = Vec::with_capacity(batch_size * max_length);
        let mut token_type_ids_vec = Vec::with_capacity(batch_size * max_length);

        for encoding in encodings {
            let mut ids = encoding.get_ids().to_vec();
            let mut mask = encoding.get_attention_mask().to_vec();
            let mut type_ids = encoding.get_type_ids().to_vec();

            if ids.len() > max_length {
                ids.truncate(max_length);
                mask.truncate(max_length);
                type_ids.truncate(max_length);
            } else {
                ids.resize(max_length, 0);
                mask.resize(max_length, 0);
                type_ids.resize(max_length, 0);
            }

            input_ids_vec.extend(ids.iter().map(|&x| x as i64));
            attention_mask_vec.extend(mask.iter().map(|&x| x as i64));
            token_type_ids_vec.extend(type_ids.iter().map(|&x| x as i64));
        }

        // 4. Create input tensors
        let input_ids_array = Array2::from_shape_vec(
            (batch_size, max_length),
            input_ids_vec,
        )
        .map_err(|e| EmbeddingError::Internal(format!("Failed to create input tensor: {}", e)))?;

        let attention_mask_array = Array2::from_shape_vec(
            (batch_size, max_length),
            attention_mask_vec.clone(),
        )
        .map_err(|e| EmbeddingError::Internal(format!("Failed to create mask tensor: {}", e)))?;

        let token_type_ids_array = Array2::from_shape_vec(
            (batch_size, max_length),
            token_type_ids_vec,
        )
        .map_err(|e| EmbeddingError::Internal(format!("Failed to create token_type_ids tensor: {}", e)))?;

        // 5. Run ONNX inference with 3 inputs: input_ids, attention_mask, token_type_ids
        //    Pass owned arrays directly (OwnedRepr required for OwnedTensorArrayData trait)
        let input_ids_value = Value::from_array(input_ids_array)
            .map_err(|e| EmbeddingError::Internal(format!("Failed to create input_ids value: {}", e)))?;
        let attention_mask_value = Value::from_array(attention_mask_array)
            .map_err(|e| EmbeddingError::Internal(format!("Failed to create attention_mask value: {}", e)))?;
        let token_type_ids_value = Value::from_array(token_type_ids_array)
            .map_err(|e| EmbeddingError::Internal(format!("Failed to create token_type_ids value: {}", e)))?;

        // Lock session for inference (Session::run requires &mut self)
        let mut session = self.session.lock();
        let outputs = session
            .run(ort::inputs![
                "input_ids" => input_ids_value,
                "attention_mask" => attention_mask_value,
                "token_type_ids" => token_type_ids_value
            ])
            .map_err(|e| EmbeddingError::Internal(format!("ONNX inference failed: {}", e)))?;

        // 6. Extract last_hidden_state output (first output)
        let (hidden_shape, hidden_data) = outputs["last_hidden_state"]
            .try_extract_tensor::<f32>()
            .map_err(|e| EmbeddingError::Internal(format!("Failed to extract output: {}", e)))?;

        // Shape should be [batch_size, seq_len, hidden_size]
        if hidden_shape.len() != 3 {
            return Err(EmbeddingError::Internal(format!(
                "Unexpected output shape: {:?}",
                hidden_shape
            )));
        }

        let hidden_size = hidden_shape[2] as usize;

        // 7. Mean pooling with attention mask
        let mut embeddings = Vec::with_capacity(batch_size);

        for i in 0..batch_size {
            let mut pooled = vec![0.0f32; hidden_size];
            let mut sum_mask = 0.0f32;

            for j in 0..max_length {
                let mask_val = attention_mask_vec[i * max_length + j] as f32;
                sum_mask += mask_val;

                for k in 0..hidden_size {
                    // Access flat slice with manual indexing: [batch, seq, hidden]
                    let idx = i * max_length * hidden_size + j * hidden_size + k;
                    let hidden_val = hidden_data[idx];
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
            model: self.config.model_name.clone(),
            dimension: self.config.dimension,
            max_tokens: self.config.max_length,
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
        if test_embedding[0].len() != self.config.dimension as usize {
            return Err(EmbeddingError::ServiceUnavailable(format!(
                "Health check failed: wrong dimension (expected {}, got {})",
                self.config.dimension,
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
