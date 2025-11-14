# ONNX+CoreML Execution Megathink - Day-by-Day Implementation

**Date**: November 10, 2025
**Status**: 🚀 **EXECUTION READY** - Detailed implementation roadmap
**Timeline**: 3 days intensive implementation
**Goal**: Production-ready ONNX Runtime + CoreML EP embedding provider

---

## Current State Analysis

### ✅ What We Have

1. **Model Downloaded**: 7.5 GB Qwen3-Embedding-0.6B ONNX
   - FP16 model: 1.1 GB (recommended for CoreML)
   - FP32 model: 2.3 GB (full precision fallback)
   - Multiple quantized versions available
   - Tokenizer and config files present

2. **Infrastructure Ready**:
   - Download script: `scripts/download_qwen3_onnx.py` ✅
   - Cargo.toml updated: ort v2.0.0-rc.10, no Candle ✅
   - lib.rs updated: ONNX exports only ✅
   - Skeleton onnx.rs: Needs rewrite for correct API ✅

3. **Documentation Complete**:
   - Investigation report (Candle Metal issue) ✅
   - Migration summary (why ONNX) ✅
   - Download completion report ✅
   - Phase 2 implementation plan ✅
   - This execution megathink ✅

4. **Environment**:
   - Python 3.9 available ✅
   - huggingface-hub installed ✅
   - Rust toolchain ready ✅
   - macOS with Metal GPU ✅

### ⏳ What We Need

1. **Python Validation** (Priority 1):
   - Install: onnx, onnxruntime, transformers
   - Validate ONNX model structure
   - Test CoreML EP activation
   - Verify <20ms inference
   - Measure baseline performance

2. **Rust Implementation** (Priority 2):
   - Rewrite onnx.rs with correct ort API
   - Implement CoreML EP configuration
   - Fix tensor I/O for ndarray
   - Implement mean pooling + L2 norm
   - Add comprehensive error handling

3. **Testing & Validation** (Priority 3):
   - 10+ integration tests
   - Performance benchmarks
   - Quality validation (vs Python)
   - Edge case handling

4. **Documentation** (Priority 4):
   - Update README with usage
   - Create migration guide
   - Add examples
   - Document performance metrics

---

## Day 1: Python Validation & API Verification (6-8 hours)

**Goal**: Confirm ONNX model works with CoreML EP, measure baseline performance

### Session 1: Environment Setup (1 hour)

#### Task 1.1: Install Python Dependencies

```bash
# Install ONNX ecosystem
pip3 install onnx onnxruntime transformers --upgrade

# Verify installations
python3 -c "import onnx; print(f'onnx: {onnx.__version__}')"
python3 -c "import onnxruntime as ort; print(f'onnxruntime: {ort.__version__}')"
python3 -c "import transformers; print(f'transformers: {transformers.__version__}')"
```

**Expected Output**:
```
onnx: 1.16.0+
onnxruntime: 1.18.0+
transformers: 4.44.0+
```

**Time**: 15 minutes

#### Task 1.2: Verify Model Files

```bash
# Check FP16 model (recommended)
ls -lh models/qwen3-embedding-0.6b/onnx/model_fp16.onnx*
# Should show: model_fp16.onnx (570K) + model_fp16.onnx_data (1.1G)

# Check tokenizer
ls -lh models/qwen3-embedding-0.6b/tokenizer.json
# Should show: tokenizer.json (11M)

# Check config
cat models/qwen3-embedding-0.6b/config.json | grep -E "(hidden_size|num_hidden_layers)"
# Should show: hidden_size: 768, num_hidden_layers: 12
```

**Time**: 5 minutes

#### Task 1.3: Quick ONNX Model Info

```python
# Quick inspection script
import onnx

model_path = "models/qwen3-embedding-0.6b/onnx/model_fp16.onnx"
model = onnx.load(model_path)

print("Model inputs:")
for input in model.graph.input[:2]:  # Show first 2 inputs
    print(f"  - {input.name}")

print("\nModel outputs:")
for output in model.graph.output[:1]:  # Show first output
    print(f"  - {output.name}")
```

**Expected**:
```
Model inputs:
  - input_ids
  - attention_mask

Model outputs:
  - last_hidden_state
```

**Time**: 10 minutes

**Session 1 Checkpoint**: Environment ready, model files verified

---

### Session 2: ONNX Validation Script (1.5 hours)

#### Task 2.1: Create Comprehensive Validation Script

**File**: `scripts/validate_qwen3_onnx.py`

```python
#!/usr/bin/env python3
"""
Comprehensive ONNX model validation for Qwen3-Embedding-0.6B.

Validates:
1. Model file integrity
2. Input/output signatures
3. Operator compatibility
4. External data file loading
"""

import onnx
import sys
from pathlib import Path

def validate_model_structure(model_path):
    """Validate ONNX model structure and metadata."""
    print(f"{'='*70}")
    print(f"ONNX Model Validation")
    print(f"{'='*70}\n")

    print(f"📋 Loading model: {model_path}")

    try:
        model = onnx.load(model_path)
    except Exception as e:
        print(f"❌ Failed to load model: {e}")
        return False

    print(f"✅ Model loaded successfully\n")

    # Validate model
    try:
        onnx.checker.check_model(model)
        print(f"✅ Model validation passed\n")
    except Exception as e:
        print(f"❌ Model validation failed: {e}")
        return False

    # Print model info
    print(f"📊 Model Information:")
    print(f"   IR Version: {model.ir_version}")
    print(f"   Opset Version: {model.opset_import[0].version}")
    print(f"   Producer: {model.producer_name} {model.producer_version}")
    print(f"   Doc String: {model.doc_string[:50]}..." if model.doc_string else "")

    # Print inputs
    print(f"\n🔹 Model Inputs ({len(model.graph.input)} total):")
    for idx, input_tensor in enumerate(model.graph.input):
        shape_str = "x".join(
            str(dim.dim_param if dim.dim_param else dim.dim_value)
            for dim in input_tensor.type.tensor_type.shape.dim
        )
        dtype = onnx.TensorProto.DataType.Name(input_tensor.type.tensor_type.elem_type)
        print(f"   {idx+1}. {input_tensor.name}")
        print(f"      Shape: [{shape_str}]")
        print(f"      Type: {dtype}")

    # Print outputs
    print(f"\n🔹 Model Outputs ({len(model.graph.output)} total):")
    for idx, output_tensor in enumerate(model.graph.output):
        shape_str = "x".join(
            str(dim.dim_param if dim.dim_param else dim.dim_value)
            for dim in output_tensor.type.tensor_type.shape.dim
        )
        dtype = onnx.TensorProto.DataType.Name(output_tensor.type.tensor_type.elem_type)
        print(f"   {idx+1}. {output_tensor.name}")
        print(f"      Shape: [{shape_str}]")
        print(f"      Type: {dtype}")

    # Check for external data
    has_external_data = any(
        init.HasField('data_location') and init.data_location == onnx.TensorProto.EXTERNAL
        for init in model.graph.initializer
    )

    print(f"\n📦 External Data:")
    if has_external_data:
        print(f"   ✅ Uses external data file (.onnx_data)")
        # Check if data file exists
        data_file = Path(model_path).with_suffix('.onnx_data')
        if data_file.exists():
            size_mb = data_file.stat().st_size / 1024 / 1024
            print(f"   ✅ Data file found: {data_file.name} ({size_mb:.1f} MB)")
        else:
            print(f"   ❌ Data file missing: {data_file.name}")
            return False
    else:
        print(f"   ℹ️  Model weights embedded in .onnx file")

    # Check operators
    op_types = set()
    for node in model.graph.node:
        op_types.add(node.op_type)

    print(f"\n🔧 Operators ({len(op_types)} unique types):")
    print(f"   {', '.join(sorted(list(op_types)[:10]))}...")

    # Check for potentially problematic ops
    problematic_ops = {'NonMaxSuppression', 'RoiAlign', 'QuantizeLinear'}
    found_problematic = op_types.intersection(problematic_ops)
    if found_problematic:
        print(f"\n   ⚠️  Potentially problematic ops for CoreML: {found_problematic}")

    print(f"\n{'='*70}")
    print(f"✅ Validation Complete - Model is valid")
    print(f"{'='*70}\n")

    return True


def main():
    import argparse

    parser = argparse.ArgumentParser(description="Validate Qwen3-Embedding ONNX model")
    parser.add_argument(
        "--model",
        default="models/qwen3-embedding-0.6b/onnx/model_fp16.onnx",
        help="Path to ONNX model file"
    )
    args = parser.parse_args()

    success = validate_model_structure(args.model)

    if success:
        print("📝 Next steps:")
        print("  1. Test with ONNX Runtime: python scripts/test_qwen3_coreml.py")
        print("  2. Implement Rust provider: edit crates/akidb-embedding/src/onnx.rs")
        print("  3. Run tests: cargo test --features onnx")
    else:
        print("❌ Validation failed. Check errors above.")

    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
```

**Run**:
```bash
python3 scripts/validate_qwen3_onnx.py
```

**Expected Output** (abbreviated):
```
=====================================================================
ONNX Model Validation
=====================================================================

📋 Loading model: models/qwen3-embedding-0.6b/onnx/model_fp16.onnx
✅ Model loaded successfully

✅ Model validation passed

📊 Model Information:
   IR Version: 9
   Opset Version: 14

🔹 Model Inputs (2 total):
   1. input_ids
      Shape: [batch_size x sequence_length]
      Type: INT64
   2. attention_mask
      Shape: [batch_size x sequence_length]
      Type: INT64

🔹 Model Outputs (1 total):
   1. last_hidden_state
      Shape: [batch_size x sequence_length x 768]
      Type: FLOAT16

📦 External Data:
   ✅ Uses external data file (.onnx_data)
   ✅ Data file found: model_fp16.onnx_data (1144.3 MB)

=====================================================================
✅ Validation Complete - Model is valid
=====================================================================
```

**Success Criteria**:
- ✅ Model loads without errors
- ✅ Validation passes
- ✅ Inputs: input_ids (INT64), attention_mask (INT64)
- ✅ Output: last_hidden_state (FLOAT16, dim 768)
- ✅ External data file found

**Time**: 30 minutes to write, 5 minutes to run

**Session 2 Checkpoint**: ONNX model structure validated

---

### Session 3: CoreML EP Testing (2-3 hours)

#### Task 3.1: Create CoreML EP Test Script

**File**: `scripts/test_qwen3_coreml.py`

**Note**: This is a critical script - it validates that CoreML EP works before we implement in Rust

```python
#!/usr/bin/env python3
"""
Test Qwen3-Embedding with ONNX Runtime + CoreML Execution Provider.

This script validates:
1. CoreML EP activation
2. Inference correctness
3. Performance benchmarks
4. Embedding quality
"""

import onnxruntime as ort
import numpy as np
import time
from transformers import AutoTokenizer
from pathlib import Path
import sys


def test_coreml_providers():
    """Test available execution providers."""
    print("🔍 Checking available execution providers...\n")

    available = ort.get_available_providers()
    print(f"Available providers: {available}")

    if 'CoreMLExecutionProvider' in available:
        print(f"✅ CoreML EP is available")
    else:
        print(f"⚠️  CoreML EP NOT available")
        print(f"   This is expected on non-macOS or macOS <15.1")
        print(f"   Will use CPU EP only\n")

    return 'CoreMLExecutionProvider' in available


def create_session(model_path, use_coreml=True):
    """Create ONNX Runtime session with CoreML EP."""
    print(f"\n🔧 Creating ONNX Runtime session...")
    print(f"   Model: {model_path}")

    sess_options = ort.SessionOptions()
    sess_options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL

    providers = []

    if use_coreml:
        # CoreML EP configuration
        coreml_options = {
            'MLComputeUnits': 'ALL',  # Use CPU, GPU, and Neural Engine
            'ModelFormat': 'MLProgram',  # Newer format (macOS 12+)
            'RequireStaticInputShapes': False,  # Allow dynamic batch sizes
            'EnableOnSubgraphs': False,  # Don't enable on subgraphs (more stable)
        }
        providers.append(('CoreMLExecutionProvider', coreml_options))

    # Always include CPU as fallback
    providers.append('CPUExecutionProvider')

    try:
        sess = ort.InferenceSession(
            str(model_path),
            sess_options=sess_options,
            providers=providers
        )

        active_providers = sess.get_providers()
        print(f"✅ Session created")
        print(f"   Active providers: {active_providers}")

        if 'CoreMLExecutionProvider' in active_providers:
            print(f"   🎉 CoreML EP is ACTIVE (using Metal GPU/ANE)")
        else:
            print(f"   ℹ️  Using CPU only")

        return sess, 'CoreMLExecutionProvider' in active_providers

    except Exception as e:
        print(f"❌ Failed to create session: {e}")
        raise


def mean_pooling(last_hidden_state, attention_mask):
    """
    Perform mean pooling on token embeddings.

    Args:
        last_hidden_state: (batch_size, seq_len, hidden_dim)
        attention_mask: (batch_size, seq_len)

    Returns:
        pooled: (batch_size, hidden_dim)
    """
    # Expand attention mask to match embedding dimensions
    attention_mask_expanded = np.expand_dims(attention_mask, axis=-1).astype(np.float32)

    # Multiply embeddings by mask (zero out padding tokens)
    masked_embeddings = last_hidden_state * attention_mask_expanded

    # Sum over sequence length
    sum_embeddings = np.sum(masked_embeddings, axis=1)

    # Sum mask values (number of non-padding tokens)
    sum_mask = np.clip(np.sum(attention_mask_expanded, axis=1), a_min=1e-9, a_max=None)

    # Divide to get mean
    mean_pooled = sum_embeddings / sum_mask

    return mean_pooled


def l2_normalize(embeddings):
    """L2 normalize embeddings to unit vectors."""
    norms = np.linalg.norm(embeddings, axis=1, keepdims=True)
    norms = np.clip(norms, a_min=1e-12, a_max=None)  # Prevent division by zero
    return embeddings / norms


def test_single_inference(sess, tokenizer, text, coreml_active):
    """Test single text inference."""
    print(f"\n📝 Testing: '{text}'")

    # Tokenize
    inputs = tokenizer(
        text,
        return_tensors="np",
        padding="max_length",
        max_length=512,
        truncation=True
    )

    # Prepare inputs
    ort_inputs = {
        'input_ids': inputs['input_ids'].astype(np.int64),
        'attention_mask': inputs['attention_mask'].astype(np.int64)
    }

    # Run inference with timing
    start = time.perf_counter()
    outputs = sess.run(None, ort_inputs)
    inference_time = (time.perf_counter() - start) * 1000  # Convert to ms

    # Extract and process output
    last_hidden_state = outputs[0]  # (1, seq_len, 768)

    # Mean pooling
    pooled = mean_pooling(last_hidden_state, inputs['attention_mask'])

    # L2 normalization
    normalized = l2_normalize(pooled)

    # Verify
    print(f"   ⏱️  Inference time: {inference_time:.2f}ms")
    print(f"   📊 Output shape: {last_hidden_state.shape}")
    print(f"   📏 Embedding dim: {normalized.shape[1]}")
    print(f"   🔢 L2 norm: {np.linalg.norm(normalized):.6f}")

    # Check performance target
    target_ms = 20
    if coreml_active:
        if inference_time < target_ms:
            print(f"   ✅ EXCELLENT: Under {target_ms}ms target!")
        elif inference_time < 50:
            print(f"   ⚠️  ACCEPTABLE: {inference_time:.2f}ms (target: <{target_ms}ms)")
        else:
            print(f"   ❌ SLOW: {inference_time:.2f}ms (target: <{target_ms}ms)")
    else:
        print(f"   ℹ️  CPU-only mode (CoreML not active)")

    return normalized, inference_time


def test_batch_inference(sess, tokenizer, texts, coreml_active):
    """Test batch inference."""
    batch_size = len(texts)
    print(f"\n📦 Batch test ({batch_size} texts)...")

    # Tokenize batch
    inputs = tokenizer(
        texts,
        return_tensors="np",
        padding=True,
        truncation=True,
        max_length=512
    )

    # Prepare inputs
    ort_inputs = {
        'input_ids': inputs['input_ids'].astype(np.int64),
        'attention_mask': inputs['attention_mask'].astype(np.int64)
    }

    # Run inference with timing
    start = time.perf_counter()
    outputs = sess.run(None, ort_inputs)
    inference_time = (time.perf_counter() - start) * 1000

    # Process
    last_hidden_state = outputs[0]
    pooled = mean_pooling(last_hidden_state, inputs['attention_mask'])
    normalized = l2_normalize(pooled)

    print(f"   ⏱️  Total time: {inference_time:.2f}ms ({inference_time/batch_size:.2f}ms per text)")
    print(f"   📊 Output shape: {last_hidden_state.shape}")
    print(f"   📏 Embeddings: {normalized.shape}")

    # Check batch performance
    target_ms = 60 if batch_size == 8 else 180
    if coreml_active:
        if inference_time < target_ms:
            print(f"   ✅ EXCELLENT: Under {target_ms}ms target!")
        else:
            print(f"   ⚠️  Slower than target ({target_ms}ms)")

    return normalized, inference_time


def test_embedding_quality(embeddings, texts):
    """Test embedding quality with similarity checks."""
    print(f"\n🔬 Testing embedding quality...")

    # Calculate cosine similarity matrix
    similarities = np.matmul(embeddings, embeddings.T)

    print(f"   Similarity matrix:")
    for i, text_i in enumerate(texts):
        print(f"     {text_i[:30]:30s} | ", end="")
        for j in range(len(texts)):
            print(f"{similarities[i,j]:.3f} ", end="")
        print()

    # Check diagonal (self-similarity should be ~1.0)
    diagonal = np.diagonal(similarities)
    if np.all(np.abs(diagonal - 1.0) < 0.01):
        print(f"   ✅ Self-similarity correct (all ~1.0)")
    else:
        print(f"   ⚠️  Self-similarity off: {diagonal}")

    # Check for reasonable diversity (not all same embedding)
    off_diagonal = similarities[np.triu_indices_from(similarities, k=1)]
    mean_sim = np.mean(off_diagonal)
    print(f"   📊 Mean cross-similarity: {mean_sim:.3f}")

    if mean_sim < 0.99:
        print(f"   ✅ Embeddings are diverse (not degenerate)")
    else:
        print(f"   ⚠️  Embeddings too similar (possible issue)")


def main():
    print("="*70)
    print("Qwen3-Embedding ONNX Runtime + CoreML EP Test")
    print("="*70)

    # Check providers
    coreml_available = test_coreml_providers()

    # Paths
    model_path = Path("models/qwen3-embedding-0.6b/onnx/model_fp16.onnx")
    tokenizer_path = Path("models/qwen3-embedding-0.6b")

    if not model_path.exists():
        print(f"\n❌ Model not found: {model_path}")
        print(f"   Run: python scripts/download_qwen3_onnx.py")
        sys.exit(1)

    # Load tokenizer
    print(f"\n📝 Loading tokenizer from {tokenizer_path}...")
    try:
        tokenizer = AutoTokenizer.from_pretrained(
            str(tokenizer_path),
            trust_remote_code=True
        )
        print(f"✅ Tokenizer loaded (vocab size: {tokenizer.vocab_size})")
    except Exception as e:
        print(f"❌ Failed to load tokenizer: {e}")
        sys.exit(1)

    # Create session
    sess, coreml_active = create_session(model_path, use_coreml=coreml_available)

    # Test single inference
    test_texts = [
        "Hello, world!",
        "ONNX Runtime with CoreML is fast.",
        "Apple Silicon M1 processor with Neural Engine.",
    ]

    embeddings_list = []
    times = []

    for text in test_texts:
        emb, time_ms = test_single_inference(sess, tokenizer, text, coreml_active)
        embeddings_list.append(emb[0])
        times.append(time_ms)

    # Test batch
    batch_texts = [
        "Apple Silicon",
        "Metal GPU",
        "CoreML framework",
        "ONNX Runtime",
        "Neural Engine",
        "Machine Learning",
        "Vector embeddings",
        "Semantic search"
    ]
    batch_emb, batch_time = test_batch_inference(sess, tokenizer, batch_texts, coreml_active)

    # Quality checks
    test_embedding_quality(np.array(embeddings_list), test_texts)

    # Summary
    print(f"\n{'='*70}")
    print(f"📊 Performance Summary")
    print(f"{'='*70}")
    print(f"   CoreML EP Active: {'Yes ✅' if coreml_active else 'No (CPU only)'}")
    print(f"   Single text avg: {np.mean(times):.2f}ms (min: {np.min(times):.2f}ms, max: {np.max(times):.2f}ms)")
    print(f"   Batch of 8: {batch_time:.2f}ms ({batch_time/8:.2f}ms per text)")
    print(f"   Embedding dimension: 768")
    print(f"   L2 normalized: ✅")

    if coreml_active:
        if np.mean(times) < 20:
            print(f"\n✅ SUCCESS: CoreML EP working, performance excellent (<20ms)!")
        elif np.mean(times) < 50:
            print(f"\n⚠️  CoreML EP working, but slower than target")
            print(f"   Consider: Model warm-up, system load, thermal throttling")
        else:
            print(f"\n❌ CoreML EP slow or not accelerating properly")
    else:
        print(f"\n   Using CPU fallback (no CoreML on this system)")

    print(f"\n📝 Next steps:")
    print(f"   1. Implement Rust provider: crates/akidb-embedding/src/onnx.rs")
    print(f"   2. Match this performance in Rust")
    print(f"   3. Run integration tests")

    print(f"\n{'='*70}\n")

    sys.exit(0)


if __name__ == "__main__":
    main()
```

**Run**:
```bash
python3 scripts/test_qwen3_coreml.py
```

**Expected Output** (Mac with CoreML):
```
======================================================================
Qwen3-Embedding ONNX Runtime + CoreML EP Test
======================================================================

🔍 Checking available execution providers...

Available providers: ['CoreMLExecutionProvider', 'CPUExecutionProvider']
✅ CoreML EP is available

🔧 Creating ONNX Runtime session...
   Model: models/qwen3-embedding-0.6b/onnx/model_fp16.onnx
✅ Session created
   Active providers: ['CoreMLExecutionProvider', 'CPUExecutionProvider']
   🎉 CoreML EP is ACTIVE (using Metal GPU/ANE)

📝 Testing: 'Hello, world!'
   ⏱️  Inference time: 14.23ms
   📊 Output shape: (1, 512, 768)
   📏 Embedding dim: 768
   🔢 L2 norm: 1.000000
   ✅ EXCELLENT: Under 20ms target!

[... more tests ...]

======================================================================
📊 Performance Summary
======================================================================
   CoreML EP Active: Yes ✅
   Single text avg: 15.67ms (min: 14.23ms, max: 18.12ms)
   Batch of 8: 52.34ms (6.54ms per text)
   Embedding dimension: 768
   L2 normalized: ✅

✅ SUCCESS: CoreML EP working, performance excellent (<20ms)!
```

**Success Criteria**:
- ✅ CoreML EP activates (in provider list)
- ✅ Single text <20ms
- ✅ Batch of 8 <60ms
- ✅ Embeddings 768-dim
- ✅ L2 norm = 1.0
- ✅ Self-similarity ~1.0
- ✅ Cross-similarity diverse

**Time**: 1 hour to write, 15 minutes to run and analyze

**Session 3 Checkpoint**: CoreML EP validated, performance confirmed

---

### Session 4: Performance Baseline Documentation (1 hour)

#### Task 4.1: Record Baseline Metrics

Create performance baseline document for comparison with Rust:

**File**: `automatosx/tmp/PYTHON-COREML-BASELINE.md`

```markdown
# Python ONNX Runtime + CoreML EP Baseline

**Date**: November 10, 2025
**Hardware**: [Your Mac model, e.g., M1 Pro]
**macOS**: [Version, e.g., 15.1]
**Model**: Qwen3-Embedding-0.6B FP16
**Provider**: CoreML Execution Provider

## Performance Metrics

### Single Text Inference

| Metric | Value |
|--------|-------|
| Min latency | X.XX ms |
| Avg latency | X.XX ms |
| Max latency | X.XX ms |
| P95 latency | X.XX ms |

### Batch Inference

| Batch Size | Total Time | Per Text |
|------------|------------|----------|
| 1 | X.XX ms | X.XX ms |
| 4 | X.XX ms | X.XX ms |
| 8 | X.XX ms | X.XX ms |
| 16 | X.XX ms | X.XX ms |

### Quality Metrics

- Embedding dimension: 768
- L2 norm: 1.000000 ± 0.000001
- Self-similarity: 1.000 ± 0.001
- Cross-similarity: 0.XXX ± 0.XXX

## Rust Implementation Target

Rust implementation should match or exceed these metrics:
- Single text: ≤ Python avg + 5ms
- Batch: ≤ Python total + 10ms
- Quality: Identical (embeddings must match)

## Next Steps

1. Implement Rust provider
2. Compare performance
3. Validate embeddings match Python
```

Fill in actual values from test run.

**Time**: 30 minutes

**Day 1 Checkpoint**: Python validation complete, baseline documented, ready for Rust implementation

---

## Day 2: Rust Implementation (10-12 hours)

**Goal**: Implement production-ready OnnxEmbeddingProvider in Rust

### Session 5: onnx.rs Core Structure (3-4 hours)

#### Task 5.1: Analyze Current ort v2.0.0-rc.10 API

Before rewriting, check actual ort API:

```bash
# Check ort documentation
cargo doc --open --package ort

# Or check crates.io
open https://docs.rs/ort/2.0.0-rc.10/ort/
```

**Key API Points to Verify**:
1. Environment creation
2. Session builder pattern
3. Execution provider configuration
4. Tensor input/output format
5. Error types

**Time**: 30 minutes

#### Task 5.2: Rewrite onnx.rs with Correct API

**Strategy**: Start with minimal working version, then add features

**Minimal Version** (Phase 1 - Get it compiling):

```rust
// crates/akidb-embedding/src/onnx.rs

use async_trait::async_trait;
use std::sync::Arc;
use ndarray::{Array2, ArrayView2, ArrayView3};

use crate::{
    BatchEmbeddingRequest, BatchEmbeddingResponse,
    EmbeddingError, EmbeddingProvider, EmbeddingResult,
    ModelInfo, Usage,
};

// Import ort types - VERIFY these imports with actual ort API
use ort::{
    Environment,
    Session,
    SessionBuilder,
    GraphOptimizationLevel,
    Value,
};

use tokenizers::{Tokenizer, Encoding, PaddingParams, PaddingStrategy, TruncationParams};
use hf_hub::api::tokio::Api;

/// ONNX Runtime embedding provider with CoreML EP support.
pub struct OnnxEmbeddingProvider {
    session: Arc<Session>,
    tokenizer: Arc<Tokenizer>,
    model_name: String,
    dimension: u32,
}

impl OnnxEmbeddingProvider {
    /// Create new ONNX provider.
    ///
    /// For CoreML EP to work:
    /// - macOS 15.1+
    /// - model_path should point to FP16 model for best performance
    pub async fn new(model_path: &str, model_name: &str) -> EmbeddingResult<Self> {
        eprintln!("🔧 Initializing ONNX provider...");
        eprintln!("   Model: {}", model_path);

        // 1. Create environment
        let environment = Arc::new(
            Environment::builder()
                .with_name("akidb-onnx")
                .build()
                .map_err(|e| EmbeddingError::Internal(format!("Environment: {}", e)))?
        );

        // 2. Try to create session with CoreML EP
        // Note: Actual CoreML EP API may differ - check ort docs
        let session = Self::create_session(&environment, model_path)?;

        eprintln!("✅ Session created");

        // 3. Load tokenizer
        let tokenizer = Self::load_tokenizer(model_name).await?;
        eprintln!("✅ Tokenizer loaded");

        // 4. Get dimension from session metadata
        let dimension = Self::get_dimension(&session)?;

        eprintln!("✅ ONNX provider initialized (dim: {})", dimension);

        Ok(Self {
            session: Arc::new(session),
            tokenizer: Arc::new(tokenizer),
            model_name: model_name.to_string(),
            dimension,
        })
    }

    fn create_session(
        env: &Environment,
        model_path: &str,
    ) -> EmbeddingResult<Session> {
        // Start with CPU-only for now
        // TODO: Add CoreML EP once CPU version works

        let session = SessionBuilder::new(env)
            .map_err(|e| EmbeddingError::Internal(format!("SessionBuilder: {}", e)))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| EmbeddingError::Internal(format!("Optimization: {}", e)))?
            .with_model_from_file(model_path)
            .map_err(|e| EmbeddingError::Internal(format!("Load model: {}", e)))?;

        Ok(session)
    }

    async fn load_tokenizer(model_name: &str) -> EmbeddingResult<Tokenizer> {
        let api = Api::new()
            .map_err(|e| EmbeddingError::Internal(format!("HF API: {}", e)))?;

        let repo = api.model(model_name.to_string());

        let tokenizer_path = repo
            .get("tokenizer.json")
            .await
            .map_err(|e| EmbeddingError::Internal(format!("Download tokenizer: {}", e)))?;

        let mut tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| EmbeddingError::Internal(format!("Load tokenizer: {}", e)))?;

        // Configure for 512 max length
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: 512,
                ..Default::default()
            }))
            .map_err(|e| EmbeddingError::Internal(format!("Truncation: {}", e)))?;

        Ok(tokenizer)
    }

    fn get_dimension(session: &Session) -> EmbeddingResult<u32> {
        // Get output shape from session metadata
        // Output should be: (batch, seq_len, hidden_size)
        // We want hidden_size (dim 2)

        // TODO: Actual implementation depends on ort API
        // For now, hardcode for Qwen3-0.6B
        Ok(768)
    }

    /// Internal embedding generation (without trait constraints).
    pub async fn embed_batch_internal(
        &self,
        texts: Vec<String>,
    ) -> EmbeddingResult<Vec<Vec<f32>>> {
        // TODO: Implement
        // 1. Tokenize
        // 2. Create tensors
        // 3. Run ONNX inference
        // 4. Mean pooling
        // 5. L2 normalize

        Err(EmbeddingError::Internal("Not implemented yet".into()))
    }
}

#[async_trait]
impl EmbeddingProvider for OnnxEmbeddingProvider {
    async fn embed_batch(
        &self,
        request: BatchEmbeddingRequest,
    ) -> EmbeddingResult<BatchEmbeddingResponse> {
        // Validation
        if request.inputs.is_empty() {
            return Err(EmbeddingError::InvalidInput("Empty input".into()));
        }

        // Generate embeddings
        let embeddings = self.embed_batch_internal(request.inputs.clone()).await?;

        // Build response
        Ok(BatchEmbeddingResponse {
            model: request.model,
            embeddings,
            usage: Usage {
                total_tokens: 0,  // TODO: Calculate
                duration_ms: 0,   // TODO: Measure
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
        // TODO: Implement
        Ok(())
    }
}
```

**Compile Test**:
```bash
cargo check --features onnx -p akidb-embedding
```

**Expected**: Compilation succeeds (even though functions not implemented)

**Time**: 2 hours to write, 30 minutes to fix compilation errors

#### Task 5.3: Implement Tokenization & Tensor Prep

Add to OnnxEmbeddingProvider:

```rust
fn tokenize_texts(&self, texts: &[String]) -> EmbeddingResult<Vec<Encoding>> {
    texts
        .iter()
        .map(|text| {
            self.tokenizer
                .encode(text.as_str(), true)
                .map_err(|e| EmbeddingError::Internal(format!("Tokenize: {}", e)))
        })
        .collect()
}

fn prepare_tensors(
    &self,
    encodings: &[Encoding],
) -> EmbeddingResult<(Array2<i64>, Array2<i64>)> {
    const MAX_LEN: usize = 512;
    let batch_size = encodings.len();

    let mut input_ids = Vec::with_capacity(batch_size * MAX_LEN);
    let mut attention_mask = Vec::with_capacity(batch_size * MAX_LEN);

    for encoding in encodings {
        let ids = encoding.get_ids();
        let mask = encoding.get_attention_mask();

        // Pad or truncate to MAX_LEN
        for i in 0..MAX_LEN {
            input_ids.push(ids.get(i).copied().unwrap_or(0) as i64);
            attention_mask.push(mask.get(i).copied().unwrap_or(0) as i64);
        }
    }

    let input_ids_array = Array2::from_shape_vec((batch_size, MAX_LEN), input_ids)
        .map_err(|e| EmbeddingError::Internal(format!("Shape input_ids: {}", e)))?;

    let attention_mask_array = Array2::from_shape_vec((batch_size, MAX_LEN), attention_mask)
        .map_err(|e| EmbeddingError::Internal(format!("Shape attention_mask: {}", e)))?;

    Ok((input_ids_array, attention_mask_array))
}
```

**Test**:
```bash
cargo check --features onnx -p akidb-embedding
```

**Time**: 1 hour

**Session 5 Checkpoint**: Core structure compiling, tokenization ready

---

### Session 6: ONNX Inference Implementation (3-4 hours)

#### Task 6.1: Implement ONNX Inference

This is the critical part - need to match ort API exactly.

**Research First**: Check ort examples or docs for current Value API

**Likely implementation**:

```rust
async fn run_onnx_inference(
    &self,
    input_ids: Array2<i64>,
    attention_mask: Array2<i64>,
) -> EmbeddingResult<Array3<f32>> {
    // Create ort::Value from ndarray
    // Note: Actual API may differ - verify with ort docs

    let input_ids_value = Value::from_array(input_ids.view())
        .map_err(|e| EmbeddingError::Internal(format!("Create input_ids tensor: {}", e)))?;

    let attention_mask_value = Value::from_array(attention_mask.view())
        .map_err(|e| EmbeddingError::Internal(format!("Create attention_mask tensor: {}", e)))?;

    // Run session
    let outputs = self.session
        .run(vec![input_ids_value, attention_mask_value])
        .map_err(|e| EmbeddingError::Internal(format!("ONNX inference: {}", e)))?;

    // Extract output tensor
    // Output is last_hidden_state: (batch, seq_len, 768)
    let last_hidden_state = outputs[0]
        .try_extract::<f32>()
        .map_err(|e| EmbeddingError::Internal(format!("Extract output: {}", e)))?;

    // Convert to ndarray
    // TODO: Verify this conversion matches ort API
    let shape = last_hidden_state.shape();
    let data = last_hidden_state.view().to_owned();

    Ok(data)
}
```

**Note**: This will require trial-and-error to match actual ort API

**Strategy**:
1. Write initial version based on docs
2. Compile and fix errors
3. Add debug prints to check tensor shapes
4. Test with simple input
5. Iterate until working

**Time**: 2-3 hours (includes debugging)

#### Task 6.2: Implement Mean Pooling

```rust
fn mean_pooling(
    &self,
    last_hidden_state: ArrayView3<f32>,  // (batch, seq, hidden)
    attention_mask: ArrayView2<i64>,     // (batch, seq)
) -> EmbeddingResult<Array2<f32>> {
    let batch_size = last_hidden_state.shape()[0];
    let seq_len = last_hidden_state.shape()[1];
    let hidden_size = last_hidden_state.shape()[2];

    let mut pooled = Array2::zeros((batch_size, hidden_size));

    for b in 0..batch_size {
        let mut sum = vec![0.0f32; hidden_size];
        let mut count = 0.0f32;

        for s in 0..seq_len {
            let mask_val = attention_mask[[b, s]] as f32;
            count += mask_val;

            for h in 0..hidden_size {
                sum[h] += last_hidden_state[[b, s, h]] * mask_val;
            }
        }

        // Compute mean
        if count > 0.0 {
            for h in 0..hidden_size {
                pooled[[b, h]] = sum[h] / count;
            }
        }
    }

    Ok(pooled)
}
```

**Test**: Add debug print to check pooled shape

**Time**: 30 minutes

#### Task 6.3: Implement L2 Normalization

```rust
fn l2_normalize(&self, embeddings: Array2<f32>) -> EmbeddingResult<Vec<Vec<f32>>> {
    let batch_size = embeddings.shape()[0];
    let hidden_size = embeddings.shape()[1];

    let mut result = Vec::with_capacity(batch_size);

    for b in 0..batch_size {
        let row = embeddings.row(b);

        // Calculate L2 norm
        let norm: f32 = row.iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt()
            .max(1e-12);  // Prevent division by zero

        // Normalize
        let normalized: Vec<f32> = row.iter()
            .map(|x| x / norm)
            .collect();

        result.push(normalized);
    }

    Ok(result)
}
```

**Time**: 30 minutes

**Session 6 Checkpoint**: Full inference pipeline implemented

---

### Session 7: Integration & Testing (2-3 hours)

#### Task 7.1: Complete embed_batch_internal

Wire everything together:

```rust
pub async fn embed_batch_internal(
    &self,
    texts: Vec<String>,
) -> EmbeddingResult<Vec<Vec<f32>>> {
    // 1. Tokenize
    let encodings = self.tokenize_texts(&texts)?;

    // 2. Prepare tensors
    let (input_ids, attention_mask) = self.prepare_tensors(&encodings)?;

    // 3. ONNX inference
    let last_hidden_state = self.run_onnx_inference(input_ids.view(), attention_mask.view()).await?;

    // 4. Mean pooling
    let pooled = self.mean_pooling(last_hidden_state.view(), attention_mask.view())?;

    // 5. L2 normalize
    let normalized = self.l2_normalize(pooled)?;

    Ok(normalized)
}
```

**Time**: 30 minutes

#### Task 7.2: First Integration Test

Write simple test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_inference() {
        let provider = OnnxEmbeddingProvider::new(
            "models/qwen3-embedding-0.6b/onnx/model_fp16.onnx",
            "Qwen/Qwen3-Embedding-0.6B"
        )
        .await
        .expect("Init failed");

        let embeddings = provider
            .embed_batch_internal(vec!["Hello".to_string()])
            .await
            .expect("Inference failed");

        assert_eq!(embeddings.len(), 1);
        assert_eq!(embeddings[0].len(), 768);

        // Check L2 normalized
        let norm: f32 = embeddings[0].iter().map(|x| x*x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01);
    }
}
```

**Run**:
```bash
cargo test --features onnx -p akidb-embedding test_basic_inference
```

**Debug Strategy** if test fails:
1. Add eprintln! at each step
2. Check tensor shapes
3. Compare with Python output
4. Fix issues iteratively

**Time**: 1-2 hours (includes debugging)

**Session 7 Checkpoint**: Basic test passing

**Day 2 Checkpoint**: Rust provider working, ready for comprehensive testing

---

## Day 3: Testing, Optimization & Documentation (6-8 hours)

[Continuing in next section due to length...]

**Status**: Megathink shows clear 3-day path from validation to production

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>
