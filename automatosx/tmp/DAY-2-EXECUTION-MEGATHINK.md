# Day 2 Execution Megathink - MiniLM Validation & Rust Implementation

**Date**: November 11, 2025 (Day 2)
**Context**: Day 1 complete with comprehensive analysis, ready to execute MiniLM validation
**Goal**: Validate MiniLM achieves <20ms, then implement Rust ONNX provider
**Timeline**: 8-12 hours total (full day)

---

## Executive Summary

Day 1 established that ONNX Runtime is 117x faster than Candle, but Qwen3's large vocabulary (151K tokens) prevents CoreML EP from fully accelerating. The optimal path forward is to test **all-MiniLM-L6-v2** (30K vocab, 6 layers, 384-dim) which should achieve <20ms with full CoreML EP acceleration.

Today's plan:
1. **Morning (2-3 hours)**: Download MiniLM, validate performance
2. **Decision Point**: Based on results, choose implementation path
3. **Afternoon/Evening (6-9 hours)**: Implement Rust ONNX provider
4. **End of Day**: Working Rust implementation with tests

**Success Criteria**: Rust provider compiles, passes basic tests, achieves similar performance to Python baseline.

---

## Part 1: MiniLM Model Validation (Morning, 2-3 hours)

### Session 1.1: Search and Download MiniLM ONNX (30-60 min)

#### Step 1: Search HuggingFace for ONNX Exports

**Priority Order** (try in sequence):

```bash
# Option 1: Transformers.js repository (MOST LIKELY)
Repository: Xenova/all-MiniLM-L6-v2
URL: https://huggingface.co/Xenova/all-MiniLM-L6-v2
Expected: onnx/ directory with quantized variants
Probability: 90%

# Option 2: onnx-community repository
Repository: onnx-community/all-MiniLM-L6-v2
URL: https://huggingface.co/onnx-community/all-MiniLM-L6-v2
Expected: Full ONNX export
Probability: 70%

# Option 3: Optimized by Microsoft
Repository: microsoft/all-MiniLM-L6-v2-onnx
Expected: Optimized ONNX model
Probability: 50%
```

**Search Strategy**:

```bash
# Use HuggingFace search
1. Go to https://huggingface.co/models
2. Search: "all-MiniLM-L6-v2 onnx"
3. Filter: Models only
4. Look for "onnx" in model card tags
```

**Verification Checklist**:
- [ ] Model card exists
- [ ] Files include `onnx/model.onnx` or `model.onnx`
- [ ] Files include `tokenizer.json`
- [ ] Files include `config.json`
- [ ] Model size reasonable (50-200MB)

#### Step 2: Download Model

**If Xenova/all-MiniLM-L6-v2 exists** (most likely):

```python
# Create download script: scripts/download_minilm_onnx.py
from huggingface_hub import snapshot_download
from pathlib import Path

def download_minilm_onnx(output_dir="models/minilm-l6-v2"):
    """Download all-MiniLM-L6-v2 ONNX model."""
    print(f"📥 Downloading all-MiniLM-L6-v2 ONNX to {output_dir}...")

    # Try Xenova first (Transformers.js)
    try:
        snapshot_download(
            repo_id="Xenova/all-MiniLM-L6-v2",
            local_dir=output_dir,
            allow_patterns=["onnx/*", "*.json"],  # ONNX files + config
        )
        print(f"✅ Downloaded from Xenova/all-MiniLM-L6-v2")
        return output_dir
    except Exception as e:
        print(f"⚠️  Xenova repo failed: {e}")

    # Fallback to onnx-community
    try:
        snapshot_download(
            repo_id="onnx-community/all-MiniLM-L6-v2",
            local_dir=output_dir,
            repo_type="model"
        )
        print(f"✅ Downloaded from onnx-community")
        return output_dir
    except Exception as e:
        print(f"❌ onnx-community failed: {e}")
        return None

if __name__ == "__main__":
    download_minilm_onnx()
```

**Run Download**:

```bash
python3 scripts/download_minilm_onnx.py
```

**Expected Output**:
```
📥 Downloading all-MiniLM-L6-v2 ONNX to models/minilm-l6-v2...
Downloading: 100%|████████| 50.5M/50.5M [00:30<00:00, 1.68MB/s]
✅ Downloaded from Xenova/all-MiniLM-L6-v2

📦 Downloaded files:
   - onnx/model.onnx (45.2 MB)
   - onnx/model_quantized.onnx (11.7 MB)
   - tokenizer.json (466 KB)
   - config.json (571 B)

✅ All critical files present
```

**If No ONNX Export Found** (10% chance):

```bash
# Manual export using Optimum
pip install optimum[onnxruntime]

optimum-cli export onnx \
  --model sentence-transformers/all-MiniLM-L6-v2 \
  --task feature-extraction \
  --optimize O3 \
  --opset 14 \
  models/minilm-l6-v2/

# This will create:
# models/minilm-l6-v2/model.onnx
# models/minilm-l6-v2/tokenizer.json
# models/minilm-l6-v2/config.json
```

**Time Estimate**: 15-30 min (download) + 10-20 min (manual export if needed) = 30-60 min

#### Step 3: Validate Model Structure

**Update validation script to support MiniLM**:

```python
# Modify scripts/validate_qwen3_onnx.py to accept --model argument
# Already did this in previous session, just need to run:

python3 scripts/validate_qwen3_onnx.py \
  --model models/minilm-l6-v2/onnx/model.onnx
```

**Expected Output**:

```
======================================================================
ONNX Model Validation: models/minilm-l6-v2/onnx/model.onnx
======================================================================

📦 Loading model...
✅ Model loaded successfully

🔍 Validating model structure...
✅ Model structure is valid

📊 Model Information:
   IR Version: 7
   Producer: optimum 1.x

📥 Model Inputs:
   - input_ids
     Shape: ['batch_size', 'sequence_length']
     Type: INT64
   - attention_mask
     Shape: ['batch_size', 'sequence_length']
     Type: INT64
   - token_type_ids (optional)
     Shape: ['batch_size', 'sequence_length']
     Type: INT64

📤 Model Outputs:
   - last_hidden_state
     Shape: ['batch_size', 'sequence_length', 384]
     Type: FLOAT
   - pooler_output (optional)
     Shape: ['batch_size', 384]
     Type: FLOAT

🔧 Operators Used:
   - MatMul: 42
   - Add: 36
   - LayerNormalization: 13
   - Attention: 12
   - Gelu: 12
   - ... (total ~150 operators)

💾 External Data: No (all weights embedded)

📏 Model Size: 45.2 MB

✅ VALIDATION PASSED

📝 Key Findings:
   ✅ Vocabulary: 30,522 tokens (<16K limit, fits in CoreML EP)
   ✅ Hidden dimension: 384
   ✅ Layers: 6 (fast inference)
   ✅ No external data (single file)
```

**Critical Check**:
- Vocabulary MUST be <16,384 for CoreML EP
- Expected: 30,522 (BERT vocab) ✅

**Time Estimate**: 5 min

### Session 1.2: CoreML EP Performance Testing (60-90 min)

#### Step 1: Update Test Script for MiniLM

**Key Differences from Qwen3**:
1. **Pooling Strategy**: Mean pooling (not last-token)
2. **No KV Cache**: Simple BERT encoder (not decoder)
3. **Optional Inputs**: token_type_ids may be optional

**Update `test_qwen3_coreml.py`** to support mean pooling:

```python
# Add mean pooling function
def mean_pool(
    last_hidden_states: np.ndarray,
    attention_mask: np.ndarray
) -> np.ndarray:
    """
    Mean pooling with attention mask.

    Args:
        last_hidden_states: Shape (batch, seq_len, hidden_dim)
        attention_mask: Shape (batch, seq_len)

    Returns:
        embeddings: Shape (batch, hidden_dim)
    """
    # Expand mask to match hidden states
    mask_expanded = np.expand_dims(attention_mask, -1)  # (batch, seq_len, 1)

    # Multiply hidden states by mask
    masked_hidden = last_hidden_states * mask_expanded  # (batch, seq_len, hidden)

    # Sum over sequence length
    sum_hidden = masked_hidden.sum(axis=1)  # (batch, hidden)

    # Sum of mask (count non-padding tokens)
    sum_mask = mask_expanded.sum(axis=1)  # (batch, 1)

    # Avoid division by zero
    sum_mask = np.clip(sum_mask, a_min=1e-9, a_max=None)

    # Mean
    mean_pooled = sum_hidden / sum_mask  # (batch, hidden)

    return mean_pooled


def embed_texts_minilm(
    session: ort.InferenceSession,
    tokenizer,
    texts: List[str],
    max_length: int = 512,
    normalize: bool = True
) -> Tuple[np.ndarray, dict]:
    """
    Generate embeddings using MiniLM with mean pooling.
    """
    # Tokenize
    start_tokenize = time.perf_counter()
    inputs = tokenizer(
        texts,
        padding=True,
        truncation=True,
        max_length=max_length,
        return_tensors="np"
    )
    tokenize_time = time.perf_counter() - start_tokenize

    # Prepare ONNX inputs (simpler than Qwen3)
    onnx_inputs = {
        'input_ids': inputs['input_ids'].astype(np.int64),
        'attention_mask': inputs['attention_mask'].astype(np.int64),
    }

    # token_type_ids optional (try with, fallback without)
    if 'token_type_ids' in inputs:
        onnx_inputs['token_type_ids'] = inputs['token_type_ids'].astype(np.int64)

    # Run inference
    start_inference = time.perf_counter()
    outputs = session.run(None, onnx_inputs)
    inference_time = time.perf_counter() - start_inference

    # Extract last_hidden_state (first output)
    last_hidden_states = outputs[0]  # Shape: (batch, seq_len, 384)

    # Apply mean pooling
    start_pool = time.perf_counter()
    embeddings = mean_pool(last_hidden_states, inputs['attention_mask'])
    pool_time = time.perf_counter() - start_pool

    # L2 normalize
    if normalize:
        embeddings = normalize_embeddings(embeddings)

    total_time = time.perf_counter() - start_tokenize

    metadata = {
        'tokenize_ms': tokenize_time * 1000,
        'inference_ms': inference_time * 1000,
        'pool_ms': pool_time * 1000,
        'total_ms': total_time * 1000,
        'batch_size': len(texts),
        'seq_len': inputs['input_ids'].shape[1],
    }

    return embeddings, metadata
```

**Create new test script** or modify existing:

```bash
# Option 1: Create separate script
cp scripts/test_qwen3_coreml.py scripts/test_minilm_coreml.py
# Then modify to use mean_pool instead of last_token_pool

# Option 2: Make existing script configurable
python3 scripts/test_qwen3_coreml.py \
  --model models/minilm-l6-v2/onnx/model.onnx \
  --pooling mean \
  --dimension 384
```

#### Step 2: Run Performance Tests

**Test Suite**:

```bash
# Run comprehensive CoreML EP test
python3 scripts/test_minilm_coreml.py
```

**Expected Output (Target Results)**:

```
======================================================================
all-MiniLM-L6-v2 ONNX Runtime CoreML EP Test
======================================================================

Model: models/minilm-l6-v2/onnx/model.onnx

📥 Loading tokenizer...
✅ Tokenizer loaded: 30,522 tokens

📦 Creating ONNX Runtime session with CoreML EP...
📦 Session created with providers:
   - CoreMLExecutionProvider ✅
   - CPUExecutionProvider

======================================================================
Test 1: Single Text Performance (10 runs)
======================================================================
🔥 Warmup run...
   Warmup time: 45.23ms

📊 Running 10 iterations...

✅ First run validation:
   Embedding dimension: 384
   L2 norm: 1.000000 (should be ~1.0)
   First 5 values: [0.0234, -0.0567, 0.0891, -0.0345, 0.0123]

📈 Performance Statistics:
   Mean:   8.45ms ✅
   Median: 8.12ms ✅
   P95:    11.23ms ✅ (<20ms TARGET MET!)
   P99:    12.67ms ✅
   Min:    7.89ms ✅
   Max:    13.45ms ✅

✅ TARGET MET: P95 11.23ms < 20ms

======================================================================
Test 2: Batch Processing Performance
======================================================================

Batch Size   Total (ms)   Per Text (ms)   Throughput (QPS)
------------ ------------ --------------- --------------------
1            8.12         8.12            123.2 ✅
2            12.34        6.17            162.1 ✅
4            18.56        4.64            215.5 ✅
8            29.78        3.72            268.8 ✅
16           52.34        3.27            305.8 ✅
32           98.67        3.08            324.7 ✅

======================================================================
Test 3: Embedding Quality & Similarity
======================================================================

📏 Similarity Scores:
   Similar queries:    0.8234 ✅
   Different queries:  0.1876 ✅
   Difference:         0.6358 ✅

✅ QUALITY CHECK PASSED: Similar queries have higher similarity

======================================================================
✅ ALL TESTS COMPLETE - TARGET ACHIEVED!
======================================================================

📊 Summary:
   ✅ P95 latency: 11.23ms (<20ms target)
   ✅ Peak throughput: 325 QPS (batch 32)
   ✅ Embedding quality: Excellent
   ✅ CoreML EP: Fully activated (no warnings)

📝 Next Steps:
   1. ✅ MiniLM validated for production use
   2. → Begin Rust implementation
   3. → Target: Match Python performance in Rust
```

**Alternative Outcome (If Not Achieving Target)**:

```
⚠️  TARGET MISSED: P95 24.56ms >= 20ms

Possible causes:
1. CoreML EP not fully activated (check for warnings)
2. Model file issue (try different export)
3. ONNX Runtime version issue

Debug steps:
1. Check session providers (should see CoreMLExecutionProvider first)
2. Try CPU-only for baseline (should be ~40-60ms)
3. If CPU-only is similar, CoreML EP not working

Next actions:
→ Debug CoreML EP activation
→ Try E5-small-v2 or BGE-small-en
→ OR accept and proceed with Rust (still much better than Qwen3)
```

#### Step 3: Quality Validation Against Qwen3

**Compare embedding quality**:

```python
# Quick quality comparison script
def compare_embeddings():
    # Load both models
    minilm_session = create_session("models/minilm-l6-v2/onnx/model.onnx")
    qwen3_session = create_session("models/qwen3-embedding-0.6b/onnx/model_fp16.onnx")

    # Test texts
    test_queries = [
        "machine learning algorithms",
        "neural network architecture",
        "database optimization",
        "cooking recipes",
    ]

    # Generate embeddings
    minilm_emb = embed_texts(minilm_session, test_queries, pooling="mean")
    qwen3_emb = embed_texts(qwen3_session, test_queries, pooling="last_token")

    # Compute similarity matrices
    minilm_sim = cosine_similarity_matrix(minilm_emb)
    qwen3_sim = cosine_similarity_matrix(qwen3_emb)

    # Compare
    print("MiniLM Similarity Matrix:")
    print(minilm_sim)
    print("\nQwen3 Similarity Matrix:")
    print(qwen3_sim)

    # Quality metrics
    # Similar pairs: (0,1), (2,3) should have high similarity
    # Different pairs: (0,3), (1,3) should have low similarity
```

**Expected**: MiniLM and Qwen3 should show similar relative similarities (correlation >0.7)

**Time Estimate**: 60-90 min total for performance testing and quality validation

### Session 1.3: Decision Point (Immediate)

**Decision Tree**:

```
CASE 1: P95 < 15ms (BEST CASE)
├── Action: Celebrate! 🎉
├── Quality: Verify acceptable (compare to Qwen3)
├── Decision: Proceed to Rust implementation immediately
└── Expected: High confidence in success

CASE 2: 15ms <= P95 < 20ms (TARGET MET)
├── Action: Analyze performance breakdown
├── Quality: Verify acceptable
├── Decision: Proceed to Rust if quality good
└── Expected: Likely success

CASE 3: 20ms <= P95 < 30ms (CLOSE)
├── Action: Debug CoreML EP activation
├── Check: Look for warning messages
├── Try: Different model export or ONNX optimization
├── Decision: If can't fix, try E5-small-v2 or BGE-small
└── Fallback: Accept if quality significantly better than alternatives

CASE 4: P95 >= 30ms (TARGET MISSED)
├── Action: Investigate root cause
│   ├── Check CoreML EP warnings
│   ├── Compare CPU-only vs CoreML (should be 3-5x speedup)
│   └── Verify model file integrity
├── Decision Options:
│   ├── Try E5-small-v2 (may be better optimized)
│   ├── Try BGE-small-en (newer model)
│   ├── Investigate MLX (Phase 2)
│   └── Accept Qwen3 CPU (118ms, fall back)
└── Timeline: Add 2-4 hours for alternatives
```

**Success Criteria for Proceeding to Rust**:

```
MUST HAVE:
✅ P95 < 30ms (at least 4x better than Qwen3)
✅ CoreML EP activates without errors
✅ Embedding quality acceptable (L2 norm ~1.0, good similarity)
✅ Model dimension known (384 for MiniLM)

NICE TO HAVE:
○ P95 < 20ms (target)
○ Throughput >100 QPS (batch processing)
○ Quality comparable to Qwen3

IF ALL "MUST HAVE" MET:
→ Proceed to Rust implementation
→ Document any performance gaps
→ Plan optimization for later if needed
```

**Decision Documentation**:

```bash
# Create decision record
cat > automatosx/tmp/MINILM-VALIDATION-DECISION.md << 'EOF'
# MiniLM Validation Decision - Day 2

## Test Results

**Model**: all-MiniLM-L6-v2 ONNX
**Date**: November 11, 2025

### Performance
- P95 latency: [RESULT]ms
- Throughput: [RESULT] QPS
- CoreML EP: [ACTIVATED/FAILED]

### Quality
- Embedding dimension: 384
- L2 normalization: [RESULT]
- Similarity separation: [RESULT]

## Decision

[PROCEED/TRY_ALTERNATIVE/FALLBACK]

### Rationale
[Explain reasoning based on results]

### Next Actions
[List specific next steps]
EOF
```

**Time Estimate**: Immediate decision once test results available

---

## Part 2: Rust ONNX Provider Implementation (Afternoon, 6-9 hours)

**Assuming**: MiniLM validation successful, proceeding with Rust implementation

### Session 2.1: Environment Preparation & API Research (1-2 hours)

#### Step 1: Study ort v2.0.0-rc.10 API Documentation

**Key APIs to master**:

```rust
// 1. Environment creation
use ort::Environment;

let env = Environment::builder()
    .with_name("akidb-onnx")
    .with_log_level(LoggingLevel::Warning)
    .build()?;

// 2. Session creation
use ort::{Session, SessionBuilder, GraphOptimizationLevel};

let session = SessionBuilder::new(&env)?
    .with_optimization_level(GraphOptimizationLevel::Level3)?
    .with_intra_threads(4)?  // Parallel ops within layer
    .with_inter_threads(1)?  // Sequential layer execution
    .with_model_from_file("model.onnx")?;

// 3. Execution provider configuration
use ort::execution_providers::{CoreMLExecutionProvider, ExecutionProvider};

let coreml = CoreMLExecutionProvider::default()
    .with_subgraphs(false)  // Disable for compatibility
    .build();

let session = SessionBuilder::new(&env)?
    .with_execution_providers([coreml])?
    .with_model_from_file("model.onnx")?;

// 4. Tensor creation and inference
use ort::{inputs, Value};
use ndarray::{Array2, ArrayView2};

// Create input tensor
let input_ids: Array2<i64> = Array2::from_shape_vec((batch, seq_len), vec)?;
let input_value = Value::from_array(input_ids.view())?;

// Run inference
let outputs = session.run(inputs![
    "input_ids" => input_value,
    "attention_mask" => mask_value,
]?)?;

// Extract output
let output_tensor: ArrayView2<f32> = outputs["last_hidden_state"].try_extract()?;
```

**API Changes from Earlier Versions**:

```rust
// OLD (ort 1.x):
session.run(vec![input_value])?;

// NEW (ort 2.0):
session.run(inputs!["input_ids" => input_value])?;

// OLD:
Value::from_array(Session, &array)?;

// NEW:
Value::from_array(array.view())?;  // No session parameter

// OLD:
CoreMLExecutionProviderOptions { ... }

// NEW:
CoreMLExecutionProvider::default()
    .with_subgraphs(false)
    .build()
```

#### Step 2: Review Existing onnx.rs Skeleton

```bash
# Read current implementation
cat crates/akidb-embedding/src/onnx.rs
```

**Current Issues to Fix**:
1. API calls outdated (ort 1.x style)
2. No actual tokenization implementation
3. No pooling implementation
4. Error handling incomplete
5. No tests

**Strategy**: Start fresh with correct API, reference Python implementation

#### Step 3: Plan Module Structure

```rust
// crates/akidb-embedding/src/onnx.rs

mod pooling;      // Mean pooling and L2 normalization
mod tokenization; // Tokenizer wrapper
mod session;      // ONNX session management
mod provider;     // Main OnnxEmbeddingProvider implementation

// Public API
pub use provider::OnnxEmbeddingProvider;
```

**Dependencies to add** (check Cargo.toml):

```toml
[dependencies]
# ONNX Runtime
ort = { version = "2.0.0-rc.10", features = ["download-binaries"], optional = true }

# Tensor operations
ndarray = { version = "0.15", optional = true }

# Tokenization
tokenizers = { version = "0.15.0", optional = true }

# Model download (optional, for convenience)
hf-hub = { version = "0.3.2", optional = true, default-features = false, features = ["tokio"] }

[features]
onnx = ["ort", "ndarray", "tokenizers", "hf-hub"]
```

**Time Estimate**: 1-2 hours for API research and planning

### Session 2.2: Implement Core Structure (2-3 hours)

#### Module 1: Session Management

```rust
// crates/akidb-embedding/src/onnx/session.rs

use ort::{Environment, Session, SessionBuilder, GraphOptimizationLevel};
use ort::execution_providers::{CoreMLExecutionProvider, ExecutionProvider};
use std::path::Path;
use std::sync::Arc;
use crate::error::{EmbeddingError, EmbeddingResult};

pub struct OnnxSession {
    session: Arc<Session>,
    dimension: usize,
}

impl OnnxSession {
    pub fn new(model_path: impl AsRef<Path>, use_coreml: bool) -> EmbeddingResult<Self> {
        // Create environment (reuse across sessions in production)
        let env = Arc::new(
            Environment::builder()
                .with_name("akidb-onnx")
                .build()
                .map_err(|e| EmbeddingError::ModelLoadError(e.to_string()))?
        );

        // Build session
        let mut builder = SessionBuilder::new(&env)
            .map_err(|e| EmbeddingError::ModelLoadError(e.to_string()))?;

        // Add CoreML EP if requested
        if use_coreml {
            let coreml = CoreMLExecutionProvider::default()
                .with_subgraphs(false)  // Compatibility
                .build();

            builder = builder
                .with_execution_providers([coreml])
                .map_err(|e| EmbeddingError::ModelLoadError(
                    format!("CoreML EP configuration failed: {}", e)
                ))?;
        }

        // Optimization settings
        builder = builder
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| EmbeddingError::ModelLoadError(e.to_string()))?
            .with_intra_threads(4)  // Tune based on CPU
            .map_err(|e| EmbeddingError::ModelLoadError(e.to_string()))?;

        // Load model
        let session = builder
            .with_model_from_file(model_path.as_ref())
            .map_err(|e| EmbeddingError::ModelLoadError(
                format!("Failed to load model from {:?}: {}", model_path.as_ref(), e)
            ))?;

        // Detect dimension from model metadata
        let dimension = Self::detect_dimension(&session)?;

        Ok(Self {
            session: Arc::new(session),
            dimension,
        })
    }

    fn detect_dimension(session: &Session) -> EmbeddingResult<usize> {
        // Get output metadata
        let outputs = session.outputs();

        // Look for last_hidden_state output
        for output in outputs {
            if output.name() == "last_hidden_state" {
                // Shape: [batch, seq_len, hidden_dim]
                let shape = output.shape();
                if shape.len() >= 3 {
                    // Last dimension is hidden_dim
                    if let Some(dim) = shape.last() {
                        return Ok(*dim as usize);
                    }
                }
            }
        }

        Err(EmbeddingError::ModelLoadError(
            "Could not detect embedding dimension from model outputs".to_string()
        ))
    }

    pub fn dimension(&self) -> usize {
        self.dimension
    }

    pub fn session(&self) -> &Arc<Session> {
        &self.session
    }
}
```

#### Module 2: Pooling Operations

```rust
// crates/akidb-embedding/src/onnx/pooling.rs

use ndarray::{Array2, ArrayView2, ArrayView3, Axis};
use crate::error::{EmbeddingError, EmbeddingResult};

/// Mean pooling with attention mask
pub fn mean_pool(
    hidden_states: ArrayView3<f32>,  // (batch, seq_len, hidden)
    attention_mask: ArrayView2<i64>,  // (batch, seq_len)
) -> EmbeddingResult<Array2<f32>> {
    let (batch_size, seq_len, hidden_dim) = hidden_states.dim();

    if attention_mask.dim() != (batch_size, seq_len) {
        return Err(EmbeddingError::InvalidInput(
            format!("Attention mask shape mismatch: expected ({}, {}), got {:?}",
                    batch_size, seq_len, attention_mask.dim())
        ));
    }

    let mut pooled = Array2::zeros((batch_size, hidden_dim));

    for i in 0..batch_size {
        let hidden_slice = hidden_states.index_axis(Axis(0), i);  // (seq_len, hidden)
        let mask_slice = attention_mask.index_axis(Axis(0), i);   // (seq_len,)

        // Count non-padding tokens
        let mask_sum: i64 = mask_slice.iter().sum();
        if mask_sum == 0 {
            return Err(EmbeddingError::InvalidInput(
                format!("Attention mask for sample {} is all zeros", i)
            ));
        }

        // Sum hidden states where mask == 1
        let mut sum_hidden = Array1::zeros(hidden_dim);
        for (j, &mask_val) in mask_slice.iter().enumerate() {
            if mask_val > 0 {
                sum_hidden = sum_hidden + hidden_slice.row(j);
            }
        }

        // Mean = sum / count
        let mean_hidden = sum_hidden / (mask_sum as f32);
        pooled.row_mut(i).assign(&mean_hidden);
    }

    Ok(pooled)
}

/// L2 normalization
pub fn l2_normalize(embeddings: &mut Array2<f32>) {
    for mut row in embeddings.rows_mut() {
        let norm = row.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-12 {  // Avoid division by zero
            row.mapv_inplace(|x| x / norm);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;

    #[test]
    fn test_mean_pool() {
        // Create test data
        let hidden = array![
            [[1.0, 2.0], [3.0, 4.0], [0.0, 0.0]],  // Batch 0, seq_len=3, hidden=2
            [[5.0, 6.0], [7.0, 8.0], [9.0, 10.0]], // Batch 1
        ];
        let mask = array![
            [1, 1, 0],  // First 2 tokens valid
            [1, 1, 1],  // All 3 tokens valid
        ];

        let pooled = mean_pool(hidden.view(), mask.view()).unwrap();

        // Batch 0: mean of [[1,2], [3,4]] = [2, 3]
        assert!((pooled[[0, 0]] - 2.0).abs() < 1e-6);
        assert!((pooled[[0, 1]] - 3.0).abs() < 1e-6);

        // Batch 1: mean of [[5,6], [7,8], [9,10]] = [7, 8]
        assert!((pooled[[1, 0]] - 7.0).abs() < 1e-6);
        assert!((pooled[[1, 1]] - 8.0).abs() < 1e-6);
    }

    #[test]
    fn test_l2_normalize() {
        let mut embeddings = array![
            [3.0, 4.0],      // norm = 5
            [1.0, 0.0],      // norm = 1
        ];

        l2_normalize(&mut embeddings);

        // First row: [3/5, 4/5] = [0.6, 0.8]
        assert!((embeddings[[0, 0]] - 0.6).abs() < 1e-6);
        assert!((embeddings[[0, 1]] - 0.8).abs() < 1e-6);

        // Second row: [1, 0] (already normalized)
        assert!((embeddings[[1, 0]] - 1.0).abs() < 1e-6);
        assert!((embeddings[[1, 1]] - 0.0).abs() < 1e-6);
    }
}
```

#### Module 3: Tokenization

```rust
// crates/akidb-embedding/src/onnx/tokenization.rs

use tokenizers::Tokenizer;
use ndarray::Array2;
use std::path::Path;
use std::sync::Arc;
use crate::error::{EmbeddingError, EmbeddingResult};

pub struct OnnxTokenizer {
    tokenizer: Arc<Tokenizer>,
}

impl OnnxTokenizer {
    pub fn from_file(path: impl AsRef<Path>) -> EmbeddingResult<Self> {
        let tokenizer = Tokenizer::from_file(path.as_ref())
            .map_err(|e| EmbeddingError::TokenizationError(
                format!("Failed to load tokenizer from {:?}: {}", path.as_ref(), e)
            ))?;

        Ok(Self {
            tokenizer: Arc::new(tokenizer),
        })
    }

    pub fn encode_batch(
        &self,
        texts: &[String],
        max_length: usize,
    ) -> EmbeddingResult<(Array2<i64>, Array2<i64>)> {
        // Encode all texts
        let encodings = self.tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| EmbeddingError::TokenizationError(e.to_string()))?;

        if encodings.is_empty() {
            return Err(EmbeddingError::InvalidInput("Empty text batch".to_string()));
        }

        let batch_size = encodings.len();

        // Find max sequence length in batch
        let seq_len = encodings
            .iter()
            .map(|e| e.len().min(max_length))
            .max()
            .unwrap_or(0);

        // Prepare arrays
        let mut input_ids = Array2::zeros((batch_size, seq_len));
        let mut attention_mask = Array2::zeros((batch_size, seq_len));

        // Fill arrays
        for (i, encoding) in encodings.iter().enumerate() {
            let ids = encoding.get_ids();
            let len = ids.len().min(seq_len);

            for j in 0..len {
                input_ids[[i, j]] = ids[j] as i64;
                attention_mask[[i, j]] = 1;
            }
            // Padding already zeros, so attention_mask is correct
        }

        Ok((input_ids, attention_mask))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenizer() {
        // This test requires actual tokenizer.json file
        // Skip if not available
        let tokenizer_path = "models/minilm-l6-v2/tokenizer.json";
        if !Path::new(tokenizer_path).exists() {
            println!("Skipping tokenizer test (file not found)");
            return;
        }

        let tokenizer = OnnxTokenizer::from_file(tokenizer_path).unwrap();
        let texts = vec!["Hello world".to_string(), "Test".to_string()];

        let (input_ids, attention_mask) = tokenizer.encode_batch(&texts, 512).unwrap();

        assert_eq!(input_ids.dim().0, 2);  // batch_size
        assert_eq!(attention_mask.dim(), input_ids.dim());

        // Check attention mask (first text should have more tokens)
        let mask_sum_0: i64 = attention_mask.row(0).iter().sum();
        let mask_sum_1: i64 = attention_mask.row(1).iter().sum();
        assert!(mask_sum_0 >= mask_sum_1);
    }
}
```

**Time Estimate**: 2-3 hours for core modules

### Session 2.3: Implement Main Provider (2-3 hours)

```rust
// crates/akidb-embedding/src/onnx/provider.rs

use async_trait::async_trait;
use ort::{inputs, Value};
use ndarray::Array3;
use std::path::{Path, PathBuf};
use crate::{
    EmbeddingProvider, BatchEmbeddingRequest, BatchEmbeddingResponse,
    ModelInfo, Usage, EmbeddingResult, EmbeddingError,
};
use super::{session::OnnxSession, tokenization::OnnxTokenizer, pooling::{mean_pool, l2_normalize}};

pub struct OnnxEmbeddingProvider {
    session: OnnxSession,
    tokenizer: OnnxTokenizer,
    model_name: String,
    max_length: usize,
}

impl OnnxEmbeddingProvider {
    /// Create new ONNX embedding provider
    pub async fn new(
        model_path: impl AsRef<Path>,
        model_name: impl Into<String>,
    ) -> EmbeddingResult<Self> {
        Self::with_config(model_path, model_name, true, 512).await
    }

    /// Create with custom configuration
    pub async fn with_config(
        model_path: impl AsRef<Path>,
        model_name: impl Into<String>,
        use_coreml: bool,
        max_length: usize,
    ) -> EmbeddingResult<Self> {
        let model_path = model_path.as_ref();

        // Load ONNX session
        let session = OnnxSession::new(model_path, use_coreml)?;

        // Load tokenizer (assume tokenizer.json in same directory)
        let tokenizer_path = model_path
            .parent()
            .ok_or_else(|| EmbeddingError::ModelLoadError("Invalid model path".to_string()))?
            .join("tokenizer.json");

        let tokenizer = OnnxTokenizer::from_file(tokenizer_path)?;

        Ok(Self {
            session,
            tokenizer,
            model_name: model_name.into(),
            max_length,
        })
    }

    /// Internal embedding implementation
    async fn embed_batch_internal(&self, texts: Vec<String>) -> EmbeddingResult<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Err(EmbeddingError::InvalidInput("Empty text batch".to_string()));
        }

        // 1. Tokenize
        let (input_ids, attention_mask) = self.tokenizer.encode_batch(&texts, self.max_length)?;

        // 2. Prepare ONNX inputs
        let input_ids_value = Value::from_array(input_ids.view())
            .map_err(|e| EmbeddingError::InferenceError(e.to_string()))?;

        let attention_mask_value = Value::from_array(attention_mask.view())
            .map_err(|e| EmbeddingError::InferenceError(e.to_string()))?;

        // 3. Run inference
        let outputs = self.session.session().run(inputs![
            "input_ids" => input_ids_value,
            "attention_mask" => attention_mask_value,
        ]?)
        .map_err(|e| EmbeddingError::InferenceError(e.to_string()))?;

        // 4. Extract last_hidden_state
        let hidden_states: Array3<f32> = outputs["last_hidden_state"]
            .try_extract()
            .map_err(|e| EmbeddingError::InferenceError(
                format!("Failed to extract hidden states: {}", e)
            ))?
            .into_owned();

        // 5. Mean pooling
        let mut pooled = mean_pool(hidden_states.view(), attention_mask.view())?;

        // 6. L2 normalize
        l2_normalize(&mut pooled);

        // 7. Convert to Vec<Vec<f32>>
        let embeddings: Vec<Vec<f32>> = pooled
            .outer_iter()
            .map(|row| row.to_vec())
            .collect();

        Ok(embeddings)
    }
}

#[async_trait]
impl EmbeddingProvider for OnnxEmbeddingProvider {
    async fn embed_batch(
        &self,
        request: BatchEmbeddingRequest,
    ) -> EmbeddingResult<BatchEmbeddingResponse> {
        let embeddings = self.embed_batch_internal(request.texts.clone()).await?;

        let usage = Usage {
            prompt_tokens: request.texts.iter().map(|t| t.len() / 4).sum(),  // Rough estimate
            total_tokens: request.texts.iter().map(|t| t.len() / 4).sum(),
        };

        Ok(BatchEmbeddingResponse {
            embeddings,
            model: self.model_name.clone(),
            usage,
        })
    }

    async fn model_info(&self) -> EmbeddingResult<ModelInfo> {
        Ok(ModelInfo {
            name: self.model_name.clone(),
            dimension: self.session.dimension() as u32,
            max_tokens: self.max_length as u32,
        })
    }

    async fn health_check(&self) -> EmbeddingResult<()> {
        // Try embedding a simple text
        let _ = self.embed_batch_internal(vec!["health check".to_string()]).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_onnx_provider_basic() {
        // Skip if model not available
        let model_path = "models/minilm-l6-v2/onnx/model.onnx";
        if !Path::new(model_path).exists() {
            println!("Skipping test (model not found)");
            return;
        }

        let provider = OnnxEmbeddingProvider::new(
            model_path,
            "all-MiniLM-L6-v2"
        ).await.unwrap();

        // Test single text
        let request = BatchEmbeddingRequest {
            texts: vec!["Hello world".to_string()],
            model: Some("all-MiniLM-L6-v2".to_string()),
        };

        let response = provider.embed_batch(request).await.unwrap();

        assert_eq!(response.embeddings.len(), 1);
        assert_eq!(response.embeddings[0].len(), 384);  // MiniLM dimension

        // Check L2 normalization
        let norm: f32 = response.embeddings[0].iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01, "L2 norm should be ~1.0, got {}", norm);
    }

    #[tokio::test]
    async fn test_onnx_provider_batch() {
        let model_path = "models/minilm-l6-v2/onnx/model.onnx";
        if !Path::new(model_path).exists() {
            return;
        }

        let provider = OnnxEmbeddingProvider::new(
            model_path,
            "all-MiniLM-L6-v2"
        ).await.unwrap();

        let request = BatchEmbeddingRequest {
            texts: vec![
                "Machine learning".to_string(),
                "Artificial intelligence".to_string(),
                "Cooking recipes".to_string(),
            ],
            model: None,
        };

        let response = provider.embed_batch(request).await.unwrap();

        assert_eq!(response.embeddings.len(), 3);

        // Check similarity (ML and AI should be more similar than ML and cooking)
        let cosine_similarity = |a: &[f32], b: &[f32]| -> f32 {
            a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
        };

        let sim_ml_ai = cosine_similarity(&response.embeddings[0], &response.embeddings[1]);
        let sim_ml_cook = cosine_similarity(&response.embeddings[0], &response.embeddings[2]);

        assert!(sim_ml_ai > sim_ml_cook,
            "ML-AI similarity ({}) should be higher than ML-Cooking ({})",
            sim_ml_ai, sim_ml_cook);
    }
}
```

**Time Estimate**: 2-3 hours for main provider implementation

### Session 2.4: Testing & Integration (1-2 hours)

```bash
# Run tests
cargo test -p akidb-embedding --features onnx -- --nocapture

# Run specific test
cargo test -p akidb-embedding test_onnx_provider_basic --features onnx -- --nocapture

# Check compilation
cargo check -p akidb-embedding --features onnx

# Run clippy
cargo clippy -p akidb-embedding --features onnx
```

**Expected Test Output**:

```
running 3 tests
test onnx::pooling::tests::test_mean_pool ... ok
test onnx::pooling::tests::test_l2_normalize ... ok
test onnx::provider::tests::test_onnx_provider_basic ... ok
test onnx::provider::tests::test_onnx_provider_batch ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Integration with akidb-service**:

```rust
// crates/akidb-service/src/lib.rs

#[cfg(feature = "onnx")]
use akidb_embedding::OnnxEmbeddingProvider;

// In collection service
pub async fn create_with_onnx_embedding() -> Result<CollectionService> {
    let embedding_provider = OnnxEmbeddingProvider::new(
        "models/minilm-l6-v2/onnx/model.onnx",
        "all-MiniLM-L6-v2"
    ).await?;

    CollectionService::new(Arc::new(embedding_provider))
}
```

**Time Estimate**: 1-2 hours for testing and integration

---

## Part 3: End of Day 2 Status (Evening)

### Success Criteria Checklist

**Morning Validation**:
- [ ] MiniLM ONNX model downloaded
- [ ] Model validated (vocab < 16K)
- [ ] CoreML EP test passed (P95 < 20ms)
- [ ] Quality validated (L2 norm, similarity)
- [ ] Decision made to proceed

**Afternoon Implementation**:
- [ ] Core modules implemented (session, pooling, tokenization)
- [ ] Main provider implemented
- [ ] Tests written and passing
- [ ] Integration with service layer
- [ ] Code compiles without warnings

**Performance**:
- [ ] Rust performance close to Python baseline (within 20%)
- [ ] L2 normalization working (norm ≈ 1.0)
- [ ] Similarity scores reasonable

### Documentation

Create end-of-day summary:

```markdown
# Day 2 Completion Summary

## Morning: MiniLM Validation
- Downloaded: [MODEL_REPO]
- Performance: P95 [X]ms ([PASS/FAIL] <20ms target)
- Quality: [ASSESSMENT]
- Decision: [PROCEED/ALTERNATIVE]

## Afternoon: Rust Implementation
- Modules implemented: [LIST]
- Tests passing: [X]/[Y]
- Performance: [Rust vs Python comparison]
- Integration: [STATUS]

## Challenges Encountered
1. [ISSUE]: [SOLUTION]
2. [ISSUE]: [SOLUTION]

## Tomorrow (Day 3)
- [ ] Additional testing
- [ ] Performance benchmarks
- [ ] Documentation
- [ ] Production readiness review
```

### Timeline Status

**If On Track**:
```
Day 2: ✅ Complete
├── Morning: MiniLM validation successful
├── Afternoon: Rust implementation working
└── Evening: Tests passing

Day 3 Plan:
├── Morning: Comprehensive testing (3-4 hours)
├── Afternoon: Performance optimization (2-3 hours)
└── Evening: Documentation & wrap-up (2-3 hours)

Expected Delivery: End of Day 3 ✅
```

**If Behind Schedule**:
```
Day 2: ⚠️ Partial
├── Issue: [DESCRIPTION]
├── Impact: [HOURS DELAY]
└── Mitigation: [PLAN]

Revised Day 3 Plan:
├── Complete remaining Day 2 work
├── Condensed testing
└── Minimal documentation

Expected Delivery: Day 4 morning
```

---

## Conclusion

Day 2 execution plan provides:

1. **Clear Morning Path**: Download MiniLM → Validate → Decide (2-3 hours)
2. **Structured Implementation**: Modular Rust code with tests (6-9 hours)
3. **Decision Gates**: Clear criteria at each step
4. **Fallback Options**: Multiple paths if issues arise
5. **Success Metrics**: Concrete checkpoints

**Confidence**: High (75%) for successful Day 2 completion

**Key Risk**: MiniLM performance - if it doesn't achieve <20ms, have E5/BGE alternatives ready

**Expected Outcome**: Working Rust ONNX provider with <20ms performance by end of Day 3

---

**Status**: Ready to execute Day 2 plan

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>
