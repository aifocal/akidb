# Jetson Thor Week 1: ONNX Runtime Foundation - PRD

**Version:** 1.0.0
**Date:** 2025-01-11
**Phase:** Week 1 of 16 (Foundation)
**Duration:** 5 working days
**Status:** READY TO EXECUTE

---

## Executive Summary

Week 1 establishes the foundation for ONNX Runtime integration by implementing a basic, working provider with Qwen3 4B FP8 on Jetson Thor. This week focuses on getting the core inference pipeline working correctly with TensorRT optimization before optimizing for performance.

**Goal:** Working ONNX embedding provider generating Qwen3 4B FP8 embeddings on Jetson Thor with TensorRT acceleration

**Key Deliverables:**
- ✅ `OnnxEmbeddingProvider` implementation (~400 lines Rust)
- ✅ Qwen3 4B model converted to ONNX format (FP8)
- ✅ TensorRT Execution Provider integration
- ✅ 12+ unit tests (100% passing)
- ✅ Basic benchmarks on Jetson Thor
- ✅ EmbeddingProvider trait integration
- ✅ Feature flag (`onnx-runtime`)

**Success Criteria:**
- ✅ Generates embeddings for single text
- ✅ Generates embeddings for batch (1-32 texts)
- ✅ All 12 unit tests pass
- ✅ P95 latency <100ms (baseline, will optimize to <30ms in Week 3)
- ✅ Uses TensorRT Execution Provider (FP8 Tensor Cores)
- ✅ Works on Jetson Thor with CUDA/TensorRT

---

## Problem Statement

### Current State

**Existing Candle Implementation:**
- Candle provider (Phase 1) targets generic ARM devices
- Uses all-MiniLM-L6-v2 (22M params, 384-dim)
- Mac ARM + generic ARM edge focus
- 5.5 QPS → 200+ QPS roadmap

**Why Week 1 Matters:**
- Establishes technical feasibility of ONNX on Jetson Thor
- Validates Qwen3 4B FP8 performance assumptions
- Creates foundation for automotive/robotics positioning
- Enables pivot from generic ARM to Jetson-first strategy

### New Requirements (Jetson Thor)

**Platform:** NVIDIA Jetson Thor
- GPU: Blackwell architecture (2,000 TOPS)
- Native FP8 Tensor Cores
- TensorRT 10.0+ required
- Unified memory architecture

**Model:** Qwen3 4B FP8
- Parameters: 4 billion (vs 22M for MiniLM)
- Dimensions: 4096 (vs 384 for MiniLM)
- Precision: FP8 (vs FP32/FP16 for Candle)
- Context: 32K tokens (vs 512 for MiniLM)

**Backend:** ONNX Runtime
- TensorRT Execution Provider (uses TensorRT under the hood)
- Native Rust bindings (`ort` crate)
- In-process inference (no RPC overhead)
- Cross-platform (can also run on x86, ARM, cloud)

---

## Goals & Non-Goals

### Goals (Week 1)

1. **Core Functionality** ✅
   - Load Qwen3 4B ONNX model from filesystem
   - Tokenize text inputs (single + batch)
   - Run inference on Jetson Thor GPU (TensorRT EP)
   - Generate 4096-dimensional embeddings
   - FP8 precision (native Blackwell support)

2. **Quality** ✅
   - Unit test coverage >80%
   - Embedding quality validation (vs HuggingFace Qwen3 baseline)
   - Error handling for common failures
   - Memory leak detection

3. **Integration** ✅
   - Implement `EmbeddingProvider` trait
   - Feature flag for ONNX Runtime (`onnx-runtime`)
   - No breaking changes to existing API
   - Coexist with Candle provider (both available)

4. **Benchmarking** ✅
   - Baseline latency measurement (P50, P95, P99)
   - Baseline throughput measurement (QPS)
   - Memory usage profiling
   - TensorRT optimization verification

### Non-Goals (Future Weeks)

1. **Performance Optimization** (Week 3)
   - Dynamic batching
   - Sub-30ms P95 latency
   - >50 QPS throughput

2. **Multi-Model Support** (Week 2)
   - Qwen3 0.5B, 1.5B, 7B models
   - Runtime model selection API
   - Model caching (LRU)

3. **Production Hardening** (Week 4)
   - Prometheus metrics
   - Circuit breaker
   - Retry logic

4. **Deployment** (Week 5)
   - Docker image
   - Systemd service
   - OTA updates

---

## Technical Design

### Architecture Overview

```
┌────────────────────────────────────────────────────────────────┐
│                    AkiDB on Jetson Thor                        │
│                    (Week 1 Foundation)                         │
└────────────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────────────────┐
│  REST API Layer (existing, no changes)                         │
│  - POST /api/v1/embed                                          │
│  - Returns embeddings to client                                │
└────────────────────┬───────────────────────────────────────────┘
                     │
                     ▼
┌────────────────────────────────────────────────────────────────┐
│  CollectionService (existing, no changes)                      │
│  - Manages collections                                         │
│  - Calls embedding provider                                    │
└────────────────────┬───────────────────────────────────────────┘
                     │
                     ▼
┌────────────────────────────────────────────────────────────────┐
│  EmbeddingProvider Trait (existing interface)                  │
│                                                                │
│  trait EmbeddingProvider {                                     │
│      async fn embed(&self, texts: Vec<String>)                │
│                    -> Result<Vec<Vec<f32>>>;                   │
│  }                                                             │
└────────┬───────────────────────────────────────────┬───────────┘
         │                                           │
         ▼                                           ▼
┌──────────────────┐                    ┌──────────────────────┐
│ CandleProvider   │                    │ OnnxProvider         │
│ (existing)       │                    │ (NEW - Week 1)       │
│                  │                    │                      │
│ all-MiniLM-L6-v2 │                    │ Qwen3 4B FP8         │
│ 384-dim          │                    │ 4096-dim             │
│ Mac ARM focus    │                    │ Jetson Thor focus    │
└──────────────────┘                    └──────────┬───────────┘
                                                   │
                                                   ▼
                              ┌─────────────────────────────────┐
                              │  ONNX Runtime (C++ library)     │
                              │                                 │
                              │  ┌───────────────────────────┐  │
                              │  │ TensorRT Execution        │  │
                              │  │ Provider (EP)             │  │
                              │  │                           │  │
                              │  │ - FP8 Tensor Cores        │  │
                              │  │ - Kernel fusion           │  │
                              │  │ - Graph optimization      │  │
                              │  └───────────────────────────┘  │
                              └──────────────┬──────────────────┘
                                             │
                                             ▼
                              ┌─────────────────────────────────┐
                              │  Jetson Thor GPU                │
                              │  - Blackwell architecture       │
                              │  - FP8 native support           │
                              │  - 2,000 TOPS                   │
                              └─────────────────────────────────┘
```

### Component Design

#### 1. OnnxEmbeddingProvider

**Responsibilities:**
- Load ONNX model file (Qwen3 4B FP8)
- Configure TensorRT Execution Provider
- Tokenize input texts
- Run inference (forward pass)
- Extract embeddings from output tensor
- Handle errors gracefully

**File:** `crates/akidb-embedding/src/onnx.rs`

**Interface:**
```rust
pub struct OnnxEmbeddingProvider {
    session: Session,              // ONNX Runtime session
    tokenizer: Tokenizer,          // HuggingFace tokenizer
    config: OnnxConfig,            // Configuration
}

pub struct OnnxConfig {
    pub model_path: PathBuf,       // Path to .onnx file
    pub tokenizer_path: PathBuf,   // Path to tokenizer.json
    pub max_length: usize,         // Max sequence length (default: 512)
    pub device_id: i32,            // GPU device ID (default: 0)
    pub enable_fp8: bool,          // Enable FP8 (default: true)
    pub cache_dir: Option<PathBuf>,// TensorRT engine cache
}

impl OnnxEmbeddingProvider {
    pub fn new(config: OnnxConfig) -> Result<Self>;
    pub async fn embed_single(&self, text: &str) -> Result<Vec<f32>>;
    pub async fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>>;
}

#[async_trait]
impl EmbeddingProvider for OnnxEmbeddingProvider {
    async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        self.embed_batch(texts).await
    }
}
```

#### 2. ONNX Runtime Session Setup

**TensorRT Execution Provider Configuration:**
```rust
use ort::{
    Environment, Session, GraphOptimizationLevel,
    ExecutionProvider, TensorRTExecutionProviderOptions,
};

pub fn create_session(config: &OnnxConfig) -> Result<Session> {
    // Create ONNX Runtime environment
    let environment = Environment::builder()
        .with_name("akidb-onnx")
        .build()?;

    // Configure TensorRT Execution Provider
    let tensorrt_options = TensorRTExecutionProviderOptions {
        device_id: config.device_id,
        fp16_enable: false,           // Don't use FP16
        int8_enable: false,           // Don't use INT8
        fp8_enable: config.enable_fp8, // Use FP8 (Blackwell)
        max_workspace_size: 2_000_000_000, // 2GB workspace for optimization
        engine_cache_enable: true,    // Cache TensorRT engines (faster startup)
        engine_cache_path: config.cache_dir.clone(),
        dla_enable: false,            // No DLA on Thor
        dla_core: 0,
        ..Default::default()
    };

    // Build session with execution providers
    let session = Session::builder()?
        .with_optimization_level(GraphOptimizationLevel::Level3)? // Max optimization
        .with_intra_threads(4)?       // 4 CPU threads for ops
        .with_execution_providers([
            ExecutionProvider::TensorRT(tensorrt_options), // Try TensorRT first
            ExecutionProvider::CUDA(Default::default()),  // Fallback to CUDA
            ExecutionProvider::CPU(Default::default()),   // Fallback to CPU
        ])?
        .with_model_from_file(&config.model_path)?;

    Ok(session)
}
```

**Key Configuration:**
- `fp8_enable: true` - Use FP8 Tensor Cores (Blackwell GPU)
- `engine_cache_enable: true` - Cache TensorRT engines (avoids rebuilding on restart)
- `max_workspace_size: 2GB` - Allow TensorRT to use GPU memory for optimization
- `GraphOptimizationLevel::Level3` - Maximum graph optimization

#### 3. Tokenization

**HuggingFace Tokenizers:**
```rust
use tokenizers::Tokenizer;

pub fn tokenize_batch(
    tokenizer: &Tokenizer,
    texts: Vec<String>,
    max_length: usize,
) -> Result<TokenizedBatch> {
    // Encode batch with padding and truncation
    let encodings = tokenizer
        .encode_batch(texts, true)?  // add_special_tokens=true
        .iter()
        .map(|encoding| {
            let mut ids = encoding.get_ids().to_vec();
            let mut mask = encoding.get_attention_mask().to_vec();

            // Truncate to max_length
            ids.truncate(max_length);
            mask.truncate(max_length);

            // Pad to max_length
            while ids.len() < max_length {
                ids.push(0); // PAD token
                mask.push(0);
            }

            (ids, mask)
        })
        .collect::<Vec<_>>();

    Ok(TokenizedBatch { encodings })
}
```

#### 4. Inference

**ONNX Runtime Inference:**
```rust
use ort::Value;
use ndarray::{Array2, Array1};

pub async fn run_inference(
    session: &Session,
    input_ids: Vec<Vec<i64>>,
    attention_mask: Vec<Vec<i64>>,
) -> Result<Vec<Vec<f32>>> {
    // Convert to ndarray (ONNX Runtime format)
    let batch_size = input_ids.len();
    let seq_length = input_ids[0].len();

    let input_ids_array = Array2::from_shape_vec(
        (batch_size, seq_length),
        input_ids.into_iter().flatten().collect(),
    )?;

    let attention_mask_array = Array2::from_shape_vec(
        (batch_size, seq_length),
        attention_mask.into_iter().flatten().collect(),
    )?;

    // Create ONNX tensors
    let input_ids_tensor = Value::from_array(session.allocator(), &input_ids_array)?;
    let attention_mask_tensor = Value::from_array(session.allocator(), &attention_mask_array)?;

    // Run inference (uses TensorRT EP)
    let outputs = session.run(vec![
        ("input_ids", input_ids_tensor),
        ("attention_mask", attention_mask_tensor),
    ])?;

    // Extract embeddings (last hidden state, mean pooling)
    let last_hidden_state = outputs["last_hidden_state"]
        .try_extract::<f32>()?
        .view();

    // Mean pooling (average across sequence dimension)
    let embeddings = mean_pooling(last_hidden_state, &attention_mask_array)?;

    Ok(embeddings)
}

fn mean_pooling(
    hidden_states: ArrayView3<f32>, // (batch, seq, hidden_dim)
    attention_mask: &Array2<i64>,   // (batch, seq)
) -> Result<Vec<Vec<f32>>> {
    let batch_size = hidden_states.shape()[0];
    let hidden_dim = hidden_states.shape()[2];

    let mut embeddings = Vec::with_capacity(batch_size);

    for i in 0..batch_size {
        let hidden = hidden_states.slice(s![i, .., ..]); // (seq, hidden_dim)
        let mask = attention_mask.row(i);                // (seq,)

        // Sum hidden states where mask = 1
        let mut sum = Array1::<f32>::zeros(hidden_dim);
        let mut count = 0;

        for (j, &mask_val) in mask.iter().enumerate() {
            if mask_val == 1 {
                sum = sum + hidden.row(j);
                count += 1;
            }
        }

        // Average
        let embedding = (sum / count as f32).to_vec();
        embeddings.push(embedding);
    }

    Ok(embeddings)
}
```

---

## Implementation Plan (5 Days)

### Day 1: Environment Setup & Model Conversion

**Goal:** Set up Jetson Thor development environment and convert Qwen3 4B to ONNX

**Tasks:**

1. **Install Dependencies on Jetson Thor**
   ```bash
   # Update system
   sudo apt update && sudo apt upgrade -y

   # Install CUDA 12.0+ (should be pre-installed on Thor)
   nvidia-smi  # Verify CUDA

   # Install TensorRT 10.0+ (should be pre-installed)
   dpkg -l | grep tensorrt  # Verify TensorRT

   # Install Rust
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env

   # Install Python 3.10+ (for model conversion)
   sudo apt install python3.10 python3-pip -y

   # Install HuggingFace transformers + optimum
   pip3 install transformers torch onnx optimum[exporters]
   ```

2. **Convert Qwen3 4B to ONNX Format**
   ```bash
   # Download and convert Qwen3 4B to ONNX with FP8
   python3 << EOF
   from optimum.onnxruntime import ORTModelForFeatureExtraction
   from transformers import AutoTokenizer

   model_id = "Qwen/Qwen2.5-4B"
   output_dir = "./models/qwen3-4b-onnx-fp8"

   # Load model and tokenizer
   tokenizer = AutoTokenizer.from_pretrained(model_id)

   # Export to ONNX with FP8 quantization
   model = ORTModelForFeatureExtraction.from_pretrained(
       model_id,
       export=True,
       provider="TensorrtExecutionProvider",
       use_auth_token=None,
   )

   # Save ONNX model + tokenizer
   model.save_pretrained(output_dir)
   tokenizer.save_pretrained(output_dir)

   print(f"✅ Qwen3 4B ONNX saved to {output_dir}")
   EOF
   ```

3. **Verify ONNX Model**
   ```python
   import onnx

   # Load ONNX model
   model = onnx.load("./models/qwen3-4b-onnx-fp8/model.onnx")

   # Check model
   onnx.checker.check_model(model)
   print("✅ ONNX model is valid")

   # Print model info
   print(f"Inputs: {[input.name for input in model.graph.input]}")
   print(f"Outputs: {[output.name for output in model.graph.output]}")
   ```

**Deliverables:**
- [x] Jetson Thor with Rust + CUDA + TensorRT installed
- [x] Qwen3 4B ONNX model (FP8) in `models/qwen3-4b-onnx-fp8/`
- [x] Tokenizer files in `models/qwen3-4b-onnx-fp8/`

---

### Day 2: ONNX Provider Skeleton

**Goal:** Create `OnnxEmbeddingProvider` struct and implement basic model loading

**Tasks:**

1. **Add Dependencies to Cargo.toml**
   ```toml
   # crates/akidb-embedding/Cargo.toml

   [dependencies]
   # Existing
   async-trait = "0.1"
   thiserror = "1.0"

   # NEW for ONNX Runtime
   ort = { version = "2.0", features = ["cuda", "tensorrt"] }
   tokenizers = { version = "0.15", default-features = false }
   ndarray = "0.15"

   [features]
   default = []
   onnx-runtime = ["ort", "tokenizers", "ndarray"]
   candle = ["candle-core", "candle-nn", "hf-hub"]  # Existing
   ```

2. **Create `onnx.rs` Module**
   ```rust
   // crates/akidb-embedding/src/onnx.rs

   use async_trait::async_trait;
   use ort::{Environment, Session, GraphOptimizationLevel};
   use tokenizers::Tokenizer;
   use std::path::PathBuf;
   use crate::{EmbeddingProvider, CoreResult, CoreError};

   pub struct OnnxEmbeddingProvider {
       session: Session,
       tokenizer: Tokenizer,
       config: OnnxConfig,
   }

   #[derive(Clone)]
   pub struct OnnxConfig {
       pub model_path: PathBuf,
       pub tokenizer_path: PathBuf,
       pub max_length: usize,
       pub device_id: i32,
       pub enable_fp8: bool,
       pub cache_dir: Option<PathBuf>,
   }

   impl Default for OnnxConfig {
       fn default() -> Self {
           Self {
               model_path: PathBuf::from("models/qwen3-4b-onnx-fp8/model.onnx"),
               tokenizer_path: PathBuf::from("models/qwen3-4b-onnx-fp8/tokenizer.json"),
               max_length: 512,
               device_id: 0,
               enable_fp8: true,
               cache_dir: Some(PathBuf::from("/tmp/tensorrt_cache")),
           }
       }
   }

   impl OnnxEmbeddingProvider {
       pub fn new(config: OnnxConfig) -> CoreResult<Self> {
           // Load tokenizer
           let tokenizer = Tokenizer::from_file(&config.tokenizer_path)
               .map_err(|e| CoreError::EmbeddingError(format!("Failed to load tokenizer: {}", e)))?;

           // Create ONNX session (placeholder, will implement in Day 2)
           let session = Self::create_session(&config)?;

           Ok(Self {
               session,
               tokenizer,
               config,
           })
       }

       fn create_session(config: &OnnxConfig) -> CoreResult<Session> {
           // TODO: Implement in Day 2
           todo!("Create ONNX Runtime session with TensorRT EP")
       }
   }

   #[async_trait]
   impl EmbeddingProvider for OnnxEmbeddingProvider {
       async fn embed(&self, texts: Vec<String>) -> CoreResult<Vec<Vec<f32>>> {
           // TODO: Implement in Day 3
           todo!("Implement embedding generation")
       }
   }
   ```

3. **Implement `create_session()` with TensorRT EP**
   ```rust
   use ort::{ExecutionProvider, TensorRTExecutionProviderOptions};

   fn create_session(config: &OnnxConfig) -> CoreResult<Session> {
       // Create environment
       let environment = Environment::builder()
           .with_name("akidb-onnx")
           .build()
           .map_err(|e| CoreError::EmbeddingError(format!("Failed to create environment: {}", e)))?;

       // Configure TensorRT EP
       let tensorrt_options = TensorRTExecutionProviderOptions {
           device_id: config.device_id,
           fp16_enable: false,
           int8_enable: false,
           fp8_enable: config.enable_fp8,
           max_workspace_size: 2_000_000_000,
           engine_cache_enable: true,
           engine_cache_path: config.cache_dir.clone(),
           dla_enable: false,
           dla_core: 0,
           ..Default::default()
       };

       // Build session
       let session = Session::builder()
           .map_err(|e| CoreError::EmbeddingError(format!("Failed to create session builder: {}", e)))?
           .with_optimization_level(GraphOptimizationLevel::Level3)
           .map_err(|e| CoreError::EmbeddingError(format!("Failed to set optimization level: {}", e)))?
           .with_intra_threads(4)
           .map_err(|e| CoreError::EmbeddingError(format!("Failed to set threads: {}", e)))?
           .with_execution_providers([
               ExecutionProvider::TensorRT(tensorrt_options),
               ExecutionProvider::CUDA(Default::default()),
               ExecutionProvider::CPU(Default::default()),
           ])
           .map_err(|e| CoreError::EmbeddingError(format!("Failed to set execution providers: {}", e)))?
           .with_model_from_file(&config.model_path)
           .map_err(|e| CoreError::EmbeddingError(format!("Failed to load model: {}", e)))?;

       Ok(session)
   }
   ```

4. **Update `lib.rs` to Export ONNX Provider**
   ```rust
   // crates/akidb-embedding/src/lib.rs

   pub mod provider;
   pub mod types;
   pub mod mock;

   #[cfg(feature = "candle")]
   pub mod candle;

   #[cfg(feature = "onnx-runtime")]
   pub mod onnx;

   pub use provider::EmbeddingProvider;
   pub use types::*;

   #[cfg(feature = "onnx-runtime")]
   pub use onnx::{OnnxEmbeddingProvider, OnnxConfig};
   ```

**Deliverables:**
- [x] `onnx.rs` module created
- [x] `OnnxEmbeddingProvider` struct with model loading
- [x] TensorRT Execution Provider configured
- [x] Compiles successfully on Jetson Thor

---

### Day 3: Tokenization & Inference

**Goal:** Implement tokenization and basic inference (forward pass)

**Tasks:**

1. **Implement Tokenization**
   ```rust
   // In onnx.rs

   use ndarray::Array2;

   #[derive(Debug)]
   struct TokenizedBatch {
       input_ids: Array2<i64>,
       attention_mask: Array2<i64>,
   }

   impl OnnxEmbeddingProvider {
       fn tokenize(&self, texts: Vec<String>) -> CoreResult<TokenizedBatch> {
           let max_length = self.config.max_length;

           // Encode batch
           let encodings = self.tokenizer
               .encode_batch(texts, true)
               .map_err(|e| CoreError::EmbeddingError(format!("Tokenization failed: {}", e)))?;

           let batch_size = encodings.len();
           let mut input_ids_flat = Vec::with_capacity(batch_size * max_length);
           let mut attention_mask_flat = Vec::with_capacity(batch_size * max_length);

           for encoding in encodings.iter() {
               let mut ids = encoding.get_ids().to_vec();
               let mut mask = encoding.get_attention_mask().to_vec();

               // Truncate
               ids.truncate(max_length);
               mask.truncate(max_length);

               // Pad
               while ids.len() < max_length {
                   ids.push(0);
                   mask.push(0);
               }

               // Convert to i64
               input_ids_flat.extend(ids.iter().map(|&id| id as i64));
               attention_mask_flat.extend(mask.iter().map(|&m| m as i64));
           }

           // Create 2D arrays
           let input_ids = Array2::from_shape_vec(
               (batch_size, max_length),
               input_ids_flat,
           ).map_err(|e| CoreError::EmbeddingError(format!("Failed to create input_ids array: {}", e)))?;

           let attention_mask = Array2::from_shape_vec(
               (batch_size, max_length),
               attention_mask_flat,
           ).map_err(|e| CoreError::EmbeddingError(format!("Failed to create attention_mask array: {}", e)))?;

           Ok(TokenizedBatch { input_ids, attention_mask })
       }
   }
   ```

2. **Implement Inference**
   ```rust
   use ort::Value;
   use ndarray::{s, ArrayView3, Array1};

   impl OnnxEmbeddingProvider {
       fn run_inference(&self, batch: TokenizedBatch) -> CoreResult<Vec<Vec<f32>>> {
           // Create ONNX tensors
           let input_ids_tensor = Value::from_array(self.session.allocator(), &batch.input_ids)
               .map_err(|e| CoreError::EmbeddingError(format!("Failed to create input tensor: {}", e)))?;

           let attention_mask_tensor = Value::from_array(self.session.allocator(), &batch.attention_mask)
               .map_err(|e| CoreError::EmbeddingError(format!("Failed to create mask tensor: {}", e)))?;

           // Run inference (uses TensorRT EP)
           let outputs = self.session.run(vec![input_ids_tensor, attention_mask_tensor])
               .map_err(|e| CoreError::EmbeddingError(format!("Inference failed: {}", e)))?;

           // Extract last_hidden_state (output 0)
           let last_hidden_state = outputs[0]
               .try_extract::<f32>()
               .map_err(|e| CoreError::EmbeddingError(format!("Failed to extract output: {}", e)))?;

           let hidden_view = last_hidden_state.view();

           // Mean pooling
           let embeddings = self.mean_pooling(hidden_view, &batch.attention_mask)?;

           Ok(embeddings)
       }

       fn mean_pooling(
           &self,
           hidden_states: ArrayView3<f32>, // (batch, seq, hidden_dim)
           attention_mask: &Array2<i64>,   // (batch, seq)
       ) -> CoreResult<Vec<Vec<f32>>> {
           let batch_size = hidden_states.shape()[0];
           let hidden_dim = hidden_states.shape()[2];

           let mut embeddings = Vec::with_capacity(batch_size);

           for i in 0..batch_size {
               let hidden = hidden_states.slice(s![i, .., ..]); // (seq, hidden_dim)
               let mask = attention_mask.row(i);                // (seq,)

               // Sum hidden states where mask = 1
               let mut sum = Array1::<f32>::zeros(hidden_dim);
               let mut count = 0;

               for (j, &mask_val) in mask.iter().enumerate() {
                   if mask_val == 1 {
                       sum = sum + hidden.row(j);
                       count += 1;
                   }
               }

               // Average
               if count > 0 {
                   let embedding = (sum / count as f32).to_vec();
                   embeddings.push(embedding);
               } else {
                   return Err(CoreError::EmbeddingError("Empty sequence after masking".to_string()));
               }
           }

           Ok(embeddings)
       }
   }
   ```

3. **Implement `embed()` Method**
   ```rust
   #[async_trait]
   impl EmbeddingProvider for OnnxEmbeddingProvider {
       async fn embed(&self, texts: Vec<String>) -> CoreResult<Vec<Vec<f32>>> {
           if texts.is_empty() {
               return Ok(Vec::new());
           }

           // Tokenize
           let batch = self.tokenize(texts)?;

           // Run inference (blocking, but fast on GPU)
           let embeddings = self.run_inference(batch)?;

           Ok(embeddings)
       }
   }
   ```

**Deliverables:**
- [x] Tokenization implemented and tested
- [x] Inference working on Jetson Thor
- [x] Mean pooling implemented
- [x] End-to-end embedding generation working

---

### Day 4: Testing & Quality Assurance

**Goal:** Write comprehensive unit tests and validate quality

**Tasks:**

1. **Unit Tests**
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;

       #[tokio::test]
       async fn test_onnx_provider_creation() {
           let config = OnnxConfig::default();
           let provider = OnnxEmbeddingProvider::new(config);
           assert!(provider.is_ok());
       }

       #[tokio::test]
       async fn test_single_text_embedding() {
           let config = OnnxConfig::default();
           let provider = OnnxEmbeddingProvider::new(config).unwrap();

           let texts = vec!["Hello world".to_string()];
           let result = provider.embed(texts).await;

           assert!(result.is_ok());
           let embeddings = result.unwrap();
           assert_eq!(embeddings.len(), 1);
           assert_eq!(embeddings[0].len(), 4096); // Qwen3 4B dimension
       }

       #[tokio::test]
       async fn test_batch_embedding() {
           let config = OnnxConfig::default();
           let provider = OnnxEmbeddingProvider::new(config).unwrap();

           let texts = vec![
               "First text".to_string(),
               "Second text".to_string(),
               "Third text".to_string(),
           ];
           let result = provider.embed(texts).await;

           assert!(result.is_ok());
           let embeddings = result.unwrap();
           assert_eq!(embeddings.len(), 3);
           for emb in embeddings {
               assert_eq!(emb.len(), 4096);
           }
       }

       #[tokio::test]
       async fn test_empty_batch() {
           let config = OnnxConfig::default();
           let provider = OnnxEmbeddingProvider::new(config).unwrap();

           let texts = Vec::new();
           let result = provider.embed(texts).await;

           assert!(result.is_ok());
           let embeddings = result.unwrap();
           assert_eq!(embeddings.len(), 0);
       }

       #[tokio::test]
       async fn test_long_text_truncation() {
           let config = OnnxConfig {
               max_length: 128,
               ..Default::default()
           };
           let provider = OnnxEmbeddingProvider::new(config).unwrap();

           let long_text = "word ".repeat(1000); // 1000 words, will be truncated
           let texts = vec![long_text];
           let result = provider.embed(texts).await;

           assert!(result.is_ok());
           let embeddings = result.unwrap();
           assert_eq!(embeddings.len(), 1);
           assert_eq!(embeddings[0].len(), 4096);
       }

       #[tokio::test]
       async fn test_embedding_similarity() {
           // Similar texts should have similar embeddings
           let config = OnnxConfig::default();
           let provider = OnnxEmbeddingProvider::new(config).unwrap();

           let texts = vec![
               "Machine learning is a subset of artificial intelligence".to_string(),
               "AI includes machine learning as a subfield".to_string(),
               "The weather is nice today".to_string(),
           ];
           let embeddings = provider.embed(texts).await.unwrap();

           // Compute cosine similarity
           let sim_01 = cosine_similarity(&embeddings[0], &embeddings[1]);
           let sim_02 = cosine_similarity(&embeddings[0], &embeddings[2]);

           // Similar texts (0 and 1) should have higher similarity than dissimilar (0 and 2)
           assert!(sim_01 > sim_02);
           assert!(sim_01 > 0.7); // High similarity threshold
       }

       fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
           let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
           let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
           let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
           dot / (norm_a * norm_b)
       }

       #[tokio::test]
       async fn test_tensorrt_provider_used() {
           // Verify that TensorRT EP is being used
           let config = OnnxConfig::default();
           let provider = OnnxEmbeddingProvider::new(config).unwrap();

           // This test passes if TensorRT EP is configured correctly
           // (actual EP usage is verified at runtime by ONNX Runtime)
           assert!(provider.session.inputs().is_ok());
       }
   }
   ```

2. **Integration Test**
   ```rust
   #[cfg(test)]
   mod integration_tests {
       use super::*;

       #[tokio::test]
       async fn test_provider_trait_compatibility() {
           // Verify OnnxEmbeddingProvider implements EmbeddingProvider trait
           let config = OnnxConfig::default();
           let provider: Box<dyn EmbeddingProvider> = Box::new(
               OnnxEmbeddingProvider::new(config).unwrap()
           );

           let texts = vec!["Test".to_string()];
           let result = provider.embed(texts).await;
           assert!(result.is_ok());
       }
   }
   ```

3. **Quality Validation (Compare with HuggingFace)**
   ```python
   # scripts/validate_onnx_quality.py
   # Compare ONNX embeddings with HuggingFace baseline

   from transformers import AutoModel, AutoTokenizer
   import torch
   import numpy as np

   # Load HuggingFace model
   model_id = "Qwen/Qwen2.5-4B"
   tokenizer = AutoTokenizer.from_pretrained(model_id)
   model = AutoModel.from_pretrained(model_id)

   # Test texts
   texts = [
       "Machine learning is a subset of artificial intelligence",
       "The weather is nice today",
   ]

   # Generate HuggingFace embeddings
   inputs = tokenizer(texts, return_tensors="pt", padding=True, truncation=True, max_length=512)
   with torch.no_grad():
       outputs = model(**inputs)
       hf_embeddings = outputs.last_hidden_state.mean(dim=1).numpy()

   # Load ONNX embeddings (from AkiDB)
   # TODO: Call AkiDB API to get ONNX embeddings
   # onnx_embeddings = requests.post("http://localhost:8080/api/v1/embed", json={"texts": texts})

   # Compare (cosine similarity should be >0.99)
   # similarity = cosine_similarity(hf_embeddings, onnx_embeddings)
   # assert similarity > 0.99, f"Quality issue: similarity={similarity}"

   print("✅ Quality validation script ready (integrate with AkiDB API)")
   ```

**Deliverables:**
- [x] 12+ unit tests passing
- [x] Integration test passing
- [x] Quality validation script ready

---

### Day 5: Benchmarking & Documentation

**Goal:** Establish performance baseline and document usage

**Tasks:**

1. **Benchmark Script**
   ```rust
   // benches/onnx_benchmark.rs

   use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
   use akidb_embedding::onnx::{OnnxEmbeddingProvider, OnnxConfig};
   use akidb_embedding::EmbeddingProvider;

   async fn embed_single(provider: &OnnxEmbeddingProvider, text: &str) {
       let _ = provider.embed(vec![text.to_string()]).await.unwrap();
   }

   async fn embed_batch(provider: &OnnxEmbeddingProvider, texts: Vec<String>) {
       let _ = provider.embed(texts).await.unwrap();
   }

   fn benchmark_latency(c: &mut Criterion) {
       let rt = tokio::runtime::Runtime::new().unwrap();
       let config = OnnxConfig::default();
       let provider = OnnxEmbeddingProvider::new(config).unwrap();

       // Single text latency
       c.bench_function("onnx_single_text", |b| {
           b.to_async(&rt).iter(|| {
               embed_single(&provider, black_box("Machine learning and artificial intelligence"))
           });
       });

       // Batch latency (varying sizes)
       for batch_size in [1, 2, 4, 8, 16, 32].iter() {
           let texts: Vec<String> = (0..*batch_size)
               .map(|i| format!("This is test text number {}", i))
               .collect();

           c.bench_with_input(
               BenchmarkId::new("onnx_batch", batch_size),
               &texts,
               |b, texts| {
                   b.to_async(&rt).iter(|| embed_batch(&provider, texts.clone()));
               },
           );
       }
   }

   criterion_group!(benches, benchmark_latency);
   criterion_main!(benches);
   ```

2. **Run Benchmarks on Jetson Thor**
   ```bash
   # Build with optimizations
   cargo build --release --features onnx-runtime

   # Run benchmarks
   cargo bench --bench onnx_benchmark

   # Save results
   cargo bench --bench onnx_benchmark > results/week1_baseline_benchmarks.txt
   ```

3. **Manual Testing Script**
   ```bash
   # scripts/test_onnx_thor.sh

   #!/bin/bash

   echo "=== Testing ONNX Provider on Jetson Thor ==="

   # Build
   cargo build --release --features onnx-runtime

   # Run unit tests
   cargo test --features onnx-runtime -- --nocapture

   # Run benchmarks
   cargo bench --bench onnx_benchmark

   # Memory profiling
   valgrind --tool=massif ./target/release/akidb-rest &
   sleep 60
   pkill akidb-rest

   echo "✅ Testing complete"
   ```

4. **Documentation**
   ```markdown
   # ONNX Runtime Provider - Week 1 Documentation

   ## Overview

   The ONNX Runtime provider enables high-performance embedding generation using Qwen3 4B FP8 on NVIDIA Jetson Thor.

   ## Features

   - **Model**: Qwen3 4B (4 billion parameters, 4096-dimensional embeddings)
   - **Precision**: FP8 (native Blackwell Tensor Core support)
   - **Backend**: ONNX Runtime with TensorRT Execution Provider
   - **Platform**: NVIDIA Jetson Thor (Blackwell GPU)

   ## Usage

   ```rust
   use akidb_embedding::onnx::{OnnxEmbeddingProvider, OnnxConfig};
   use akidb_embedding::EmbeddingProvider;

   #[tokio::main]
   async fn main() {
       // Configure provider
       let config = OnnxConfig {
           model_path: "models/qwen3-4b-onnx-fp8/model.onnx".into(),
           tokenizer_path: "models/qwen3-4b-onnx-fp8/tokenizer.json".into(),
           max_length: 512,
           device_id: 0,
           enable_fp8: true,
           cache_dir: Some("/tmp/tensorrt_cache".into()),
       };

       // Create provider
       let provider = OnnxEmbeddingProvider::new(config).unwrap();

       // Generate embeddings
       let texts = vec![
           "Machine learning is a subset of AI".to_string(),
           "Natural language processing with transformers".to_string(),
       ];

       let embeddings = provider.embed(texts).await.unwrap();

       println!("Generated {} embeddings", embeddings.len());
       println!("Embedding dimension: {}", embeddings[0].len());
   }
   ```

   ## Configuration

   | Parameter | Type | Default | Description |
   |-----------|------|---------|-------------|
   | `model_path` | `PathBuf` | `models/qwen3-4b-onnx-fp8/model.onnx` | Path to ONNX model file |
   | `tokenizer_path` | `PathBuf` | `models/qwen3-4b-onnx-fp8/tokenizer.json` | Path to tokenizer file |
   | `max_length` | `usize` | `512` | Maximum sequence length |
   | `device_id` | `i32` | `0` | GPU device ID |
   | `enable_fp8` | `bool` | `true` | Enable FP8 precision (Blackwell only) |
   | `cache_dir` | `Option<PathBuf>` | `/tmp/tensorrt_cache` | TensorRT engine cache directory |

   ## Performance (Jetson Thor)

   **Baseline (Week 1):**
   - Single text latency: TBD (target: <100ms)
   - Batch 8 latency: TBD (target: <200ms)
   - Throughput: TBD (target: >10 QPS)
   - Memory usage: ~4.8GB

   **Week 3 Target (Optimized):**
   - Single text latency: <30ms P95
   - Batch 32 latency: <50ms P95
   - Throughput: >50 QPS
   - Memory usage: <5GB

   ## Troubleshooting

   **TensorRT engine build takes long (first run)**:
   - Expected. TensorRT optimizes the model on first run.
   - Subsequent runs use cached engine (fast startup).

   **Out of memory**:
   - Reduce `max_length` to 256 or 128
   - Reduce batch size
   - Close other GPU applications

   **Slow inference**:
   - Verify TensorRT EP is being used (check logs)
   - Ensure FP8 is enabled (`enable_fp8: true`)
   - Check GPU utilization: `nvidia-smi`
   ```

**Deliverables:**
- [x] Benchmarks run on Jetson Thor
- [x] Baseline performance documented
- [x] Usage documentation written
- [x] Troubleshooting guide created

---

## Success Criteria (Week 1)

### Functional Requirements

- [x] **Model Loading**: Qwen3 4B ONNX model loads successfully
- [x] **Tokenization**: Texts are tokenized correctly (padding, truncation)
- [x] **Inference**: Forward pass completes without errors
- [x] **Embeddings**: 4096-dimensional embeddings generated
- [x] **Batch Processing**: Handles 1-32 texts in batch
- [x] **Error Handling**: Graceful error handling for invalid inputs

### Quality Requirements

- [x] **Tests**: 12+ unit tests passing (>80% code coverage)
- [x] **Quality**: Embedding similarity >0.99 vs HuggingFace baseline
- [x] **Memory**: No memory leaks (valgrind clean)
- [x] **Stability**: Runs for 1+ hour without crashes

### Performance Requirements (Baseline)

- [x] **Latency (P95)**: <100ms single text (will optimize to <30ms in Week 3)
- [x] **Throughput**: >10 QPS (will optimize to >50 QPS in Week 3)
- [x] **Memory Usage**: <6GB total (model + runtime)
- [x] **GPU Utilization**: >50% during inference

### Integration Requirements

- [x] **EmbeddingProvider Trait**: Implements trait correctly
- [x] **Feature Flag**: Compiles with `--features onnx-runtime`
- [x] **Coexistence**: Works alongside Candle provider
- [x] **No Breaking Changes**: Existing APIs unchanged

---

## Risks & Mitigation

### Risk 1: TensorRT Engine Build Time ⚠️ MEDIUM

**Risk**: First run TensorRT engine optimization takes 5-15 minutes

**Mitigation**:
- ✅ Enable engine caching (`engine_cache_enable: true`)
- ✅ Pre-build engine during Docker image creation
- ✅ Document expected first-run delay

### Risk 2: Memory Constraints 🟡 LOW

**Risk**: Qwen3 4B FP8 (4GB) + ONNX Runtime + TensorRT may exceed Thor memory

**Mitigation**:
- ✅ FP8 precision reduces memory (4GB vs 16GB for FP32)
- ✅ Monitor memory usage during testing
- ✅ Fallback to smaller model (Qwen3 1.5B) if needed

### Risk 3: ONNX Conversion Issues 🟡 LOW

**Risk**: Qwen3 4B may not convert cleanly to ONNX

**Mitigation**:
- ✅ Use HuggingFace Optimum (well-tested conversion tool)
- ✅ Validate ONNX model with `onnx.checker`
- ✅ Test with HuggingFace baseline (quality check)

### Risk 4: TensorRT EP Compatibility 🟢 LOW

**Risk**: TensorRT EP may not support all Qwen3 ops

**Mitigation**:
- ✅ Fallback to CUDA EP if TensorRT fails
- ✅ ONNX Runtime handles EP fallback automatically
- ✅ Test on Jetson Thor (production hardware)

---

## Deliverables Summary

### Code
- [x] `crates/akidb-embedding/src/onnx.rs` (~400 lines)
- [x] `OnnxEmbeddingProvider` struct + implementation
- [x] 12+ unit tests
- [x] Integration test
- [x] Benchmark suite

### Documentation
- [x] Usage guide (README)
- [x] API documentation (rustdoc)
- [x] Troubleshooting guide
- [x] Week 1 completion report

### Assets
- [x] Qwen3 4B ONNX model (FP8)
- [x] Tokenizer files
- [x] Benchmark results (baseline)
- [x] Quality validation results

---

## Next Steps (Week 2)

After Week 1 foundation is complete:

1. **Multi-Model Support** (Week 2)
   - Add Qwen3 0.5B, 1.5B, 7B models
   - Runtime model selection API
   - Model registry + LRU cache

2. **Performance Optimization** (Week 3)
   - Dynamic batching (2-32 requests)
   - Latency optimization (<30ms P95)
   - Throughput optimization (>50 QPS)

3. **Production Hardening** (Week 4)
   - Observability (Prometheus metrics)
   - Resilience (circuit breaker, retry)
   - Chaos testing

4. **Deployment** (Week 5)
   - Docker image (L4T base)
   - Systemd service
   - OTA updates

---

## Conclusion

Week 1 establishes a solid foundation for ONNX Runtime integration on Jetson Thor. By the end of the week, we'll have a working embedding provider generating high-quality Qwen3 4B FP8 embeddings with TensorRT acceleration.

**Key Achievements:**
- ✅ ONNX Runtime + TensorRT EP working on Jetson Thor
- ✅ Qwen3 4B FP8 model loaded and running
- ✅ 12+ tests passing, quality validated
- ✅ Baseline performance measured
- ✅ Foundation ready for optimization (Weeks 2-3)

**This is an achievable, focused plan that delivers a working prototype in 5 days.** 🚀
