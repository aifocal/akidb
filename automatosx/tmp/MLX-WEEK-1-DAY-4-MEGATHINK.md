# MLX Week 1 Day 4: Real Transformer Inference - Comprehensive Megathink

**Date:** 2025-11-08
**Status:** PLANNING
**Complexity:** ⚠️ **HIGH** (Full transformer implementation)
**Estimated Time:** 12-16 hours (may span 2 days)

---

## Table of Contents

1. [Current State Analysis](#current-state-analysis)
2. [Model Architecture Deep Dive](#model-architecture-deep-dive)
3. [Implementation Options](#implementation-options)
4. [Recommended Approach](#recommended-approach)
5. [Detailed Implementation Plan](#detailed-implementation-plan)
6. [Alternative: Lightweight Approach](#alternative-lightweight-approach)
7. [Risk Assessment](#risk-assessment)
8. [Success Criteria](#success-criteria)

---

## Current State Analysis

### What We Have ✅

**Downloaded Model:**
- Location: `~/.cache/akidb/models/qwen3-0.6b-4bit/`
- Format: SafeTensors (320MB)
- Quantization: 4-bit DWQ (biases + scales)
- Total parameters: 595,776,512
- Architecture: Qwen2 (28 layers, 1024 hidden, 151K vocab)

**Files Present:**
```
model.safetensors          (320MB) - 4-bit quantized weights
model.safetensors.index.json (49KB) - Weight mapping
config.json                (937B)  - Model configuration
tokenizer.json             (11MB)  - Fast tokenizer
vocab.json                 (2.6MB) - Vocabulary
merges.txt                 (1.6MB) - BPE merges
```

**Working Pipeline:**
- ✅ PyO3 bridge (Rust ↔ Python)
- ✅ Model downloading & caching
- ✅ Placeholder tokenization
- ✅ Placeholder forward pass
- ✅ Mean pooling (REAL)
- ✅ CLS pooling (REAL)
- ✅ L2 normalization (REAL)
- ✅ End-to-end integration

### What We Need ❌

**Missing Components:**
1. **Real Tokenizer** - Convert text → token IDs
2. **Model Loading** - Load 4-bit quantized weights into MLX
3. **Transformer Architecture** - Build 28-layer Qwen2 model
4. **Forward Pass** - Run inference through all layers
5. **Embedding Extraction** - Get hidden states for pooling

---

## Model Architecture Deep Dive

### Qwen2 Architecture

**From config.json:**
```json
{
  "architectures": ["Qwen2ForCausalLM"],
  "attention_dropout": 0.0,
  "bos_token_id": 151643,
  "eos_token_id": 151645,
  "hidden_act": "silu",
  "hidden_size": 1024,
  "initializer_range": 0.02,
  "intermediate_size": 2816,
  "max_position_embeddings": 32768,
  "model_type": "qwen2",
  "num_attention_heads": 16,
  "num_hidden_layers": 28,
  "num_key_value_heads": 2,
  "rms_norm_eps": 1e-06,
  "rope_theta": 1000000.0,
  "sliding_window": 32768,
  "tie_word_embeddings": true,
  "torch_dtype": "bfloat16",
  "transformers_version": "4.48.0",
  "use_cache": true,
  "use_sliding_window": false,
  "vocab_size": 151936
}
```

**Key Parameters:**
- **28 layers** (each with self-attention + MLP)
- **16 attention heads** (+ 2 KV heads for GQA)
- **1024 hidden size**
- **2816 intermediate size** (MLP)
- **RMSNorm** (not LayerNorm)
- **SiLU activation** (Swish)
- **RoPE embeddings** (rotary position encodings)
- **GQA** (Grouped Query Attention - 16 Q heads, 2 KV heads)

**Weight Structure (from index.json):**
```
model.embed_tokens.weight              # [151936, 1024]
model.embed_tokens.biases/scales       # 4-bit quantization params

For each of 28 layers:
  model.layers.{i}.input_layernorm.weight
  model.layers.{i}.self_attn.q_proj.{weight,biases,scales}
  model.layers.{i}.self_attn.k_proj.{weight,biases,scales}
  model.layers.{i}.self_attn.v_proj.{weight,biases,scales}
  model.layers.{i}.self_attn.o_proj.{weight,biases,scales}
  model.layers.{i}.self_attn.q_norm.weight
  model.layers.{i}.self_attn.k_norm.weight
  model.layers.{i}.post_attention_layernorm.weight
  model.layers.{i}.mlp.gate_proj.{weight,biases,scales}
  model.layers.{i}.mlp.up_proj.{weight,biases,scales}
  model.layers.{i}.mlp.down_proj.{weight,biases,scales}

model.norm.weight                      # Final RMSNorm
model.lm_head.{weight,biases,scales}   # LM head (not needed for embeddings)
```

**Quantization Format (4-bit):**
- Each linear layer has:
  - `weight`: INT4 packed weights
  - `biases`: FP16 biases (for de-quantization)
  - `scales`: FP16 scales (for de-quantization)

---

## Implementation Options

### Option 1: Use mlx-lm Library (EASIEST)

**Pros:**
- ✅ Model loading handled automatically
- ✅ Quantization support built-in
- ✅ Tokenizer integration
- ✅ ~20 lines of code

**Cons:**
- ❌ Designed for text generation, not embeddings
- ❌ May not expose hidden states easily
- ❌ Additional dependency (`mlx-lm`)

**Code Example:**
```python
from mlx_lm import load

model, tokenizer = load("mlx-community/Qwen3-Embedding-0.6B-4bit-DWQ")
# Problem: How to get embeddings instead of generated text?
```

### Option 2: Use mlx.nn + Manual Architecture (HARDEST)

**Pros:**
- ✅ Full control over model
- ✅ Can extract embeddings at any layer
- ✅ Minimal dependencies (only MLX)

**Cons:**
- ❌ Must implement entire Qwen2 architecture (~500+ lines)
- ❌ Must handle 4-bit quantization manually
- ❌ Complex rope embeddings, GQA, RMSNorm
- ❌ High risk of bugs

**Required Components:**
```python
class Qwen2Attention(nn.Module):
    # Grouped Query Attention with RoPE

class Qwen2MLP(nn.Module):
    # Gate + Up + Down projections with SiLU

class Qwen2DecoderLayer(nn.Module):
    # Self-attention + MLP + RMSNorm

class Qwen2Model(nn.Module):
    # Embedding + 28 layers + final norm
```

### Option 3: Hybrid Approach (RECOMMENDED)

**Use `sentence-transformers` + MLX backend**

**Pros:**
- ✅ Designed specifically for embeddings
- ✅ Handles pooling strategies
- ✅ Simpler API
- ✅ Can potentially use MLX backend

**Cons:**
- ❌ May not have MLX support yet
- ❌ Additional dependency

**Code Example:**
```python
from sentence_transformers import SentenceTransformer

model = SentenceTransformer('Qwen/Qwen3-Embedding-0.6B')
embeddings = model.encode(["text1", "text2"])
```

**Problem:** sentence-transformers uses PyTorch by default, not MLX

### Option 4: Pragmatic Hybrid (BEST FOR NOW)

**Use HuggingFace transformers + manual pooling**

**Pros:**
- ✅ Model loading handled
- ✅ Can access hidden states
- ✅ Well-documented
- ✅ ~50 lines of code

**Cons:**
- ❌ Uses PyTorch, not MLX (less optimized for Apple Silicon)
- ❌ Larger memory footprint

**Code Example:**
```python
from transformers import AutoTokenizer, AutoModel
import torch

tokenizer = AutoTokenizer.from_pretrained(model_path)
model = AutoModel.from_pretrained(model_path)

inputs = tokenizer(texts, padding=True, return_tensors='pt')
with torch.no_grad():
    outputs = model(**inputs)
    embeddings = outputs.last_hidden_state  # [batch, seq_len, 1024]

# Apply mean pooling
embeddings = mean_pool(embeddings, inputs['attention_mask'])
```

---

## Recommended Approach

**Strategy: Phased Implementation**

### Phase 1 (Day 4): HuggingFace Transformers (Quick Win)

**Goal:** Get real embeddings working ASAP

**Steps:**
1. Install `transformers` library
2. Load tokenizer from model directory
3. Load model with `AutoModel`
4. Extract hidden states
5. Apply existing pooling/normalization
6. Verify embeddings are semantically meaningful

**Time:** 4-6 hours
**Risk:** LOW
**Performance:** ~50-100ms per batch (CPU fallback)

### Phase 2 (Week 2): MLX Optimization (Performance)

**Goal:** Migrate to MLX for Apple Silicon acceleration

**Steps:**
1. Install `mlx-lm` or use raw MLX
2. Convert forward pass to MLX
3. Benchmark performance improvement
4. Optimize batching

**Time:** 8-12 hours
**Risk:** MEDIUM
**Performance:** <25ms per batch (target)

---

## Detailed Implementation Plan

### Day 4 Phase 1: HuggingFace Transformers

#### Hour 1-2: Setup & Dependencies

**Tasks:**
1. Install transformers library
2. Test model loading
3. Verify tokenizer works

**Code:**
```bash
pip install transformers torch
```

```python
# Test script
from transformers import AutoTokenizer, AutoModel

model_path = "~/.cache/akidb/models/qwen3-0.6b-4bit"
tokenizer = AutoTokenizer.from_pretrained(model_path, local_files_only=True)
model = AutoModel.from_pretrained(model_path, local_files_only=True)

print(f"Model loaded: {model.config.model_type}")
print(f"Hidden size: {model.config.hidden_size}")
```

**Validation:**
- No errors during load
- Tokenizer encodes text correctly
- Model config matches expectations

#### Hour 3-4: Update mlx_inference.py

**Replace placeholder tokenization:**
```python
from transformers import AutoTokenizer

class MLXEmbeddingModel:
    def __init__(self, model_path: Path):
        # ... existing code ...

        # Load real tokenizer
        self.tokenizer = AutoTokenizer.from_pretrained(
            str(model_path),
            local_files_only=True
        )

    def tokenize(self, texts: List[str], max_length: int = 512) -> dict:
        """Real tokenization using HuggingFace."""
        encoded = self.tokenizer(
            texts,
            padding=True,
            truncation=True,
            max_length=max_length,
            return_tensors='np'  # NumPy for MLX compatibility
        )

        return {
            "input_ids": mx.array(encoded['input_ids']),
            "attention_mask": mx.array(encoded['attention_mask']),
        }
```

#### Hour 5-6: Implement Real Forward Pass

**Option A: Use transformers model directly (easier)**
```python
from transformers import AutoModel
import torch

class MLXEmbeddingModel:
    def __init__(self, model_path: Path):
        # ... existing code ...

        # Load transformer model
        self.hf_model = AutoModel.from_pretrained(
            str(model_path),
            local_files_only=True
        )
        self.hf_model.eval()  # Inference mode

    def forward(self, input_ids: mx.array, attention_mask: mx.array) -> mx.array:
        """Forward pass using HuggingFace model."""
        # Convert MLX arrays to PyTorch tensors
        input_ids_pt = torch.from_numpy(np.array(input_ids))
        attention_mask_pt = torch.from_numpy(np.array(attention_mask))

        # Run inference
        with torch.no_grad():
            outputs = self.hf_model(
                input_ids=input_ids_pt,
                attention_mask=attention_mask_pt,
                output_hidden_states=True
            )

        # Get last hidden state
        hidden_states = outputs.last_hidden_state  # [batch, seq_len, 1024]

        # Convert back to MLX
        hidden_states_mx = mx.array(hidden_states.numpy())

        return hidden_states_mx
```

**Option B: Pure MLX (harder, for later)**
```python
# Defer to Phase 2
```

#### Hour 7-8: Integration & Testing

**Update embedding_service.py:**
```python
# No changes needed! MLXEmbeddingModel interface unchanged
```

**Run tests:**
```bash
PYO3_PYTHON=/opt/homebrew/bin/python3.13 cargo test -p akidb-embedding mlx
```

**Validation:**
- All tests pass
- Embeddings are NOT random (semantic meaning)
- Check cosine similarity:
  - sim("cat", "dog") > sim("cat", "car")
  - sim("king", "queen") > sim("king", "apple")

#### Hour 9-10: Semantic Verification

**Create test script:**
```python
def test_semantic_similarity():
    service = EmbeddingService()

    # Test 1: Similar concepts
    emb1 = service.embed(["The cat sat on the mat"])[0]
    emb2 = service.embed(["A feline rested on the rug"])[0]
    sim_similar = cosine_similarity(emb1, emb2)

    # Test 2: Different concepts
    emb3 = service.embed(["The stock market crashed today"])[0]
    sim_different = cosine_similarity(emb1, emb3)

    assert sim_similar > 0.7, f"Similar texts should be close: {sim_similar}"
    assert sim_different < 0.5, f"Different texts should be far: {sim_different}"
    assert sim_similar > sim_different, "Sanity check"

    print(f"✓ Similar texts: {sim_similar:.3f}")
    print(f"✓ Different texts: {sim_different:.3f}")
```

#### Hour 11-12: Performance Benchmarking

**Create benchmark script:**
```python
import time

def benchmark_inference():
    service = EmbeddingService()

    texts = ["This is a test sentence."] * 10  # Batch of 10

    # Warmup
    service.embed(texts)

    # Benchmark
    times = []
    for _ in range(100):
        start = time.time()
        service.embed(texts)
        times.append((time.time() - start) * 1000)  # ms

    print(f"Mean: {np.mean(times):.2f}ms")
    print(f"P50:  {np.percentile(times, 50):.2f}ms")
    print(f"P95:  {np.percentile(times, 95):.2f}ms")
    print(f"P99:  {np.percentile(times, 99):.2f}ms")
```

**Expected Results (PyTorch CPU):**
- Mean: ~80-150ms (10 texts)
- P95: ~200ms
- P99: ~300ms

**Note:** This is CPU fallback. MLX optimization (Phase 2) will target <25ms.

---

## Alternative: Lightweight Approach

**If transformers library is too heavy, use mlx-lm:**

```python
from mlx_lm import load

class MLXEmbeddingModel:
    def __init__(self, model_path: Path):
        self.model, self.tokenizer = load(str(model_path))

    def embed(self, texts, pooling="mean", normalize=True):
        # Tokenize
        encoded = self.tokenizer(texts, return_tensors='mlx')

        # Get hidden states (need to modify mlx-lm generate function)
        # This requires diving into mlx-lm internals
        outputs = self.model(**encoded, output_hidden_states=True)
        hidden_states = outputs.hidden_states[-1]  # Last layer

        # Pool and normalize
        if pooling == "mean":
            embeddings = self._mean_pool(hidden_states, encoded['attention_mask'])

        if normalize:
            embeddings = self._l2_normalize(embeddings)

        return np.array(embeddings)
```

**Challenge:** mlx-lm's `generate()` function doesn't expose hidden states easily.

---

## Risk Assessment

### High Risks ⚠️

**1. Model Loading Complexity**
- **Risk:** SafeTensors with 4-bit quantization is complex
- **Mitigation:** Use transformers library (handles automatically)
- **Fallback:** Use mlx-lm if transformers fails

**2. Performance Not Meeting Target**
- **Risk:** PyTorch CPU may be too slow (<25ms P95)
- **Mitigation:** Defer MLX optimization to Phase 2
- **Acceptance:** Day 4 focuses on correctness, not speed

**3. Memory Issues**
- **Risk:** 600MB model + PyTorch overhead = ~1.5GB RAM
- **Mitigation:** Mac ARM has sufficient RAM (8GB+)
- **Monitoring:** Track memory usage in tests

### Medium Risks ⚙️

**4. Tokenizer Compatibility**
- **Risk:** Tokenizer may not work with local files
- **Mitigation:** Verify with quick test before full implementation
- **Fallback:** Use `tokenizers` library directly

**5. Hidden State Extraction**
- **Risk:** Model may not expose hidden states
- **Mitigation:** Use `output_hidden_states=True` parameter
- **Verification:** Test with small example first

### Low Risks ✅

**6. Integration Breaking**
- **Risk:** PyO3 bridge may have issues with PyTorch tensors
- **Mitigation:** Convert torch → numpy → MLX (extra copy, but safe)
- **Impact:** Minimal performance hit (~1-2ms)

---

## Success Criteria

### Minimum Viable (Day 4 Complete)

- ✅ Real tokenizer working (not random token IDs)
- ✅ Real model inference (not random embeddings)
- ✅ Embeddings are semantically meaningful
- ✅ All Rust tests pass
- ✅ Cosine similarity tests pass
- ✅ No crashes or memory leaks

### Stretch Goals (Nice to Have)

- 🎯 Performance <100ms P95 (10 texts, PyTorch CPU)
- 🎯 Batch size optimization
- 🎯 Caching for identical texts
- 🎯 Memory usage <2GB

### Phase 2 Goals (Week 2)

- 🚀 Pure MLX implementation
- 🚀 Performance <25ms P95 (MLX GPU)
- 🚀 Memory usage <1GB
- 🚀 Batching optimization

---

## Implementation Checklist

### Prerequisites
- [ ] Install `transformers` library
- [ ] Install `torch` (PyTorch)
- [ ] Verify model loads correctly
- [ ] Verify tokenizer works

### Core Implementation
- [ ] Update `tokenize()` with real tokenizer
- [ ] Update `forward()` with transformer model
- [ ] Keep existing pooling/normalization unchanged
- [ ] Update `embedding_service.py` if needed

### Testing & Validation
- [ ] Run existing Rust tests (should pass)
- [ ] Create semantic similarity test
- [ ] Create benchmark script
- [ ] Verify embeddings are NOT random
- [ ] Check cosine similarity makes sense

### Documentation
- [ ] Update completion report
- [ ] Document performance metrics
- [ ] Note limitations (PyTorch CPU)
- [ ] Plan Phase 2 (MLX optimization)

---

## Next Steps After Day 4

### Week 1 Day 5: Configuration & E2E

**Goals:**
- YAML configuration loading
- Multi-model support testing (Qwen3 + Gemma)
- User-provided embeddings mode
- End-to-end integration tests
- Collection service integration

### Week 2: Optimization & Production

**Day 6-7: MLX Optimization**
- Migrate to pure MLX (no PyTorch)
- Performance tuning
- Batch optimization

**Day 8-9: Testing & Polish**
- Load testing @ 50 QPS
- Memory profiling
- Error handling

**Day 10: Documentation & Completion**
- API documentation
- Deployment guide
- Final integration

---

## Conclusion

**Recommended Path for Day 4:**

1. **Use HuggingFace transformers** (pragmatic, proven)
2. **Accept PyTorch CPU performance** (optimize later)
3. **Focus on correctness** (semantic embeddings)
4. **Defer MLX optimization to Week 2** (performance tuning)

**Time Estimate:** 12 hours (full day + evening)

**Success Probability:** 90% (transformers is well-tested)

**Performance Expected:**
- Current (placeholder): ~5ms (but wrong)
- Day 4 (transformers): ~100ms (but correct!)
- Week 2 (MLX): <25ms (correct + fast)

**Philosophy:** "Make it work, make it right, make it fast" - we're at step 2 (make it right).

---

**Status:** Ready to implement ✅
**Blocker:** None
**Dependencies:** transformers, torch
**Next Action:** Install dependencies and begin Hour 1
