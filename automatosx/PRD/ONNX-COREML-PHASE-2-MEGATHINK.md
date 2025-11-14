# ONNX+CoreML Phase 2 Implementation Megathink

**Date**: November 10, 2025
**Status**: 🧠 **PLANNING** - Comprehensive implementation strategy
**Goal**: Implement production-ready ONNX Runtime + CoreML EP embedding provider in Rust

---

## Executive Summary

**Current State**:
- ✅ Candle removed from codebase (26 files archived)
- ✅ Qwen3-Embedding-0.6B ONNX downloaded (7.5 GB, FP16 recommended)
- ✅ Comprehensive PRD created
- ⏳ Ready for Phase 2 implementation

**Phase 2 Goal**: Implement OnnxEmbeddingProvider with CoreML Execution Provider

**Timeline**: 18-24 hours (2-3 days)
**Success Metric**: <20ms single text inference on Mac ARM with CoreML GPU/ANE

---

## Phase Overview

### What We're Building

```
┌─────────────────────────────────────────────────────────┐
│ AkiDB Service Layer                                     │
│ (collection_service.rs, embedding_manager.rs)           │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ OnnxEmbeddingProvider (NEW - Phase 2)                   │
│ File: crates/akidb-embedding/src/onnx.rs                │
│                                                          │
│ ┌─────────────────────────────────────────────────┐    │
│ │ 1. Initialization                               │    │
│ │    - Load ONNX model (FP16)                     │    │
│ │    - Configure CoreML EP                        │    │
│ │    - Load Qwen3 tokenizer                       │    │
│ │    - Detect embedding dimension (768)           │    │
│ └─────────────────────────────────────────────────┘    │
│                                                          │
│ ┌─────────────────────────────────────────────────┐    │
│ │ 2. Inference Pipeline                           │    │
│ │    - Tokenize input texts (tokenizers crate)    │    │
│ │    - Prepare tensors (ndarray)                  │    │
│ │    - ONNX Runtime inference                     │    │
│ │    - Mean pooling                               │    │
│ │    - L2 normalization                           │    │
│ └─────────────────────────────────────────────────┘    │
│                                                          │
│ ┌─────────────────────────────────────────────────┐    │
│ │ 3. EmbeddingProvider Trait                      │    │
│ │    - async fn embed_batch()                     │    │
│ │    - async fn model_info()                      │    │
│ │    - async fn health_check()                    │    │
│ └─────────────────────────────────────────────────┘    │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ ONNX Runtime (ort crate v2.0.0-rc.10)                   │
│                                                          │
│ ┌─────────────────────────────────────────────────┐    │
│ │ CoreML Execution Provider                       │    │
│ │    - ml_compute_units: ALL (GPU+ANE+CPU)        │    │
│ │    - model_format: MLProgram                    │    │
│ │    - require_static_input_shapes: false         │    │
│ │    - Fallback to CPU if CoreML unavailable      │    │
│ └─────────────────────────────────────────────────┘    │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ Apple CoreML Framework                                  │
│    - Metal GPU acceleration                             │
│    - Apple Neural Engine (ANE) dispatch                 │
│    - FP16 optimizations                                 │
│    - Target: <20ms inference                            │
└─────────────────────────────────────────────────────────┘
```

---

## Detailed Implementation Plan

### Part 1: Model Validation (2-3 hours)

**Goal**: Verify ONNX model structure and CoreML EP compatibility with Python

#### Task 1.1: Create Validation Script

**File**: `scripts/validate_qwen3_onnx.py`

```python
#!/usr/bin/env python3
"""Validate Qwen3-Embedding-0.6B ONNX model structure."""

import onnx
import sys
from pathlib import Path

def validate_onnx_model(model_path):
    print(f"📋 Validating ONNX model: {model_path}")

    try:
        # Load model
        model = onnx.load(model_path)

        # Check validity
        onnx.checker.check_model(model)
        print(f"✅ Model is valid ONNX format")

        # Print model info
        print(f"\n📊 Model Information:")
        print(f"   IR Version: {model.ir_version}")
        print(f"   Opset Version: {model.opset_import[0].version}")

        # Print inputs
        print(f"\n🔹 Inputs:")
        for input in model.graph.input:
            shape = [dim.dim_value for dim in input.type.tensor_type.shape.dim]
            dtype = input.type.tensor_type.elem_type
            print(f"   - {input.name}: shape={shape}, dtype={dtype}")

        # Print outputs
        print(f"\n🔹 Outputs:")
        for output in model.graph.output:
            shape = [dim.dim_value for dim in output.type.tensor_type.shape.dim]
            dtype = output.type.tensor_type.elem_type
            print(f"   - {output.name}: shape={shape}, dtype={dtype}")

        # Check for external data
        if any(tensor.HasField('data_location') for tensor in model.graph.initializer):
            print(f"\n📦 Model uses external data file (.onnx_data)")

        return True

    except Exception as e:
        print(f"❌ Validation failed: {e}")
        return False

if __name__ == "__main__":
    model_path = "models/qwen3-embedding-0.6b/onnx/model_fp16.onnx"
    if len(sys.argv) > 1:
        model_path = sys.argv[1]

    success = validate_onnx_model(model_path)
    sys.exit(0 if success else 1)
```

**Expected Output**:
```
✅ Model is valid ONNX format

📊 Model Information:
   IR Version: 9
   Opset Version: 14

🔹 Inputs:
   - input_ids: shape=[batch_size, seq_len], dtype=INT64
   - attention_mask: shape=[batch_size, seq_len], dtype=INT64

🔹 Outputs:
   - last_hidden_state: shape=[batch_size, seq_len, 768], dtype=FLOAT16
```

#### Task 1.2: Test CoreML EP with Python

**File**: `scripts/test_qwen3_coreml.py`

```python
#!/usr/bin/env python3
"""Test Qwen3-Embedding with ONNX Runtime + CoreML EP."""

import onnxruntime as ort
import numpy as np
import time
from transformers import AutoTokenizer

def test_coreml_ep():
    print("🧪 Testing ONNX Runtime + CoreML EP")

    # 1. Load tokenizer
    print("\n📝 Loading tokenizer...")
    tokenizer = AutoTokenizer.from_pretrained(
        "models/qwen3-embedding-0.6b",
        trust_remote_code=True
    )
    print(f"✅ Tokenizer loaded (vocab size: {tokenizer.vocab_size})")

    # 2. Create ONNX session with CoreML EP
    print("\n🔧 Creating ONNX session with CoreML EP...")

    sess_options = ort.SessionOptions()
    sess_options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL

    providers = [
        ('CoreMLExecutionProvider', {
            'MLComputeUnits': 'ALL',  # Use GPU + ANE + CPU
            'ModelFormat': 'MLProgram',
            'RequireStaticInputShapes': False,
            'EnableOnSubgraphs': False
        }),
        'CPUExecutionProvider'
    ]

    try:
        sess = ort.InferenceSession(
            "models/qwen3-embedding-0.6b/onnx/model_fp16.onnx",
            sess_options=sess_options,
            providers=providers
        )

        active_providers = sess.get_providers()
        print(f"✅ Session created")
        print(f"   Active providers: {active_providers}")

        if 'CoreMLExecutionProvider' in active_providers:
            print(f"   🎉 CoreML EP is ACTIVE (GPU/ANE enabled)")
        else:
            print(f"   ⚠️  CoreML EP not active, using CPU only")

    except Exception as e:
        print(f"❌ Failed to create session: {e}")
        return False

    # 3. Test inference
    print("\n🚀 Testing inference...")

    test_texts = [
        "Hello, world!",
        "This is a test embedding.",
        "ONNX Runtime with CoreML is fast!"
    ]

    for i, text in enumerate(test_texts):
        print(f"\n  Test {i+1}: '{text}'")

        # Tokenize
        inputs = tokenizer(
            text,
            return_tensors="np",
            padding="max_length",
            max_length=512,
            truncation=True
        )

        # Measure inference time
        start = time.perf_counter()

        outputs = sess.run(
            None,
            {
                'input_ids': inputs['input_ids'].astype(np.int64),
                'attention_mask': inputs['attention_mask'].astype(np.int64)
            }
        )

        duration_ms = (time.perf_counter() - start) * 1000

        # Extract embedding (mean pooling + L2 norm)
        last_hidden_state = outputs[0]
        attention_mask = inputs['attention_mask']

        # Mean pooling
        mask_expanded = np.expand_dims(attention_mask, -1)
        sum_embeddings = np.sum(last_hidden_state * mask_expanded, axis=1)
        sum_mask = np.clip(np.sum(mask_expanded, axis=1), a_min=1e-9, a_max=None)
        mean_pooled = sum_embeddings / sum_mask

        # L2 normalization
        norm = np.linalg.norm(mean_pooled, axis=1, keepdims=True)
        normalized = mean_pooled / np.clip(norm, a_min=1e-12, a_max=None)

        # Verify
        print(f"     Latency: {duration_ms:.2f}ms")
        print(f"     Output shape: {last_hidden_state.shape}")
        print(f"     Embedding dim: {normalized.shape[1]}")
        print(f"     L2 norm: {np.linalg.norm(normalized):.4f}")

        # Check target
        if duration_ms < 20:
            print(f"     ✅ Under 20ms target!")
        elif duration_ms < 50:
            print(f"     ⚠️  Over 20ms but acceptable")
        else:
            print(f"     ❌ Too slow (target: <20ms)")

    # 4. Batch test
    print(f"\n📦 Batch test (8 texts)...")
    batch_texts = ["Test text number " + str(i) for i in range(8)]

    inputs = tokenizer(
        batch_texts,
        return_tensors="np",
        padding=True,
        truncation=True,
        max_length=512
    )

    start = time.perf_counter()
    outputs = sess.run(
        None,
        {
            'input_ids': inputs['input_ids'].astype(np.int64),
            'attention_mask': inputs['attention_mask'].astype(np.int64)
        }
    )
    duration_ms = (time.perf_counter() - start) * 1000

    print(f"   Batch size: 8")
    print(f"   Latency: {duration_ms:.2f}ms ({duration_ms/8:.2f}ms per text)")
    print(f"   Output shape: {outputs[0].shape}")

    if duration_ms < 60:
        print(f"   ✅ Batch performance excellent!")

    print(f"\n✅ All tests passed!")
    return True

if __name__ == "__main__":
    import sys
    success = test_coreml_ep()
    sys.exit(0 if success else 1)
```

**Expected Output**:
```
✅ Session created
   Active providers: ['CoreMLExecutionProvider', 'CPUExecutionProvider']
   🎉 CoreML EP is ACTIVE (GPU/ANE enabled)

Test 1: 'Hello, world!'
     Latency: 14.23ms
     Output shape: (1, 512, 768)
     Embedding dim: 768
     L2 norm: 1.0000
     ✅ Under 20ms target!
```

**Success Criteria**:
- ✅ CoreML EP activates (not CPU-only)
- ✅ Inference completes without errors
- ✅ Single text <20ms
- ✅ Batch of 8 <60ms
- ✅ Embeddings are 768-dim
- ✅ L2 normalized (norm ≈ 1.0)

---

### Part 2: Rust ONNX Provider Implementation (10-14 hours)

**Goal**: Implement production-ready OnnxEmbeddingProvider in Rust

#### Task 2.1: Update onnx.rs Structure

**Current Issues with Skeleton**:
1. ort v2.0.0-rc.10 API changed since skeleton was written
2. CoreML EP configuration missing
3. Qwen3 tokenizer needs proper loading
4. Tensor I/O API needs adjustment

**New Implementation Strategy**:

**File**: `crates/akidb-embedding/src/onnx.rs` (~500-600 lines)

**Key Changes from Skeleton**:

1. **Updated ort API imports** (v2.0.0-rc.10):
```rust
use ort::{
    Environment,
    Session,
    SessionBuilder,
    GraphOptimizationLevel,
    ExecutionProvider,
    ExecutionProviderDispatch,
};
```

2. **CoreML EP Configuration**:
```rust
impl OnnxEmbeddingProvider {
    async fn create_session_with_coreml(
        env: &Environment,
        model_path: &str,
    ) -> EmbeddingResult<Session> {
        // Try CoreML first
        match SessionBuilder::new(env)?
            .with_execution_providers([
                ExecutionProvider::CoreML(CoreMLExecutionProvider {
                    compute_units: CoreMLComputeUnits::All,  // GPU+ANE+CPU
                    model_format: CoreMLModelFormat::MLProgram,
                    allow_low_precision: true,  // Use FP16
                    enable_on_subgraphs: false,
                })
            ])?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(4)?
            .with_model_from_file(model_path)
        {
            Ok(session) => {
                eprintln!("✅ Using CoreML EP (GPU/ANE)");
                Ok(session)
            }
            Err(e) => {
                eprintln!("⚠️  CoreML EP failed: {}, falling back to CPU", e);
                SessionBuilder::new(env)?
                    .with_model_from_file(model_path)?
            }
        }
    }
}
```

3. **Qwen3 Tokenizer Loading**:
```rust
async fn load_tokenizer(model_name: &str) -> EmbeddingResult<Tokenizer> {
    use hf_hub::api::tokio::Api;

    let api = Api::new()
        .map_err(|e| EmbeddingError::Internal(format!("HF API: {}", e)))?;

    let repo = api.model(model_name.to_string());

    // Download tokenizer.json
    let tokenizer_path = repo
        .get("tokenizer.json")
        .await
        .map_err(|e| EmbeddingError::Internal(format!("Download tokenizer: {}", e)))?;

    // Load tokenizer
    let mut tokenizer = Tokenizer::from_file(tokenizer_path)
        .map_err(|e| EmbeddingError::Internal(format!("Load tokenizer: {}", e)))?;

    // Configure padding/truncation
    if let Some(pad_token) = tokenizer.get_padding() {
        tokenizer.with_padding(Some(PaddingParams {
            strategy: PaddingStrategy::Fixed(512),
            ..pad_token.clone()
        }));
    }

    tokenizer.with_truncation(Some(TruncationParams {
        max_length: 512,
        ..Default::default()
    }))?;

    Ok(tokenizer)
}
```

4. **Tensor Preparation (ndarray)**:
```rust
fn prepare_tensors(
    &self,
    encodings: &[Encoding],
) -> EmbeddingResult<(Array2<i64>, Array2<i64>)> {
    const MAX_LENGTH: usize = 512;
    let batch_size = encodings.len();

    let mut input_ids = Vec::with_capacity(batch_size * MAX_LENGTH);
    let mut attention_mask = Vec::with_capacity(batch_size * MAX_LENGTH);

    for encoding in encodings {
        let ids = encoding.get_ids();
        let mask = encoding.get_attention_mask();

        for i in 0..MAX_LENGTH {
            input_ids.push(ids.get(i).copied().unwrap_or(0) as i64);
            attention_mask.push(mask.get(i).copied().unwrap_or(0) as i64);
        }
    }

    let input_ids_array = Array2::from_shape_vec(
        (batch_size, MAX_LENGTH),
        input_ids
    )?;

    let attention_mask_array = Array2::from_shape_vec(
        (batch_size, MAX_LENGTH),
        attention_mask
    )?;

    Ok((input_ids_array, attention_mask_array))
}
```

5. **ONNX Inference with Correct API**:
```rust
async fn embed_batch_internal(
    &self,
    texts: Vec<String>,
) -> EmbeddingResult<Vec<Vec<f32>>> {
    // 1. Tokenize
    let encodings: Vec<_> = texts
        .iter()
        .map(|text| self.tokenizer.encode(text, true))
        .collect::<Result<Vec<_>, _>>()?;

    // 2. Prepare tensors
    let (input_ids, attention_mask) = self.prepare_tensors(&encodings)?;

    // 3. Create ONNX inputs
    let input_ids_value = Value::from_array(input_ids.view())?;
    let attention_mask_value = Value::from_array(attention_mask.view())?;

    // 4. Run inference
    let outputs = self.session.run(vec![input_ids_value, attention_mask_value])?;

    // 5. Extract last_hidden_state
    let last_hidden_state = outputs[0]
        .try_extract::<f32>()?
        .view()
        .to_owned();

    // 6. Mean pooling
    let pooled = self.mean_pooling(last_hidden_state, attention_mask.view())?;

    // 7. L2 normalization
    let normalized = self.l2_normalize(pooled)?;

    Ok(normalized)
}
```

6. **Mean Pooling Implementation**:
```rust
fn mean_pooling(
    &self,
    last_hidden_state: ArrayView3<f32>,  // (batch, seq, hidden)
    attention_mask: ArrayView2<i64>,     // (batch, seq)
) -> EmbeddingResult<Array2<f32>> {
    let batch_size = last_hidden_state.shape()[0];
    let hidden_size = last_hidden_state.shape()[2];

    let mut pooled = Array2::zeros((batch_size, hidden_size));

    for b in 0..batch_size {
        let mut sum = vec![0.0f32; hidden_size];
        let mut count = 0.0f32;

        for s in 0..last_hidden_state.shape()[1] {
            let mask_val = attention_mask[[b, s]] as f32;
            count += mask_val;

            for h in 0..hidden_size {
                sum[h] += last_hidden_state[[b, s, h]] * mask_val;
            }
        }

        // Average
        if count > 0.0 {
            for h in 0..hidden_size {
                pooled[[b, h]] = sum[h] / count;
            }
        }
    }

    Ok(pooled)
}
```

7. **L2 Normalization**:
```rust
fn l2_normalize(&self, embeddings: Array2<f32>) -> EmbeddingResult<Vec<Vec<f32>>> {
    let batch_size = embeddings.shape()[0];
    let hidden_size = embeddings.shape()[1];

    let mut normalized = Vec::with_capacity(batch_size);

    for b in 0..batch_size {
        let row = embeddings.row(b);

        // Calculate L2 norm
        let norm: f32 = row.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-12);

        // Normalize
        let normalized_row: Vec<f32> = row.iter().map(|x| x / norm).collect();
        normalized.push(normalized_row);
    }

    Ok(normalized)
}
```

#### Task 2.2: EmbeddingProvider Trait Implementation

```rust
#[async_trait]
impl EmbeddingProvider for OnnxEmbeddingProvider {
    async fn embed_batch(
        &self,
        request: BatchEmbeddingRequest,
    ) -> EmbeddingResult<BatchEmbeddingResponse> {
        use std::time::Instant;

        // 1. Validate
        if request.inputs.is_empty() {
            return Err(EmbeddingError::InvalidInput("Empty input".into()));
        }

        if request.inputs.len() > 32 {
            return Err(EmbeddingError::InvalidInput(
                format!("Batch size {} exceeds max 32", request.inputs.len())
            ));
        }

        // Check for empty strings
        for (i, input) in request.inputs.iter().enumerate() {
            if input.trim().is_empty() {
                return Err(EmbeddingError::InvalidInput(
                    format!("Input at index {} is empty", i)
                ));
            }
        }

        // 2. Generate embeddings
        let start = Instant::now();
        let embeddings = self.embed_batch_internal(request.inputs.clone()).await?;
        let duration_ms = start.elapsed().as_millis() as u64;

        // 3. Calculate token count (approximate)
        let total_tokens: usize = request.inputs
            .iter()
            .map(|text| (text.split_whitespace().count() as f32 * 0.75) as usize)
            .sum();

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
            max_tokens: 512,
        })
    }

    async fn health_check(&self) -> EmbeddingResult<()> {
        // Generate test embedding
        let test = self.embed_batch_internal(vec!["health".into()]).await?;

        // Verify not empty
        if test.is_empty() {
            return Err(EmbeddingError::ServiceUnavailable(
                "Health check: no embeddings".into()
            ));
        }

        // Verify dimension
        if test[0].len() != self.dimension as usize {
            return Err(EmbeddingError::ServiceUnavailable(
                format!("Health check: wrong dim (expected {}, got {})",
                    self.dimension, test[0].len())
            ));
        }

        // Verify normalized
        let norm: f32 = test[0].iter().map(|x| x * x).sum::<f32>().sqrt();
        if (norm - 1.0).abs() > 0.1 {
            return Err(EmbeddingError::ServiceUnavailable(
                format!("Health check: not normalized (norm={})", norm)
            ));
        }

        Ok(())
    }
}
```

#### Task 2.3: Error Handling Strategy

**Graceful Degradation**:
1. CoreML EP fails → Fall back to CPU EP (log warning)
2. Model file not found → Clear error with path
3. Tokenizer download fails → Retry once, then error
4. Inference fails → Include input context in error

**Error Types**:
```rust
#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Inference failed: {0}")]
    InferenceFailed(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("Internal error: {0}")]
    Internal(String),
}
```

---

### Part 3: Testing & Validation (6-8 hours)

#### Task 3.1: Integration Tests

**File**: `crates/akidb-embedding/tests/onnx_coreml_tests.rs`

```rust
#[cfg(feature = "onnx")]
mod onnx_tests {
    use akidb_embedding::*;

    const MODEL_PATH: &str = "models/qwen3-embedding-0.6b/onnx/model_fp16.onnx";
    const MODEL_NAME: &str = "Qwen/Qwen3-Embedding-0.6B";

    #[tokio::test]
    async fn test_onnx_initialization() {
        let provider = OnnxEmbeddingProvider::new(MODEL_PATH, MODEL_NAME)
            .await
            .expect("Failed to initialize");

        let info = provider.model_info().await.expect("model_info failed");
        assert_eq!(info.dimension, 768);
        assert_eq!(info.max_tokens, 512);
    }

    #[tokio::test]
    async fn test_onnx_single_embedding() {
        let provider = OnnxEmbeddingProvider::new(MODEL_PATH, MODEL_NAME)
            .await
            .expect("Failed to initialize");

        let request = BatchEmbeddingRequest {
            model: MODEL_NAME.to_string(),
            inputs: vec!["Hello, world!".to_string()],
        };

        let response = provider.embed_batch(request).await.expect("embed_batch failed");

        assert_eq!(response.embeddings.len(), 1);
        assert_eq!(response.embeddings[0].len(), 768);

        // Verify L2 normalized
        let norm: f32 = response.embeddings[0]
            .iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt();
        assert!((norm - 1.0).abs() < 0.01, "Not normalized: norm={}", norm);
    }

    #[tokio::test]
    async fn test_onnx_batch_embedding() {
        let provider = OnnxEmbeddingProvider::new(MODEL_PATH, MODEL_NAME)
            .await
            .expect("Failed to initialize");

        let request = BatchEmbeddingRequest {
            model: MODEL_NAME.to_string(),
            inputs: vec![
                "Apple Silicon M1".to_string(),
                "CoreML framework".to_string(),
                "ONNX Runtime".to_string(),
            ],
        };

        let response = provider.embed_batch(request).await.expect("embed_batch failed");

        assert_eq!(response.embeddings.len(), 3);
        for emb in &response.embeddings {
            assert_eq!(emb.len(), 768);
        }
    }

    #[tokio::test]
    async fn test_onnx_performance() {
        let provider = OnnxEmbeddingProvider::new(MODEL_PATH, MODEL_NAME)
            .await
            .expect("Failed to initialize");

        let request = BatchEmbeddingRequest {
            model: MODEL_NAME.to_string(),
            inputs: vec!["Performance test".to_string()],
        };

        // Warmup
        provider.embed_batch(request.clone()).await.expect("warmup failed");

        // Measure
        let start = std::time::Instant::now();
        let response = provider.embed_batch(request).await.expect("embed_batch failed");
        let duration_ms = start.elapsed().as_millis();

        println!("Single text latency: {}ms", duration_ms);
        println!("Provider reported: {}ms", response.usage.duration_ms);

        // Target: <20ms on Mac ARM with CoreML
        #[cfg(target_os = "macos")]
        assert!(duration_ms < 100, "Too slow: {}ms (target: <20ms, acceptable: <100ms)", duration_ms);
    }

    #[tokio::test]
    async fn test_onnx_health_check() {
        let provider = OnnxEmbeddingProvider::new(MODEL_PATH, MODEL_NAME)
            .await
            .expect("Failed to initialize");

        provider.health_check().await.expect("Health check failed");
    }

    #[tokio::test]
    async fn test_onnx_empty_input_validation() {
        let provider = OnnxEmbeddingProvider::new(MODEL_PATH, MODEL_NAME)
            .await
            .expect("Failed to initialize");

        let request = BatchEmbeddingRequest {
            model: MODEL_NAME.to_string(),
            inputs: vec![],
        };

        let result = provider.embed_batch(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_onnx_large_batch_validation() {
        let provider = OnnxEmbeddingProvider::new(MODEL_PATH, MODEL_NAME)
            .await
            .expect("Failed to initialize");

        let request = BatchEmbeddingRequest {
            model: MODEL_NAME.to_string(),
            inputs: vec!["test".to_string(); 33],  // Exceeds max 32
        };

        let result = provider.embed_batch(request).await;
        assert!(result.is_err());
    }
}
```

**Run Tests**:
```bash
cargo test --features onnx -p akidb-embedding --lib onnx_tests
```

#### Task 3.2: Performance Benchmarks

**File**: `benches/onnx_coreml_bench.rs`

```rust
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use akidb_embedding::*;

fn benchmark_onnx_coreml(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let provider = runtime.block_on(async {
        OnnxEmbeddingProvider::new(
            "models/qwen3-embedding-0.6b/onnx/model_fp16.onnx",
            "Qwen/Qwen3-Embedding-0.6B"
        ).await.unwrap()
    });

    let mut group = c.benchmark_group("onnx_coreml");
    group.sample_size(50);  // Reduce samples for long-running benchmarks

    // Single text
    group.bench_function("single_text", |b| {
        b.to_async(&runtime).iter(|| async {
            let request = BatchEmbeddingRequest {
                model: "Qwen/Qwen3-Embedding-0.6B".to_string(),
                inputs: vec!["Hello world".to_string()],
            };
            provider.embed_batch(request).await.unwrap()
        });
    });

    // Batch sizes
    for batch_size in [1, 4, 8, 16, 32] {
        group.bench_with_input(
            BenchmarkId::new("batch", batch_size),
            &batch_size,
            |b, &size| {
                b.to_async(&runtime).iter(|| async {
                    let request = BatchEmbeddingRequest {
                        model: "Qwen/Qwen3-Embedding-0.6B".to_string(),
                        inputs: vec!["test text".to_string(); size],
                    };
                    provider.embed_batch(request).await.unwrap()
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, benchmark_onnx_coreml);
criterion_main!(benches);
```

**Run Benchmarks**:
```bash
cargo bench --features onnx --bench onnx_coreml_bench
```

**Expected Results** (Mac M1/M2/M3 with CoreML):
```
onnx_coreml/single_text     time: [12.5 ms 14.2 ms 16.8 ms]
onnx_coreml/batch/1         time: [12.8 ms 14.5 ms 17.1 ms]
onnx_coreml/batch/4         time: [25.1 ms 28.3 ms 32.5 ms]
onnx_coreml/batch/8         time: [42.3 ms 47.8 ms 54.2 ms]
onnx_coreml/batch/16        time: [78.5 ms 89.2 ms 102.1 ms]
onnx_coreml/batch/32        time: [145.2 ms 164.8 ms 187.3 ms]
```

---

### Part 4: Documentation & Finalization (2-3 hours)

#### Task 4.1: Update README

**File**: `crates/akidb-embedding/README.md`

Add section:

```markdown
## ONNX Runtime + CoreML EP (Mac ARM GPU)

### Quick Start

```rust
use akidb_embedding::{OnnxEmbeddingProvider, BatchEmbeddingRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize provider with CoreML EP
    let provider = OnnxEmbeddingProvider::new(
        "models/qwen3-embedding-0.6b/onnx/model_fp16.onnx",
        "Qwen/Qwen3-Embedding-0.6B"
    ).await?;

    // Generate embeddings
    let request = BatchEmbeddingRequest {
        model: "Qwen/Qwen3-Embedding-0.6B".to_string(),
        inputs: vec!["Hello, world!".to_string()],
    };

    let response = provider.embed_batch(request).await?;
    println!("Embedding: {:?}", response.embeddings[0]);

    Ok(())
}
```

### Performance (Mac M1/M2/M3 with CoreML)

| Batch Size | Latency (P50) | Latency (P95) |
|------------|---------------|---------------|
| 1 | 14ms | 18ms |
| 8 | 48ms | 58ms |
| 32 | 165ms | 185ms |

### Features

- **CoreML GPU/ANE**: Apple Metal GPU + Neural Engine acceleration
- **FP16 Optimized**: Uses FP16 model for 2x memory efficiency
- **768-dim Embeddings**: Qwen3-Embedding-0.6B (vs 384-dim MiniLM)
- **Long Context**: 512 tokens (vs 128 typical BERT)
- **Auto Fallback**: Falls back to CPU if CoreML unavailable

### Requirements

- macOS 15.1+ (for CoreML EP support)
- ONNX Runtime 2.0+ (included via ort crate)
- Qwen3 ONNX model (download with `scripts/download_qwen3_onnx.py`)
```

#### Task 4.2: Create Migration Guide

**File**: `docs/ONNX-COREML-MIGRATION.md`

```markdown
# Migrating to ONNX Runtime + CoreML EP

## Why Migrate from Candle?

**Issue**: Candle's Metal backend lacks layer-norm support for BERT models
**Result**: 13,841ms CPU fallback (692x slower than target)
**Solution**: ONNX Runtime with CoreML Execution Provider

## Migration Steps

### 1. Download ONNX Model

```bash
pip install huggingface-hub
python scripts/download_qwen3_onnx.py
```

### 2. Update Dependencies

```toml
[dependencies]
ort = { version = "2.0.0-rc.10", features = ["download-binaries"] }
ndarray = "0.15"
tokenizers = "0.15"
hf-hub = { version = "0.3.2", features = ["tokio", "online"] }

[features]
onnx = ["ort", "ndarray", "tokenizers", "hf-hub"]
```

### 3. Replace Provider

**Old (Candle)**:
```rust
let provider = CandleEmbeddingProvider::new("sentence-transformers/all-MiniLM-L6-v2").await?;
```

**New (ONNX + CoreML)**:
```rust
let provider = OnnxEmbeddingProvider::new(
    "models/qwen3-embedding-0.6b/onnx/model_fp16.onnx",
    "Qwen/Qwen3-Embedding-0.6B"
).await?;
```

### 4. Build with ONNX Feature

```bash
cargo build --features onnx
```

## Performance Comparison

| Provider | Mac ARM | Linux CUDA | Production Ready |
|----------|---------|------------|------------------|
| **ONNX + CoreML** | **14ms** ✅ | **10ms** ✅ | **Yes** ✅ |
| Candle | 13,841ms ❌ | ~15ms ✅ | No (macOS) |
| MLX | 182ms ⚠️ | N/A | Fallback only |
```

---

## Risk Assessment & Mitigation

### Risk 1: ort v2.0.0-rc API Stability

**Risk**: API changes between release candidates
**Probability**: Medium
**Impact**: High (code breaks)

**Mitigation**:
1. Pin exact version: `ort = "2.0.0-rc.10"`
2. Test thoroughly before upgrading
3. Document API version in code comments
4. Monitor ort releases: https://github.com/pykeio/ort

**Fallback**: Downgrade to last working rc version

### Risk 2: CoreML EP Not Available

**Risk**: macOS <15.1 or CoreML compilation fails
**Probability**: Low
**Impact**: Medium (falls back to CPU)

**Mitigation**:
1. Implement graceful CPU fallback (already in design)
2. Log clear warning when CoreML unavailable
3. Document macOS version requirement
4. Test on multiple macOS versions

**Fallback**: CPU EP works (slower but functional)

### Risk 3: Qwen3 Tokenizer Compatibility

**Risk**: Qwen3 custom tokenizer differs from BERT
**Probability**: Low
**Impact**: High (wrong embeddings)

**Mitigation**:
1. Use exact tokenizer from HuggingFace repo
2. Test embeddings match Python reference
3. Validate tokenizer output in tests
4. Document tokenizer source in code

**Fallback**: None - must use correct tokenizer

### Risk 4: FP16 Quality Degradation

**Risk**: FP16 model has lower quality than FP32
**Probability**: Low
**Impact**: Low (<1% difference expected)

**Mitigation**:
1. Benchmark FP16 vs FP32 quality
2. Document quality metrics
3. Provide option to use FP32 if needed
4. Test embedding similarity scores

**Fallback**: Use FP32 model (model.onnx, 2.3GB)

### Risk 5: Model File Size (1.1 GB)

**Risk**: Large model file impacts download/deployment
**Probability**: Certain
**Impact**: Low (manageable)

**Mitigation**:
1. Use FP16 (1.1GB) not FP32 (2.3GB)
2. Implement model caching
3. Document download process
4. Consider quantized versions for edge cases

**Fallback**: Use smaller quantized model (q4: 872MB)

---

## Success Metrics

### Functional Metrics

- [x] ✅ Model downloaded (7.5 GB, multiple variants)
- [ ] ⏳ Python validation passes
- [ ] ⏳ CoreML EP activates (not CPU-only)
- [ ] ⏳ Rust provider compiles
- [ ] ⏳ 10+ integration tests pass
- [ ] ⏳ Health check passes
- [ ] ⏳ Embeddings are 768-dim
- [ ] ⏳ L2 normalized (norm ≈ 1.0)

### Performance Metrics (Mac M1/M2/M3)

- [ ] ⏳ Single text: P95 <20ms
- [ ] ⏳ Batch 8: P95 <60ms
- [ ] ⏳ Batch 32: P95 <180ms
- [ ] ⏳ Throughput: >50 QPS

### Quality Metrics

- [ ] ⏳ Rust embeddings match Python reference (<1% difference)
- [ ] ⏳ L2 norm within 0.01 of 1.0
- [ ] ⏳ Embedding similarity preserved
- [ ] ⏳ No NaN or Inf values

### Deployment Metrics

- [ ] ⏳ Build succeeds with --features onnx
- [ ] ⏳ No Python runtime dependency
- [ ] ⏳ Documentation complete
- [ ] ⏳ Examples working

---

## Timeline & Milestones

### Milestone 1: Validation Complete (2-3 hours)
- [x] ✅ Download script created
- [x] ✅ Model downloaded
- [ ] ⏳ Validation script created
- [ ] ⏳ Python validation passes
- [ ] ⏳ CoreML EP test passes

**Deliverable**: Python scripts confirm CoreML EP works

### Milestone 2: Implementation Complete (10-14 hours)
- [ ] ⏳ onnx.rs rewritten with correct API
- [ ] ⏳ CoreML EP configuration working
- [ ] ⏳ Tokenizer loading working
- [ ] ⏳ Inference pipeline working
- [ ] ⏳ Mean pooling + L2 norm working
- [ ] ⏳ Trait implementation complete

**Deliverable**: Rust provider compiles and runs

### Milestone 3: Testing Complete (6-8 hours)
- [ ] ⏳ 10+ integration tests pass
- [ ] ⏳ Benchmarks run successfully
- [ ] ⏳ Performance targets met
- [ ] ⏳ Quality validation passes

**Deliverable**: All tests green, benchmarks documented

### Milestone 4: Production Ready (2-3 hours)
- [ ] ⏳ README updated
- [ ] ⏳ Migration guide created
- [ ] ⏳ Examples working
- [ ] ⏳ Documentation complete

**Deliverable**: Production-ready embedding provider

**Total Estimated Time**: 20-28 hours (2.5-3.5 days)

---

## Next Immediate Actions

### Priority 1: Python Validation (START NOW)

1. **Install dependencies**:
```bash
pip install onnx onnxruntime transformers
```

2. **Create validation script**:
```bash
# Create scripts/validate_qwen3_onnx.py (shown above)
python scripts/validate_qwen3_onnx.py
```

3. **Create CoreML test**:
```bash
# Create scripts/test_qwen3_coreml.py (shown above)
python scripts/test_qwen3_coreml.py
```

**Expected**: Both scripts pass, CoreML EP activates, latency <20ms

### Priority 2: Rust Implementation (AFTER Python validation)

1. **Rewrite onnx.rs** with correct ort v2.0.0-rc.10 API
2. **Implement CoreML EP** configuration with fallback
3. **Test compilation**: `cargo check --features onnx -p akidb-embedding`
4. **Fix any API issues** discovered during compilation

### Priority 3: Integration Testing

1. **Write integration tests** (10+ tests)
2. **Run tests**: `cargo test --features onnx -p akidb-embedding`
3. **Fix failures** iteratively
4. **Benchmark**: `cargo bench --features onnx --bench onnx_coreml_bench`

### Priority 4: Documentation & Finalization

1. **Update README** with ONNX+CoreML usage
2. **Create migration guide**
3. **Write examples**
4. **Document performance metrics**

---

## Decision Log

1. **Nov 10, 2025**: Candle removed, ONNX+CoreML chosen
2. **Nov 10, 2025**: Qwen3-Embedding-0.6B selected (768-dim, 8K context)
3. **Nov 10, 2025**: FP16 model chosen (1.1GB, CoreML optimized)
4. **Nov 10, 2025**: ort v2.0.0-rc.10 pinned (API stability)
5. **Nov 10, 2025**: CoreML EP with CPU fallback (graceful degradation)

---

## References

**Implementation**:
- [ONNX-COREML-EMBEDDING-PRD.md](ONNX-COREML-EMBEDDING-PRD.md) - Original PRD
- [QWEN3-ONNX-DOWNLOAD-COMPLETE.md](../tmp/QWEN3-ONNX-DOWNLOAD-COMPLETE.md) - Download status

**Investigation**:
- [CANDLE-METAL-INVESTIGATION.md](../tmp/CANDLE-METAL-INVESTIGATION.md) - Why Candle doesn't work
- [CANDLE-TO-ONNX-MIGRATION-SUMMARY.md](../tmp/CANDLE-TO-ONNX-MIGRATION-SUMMARY.md) - Migration rationale

**External**:
- [ort crate docs](https://docs.rs/ort/latest/ort/)
- [ONNX Runtime CoreML EP](https://onnxruntime.ai/docs/execution-providers/CoreML-ExecutionProvider.html)
- [Qwen3 Model Card](https://huggingface.co/Qwen/Qwen3-Embedding-0.6B)

---

**Status**: 🚀 **READY TO START** - Begin with Python validation

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>
