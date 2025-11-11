# Candle Phase 1: Foundation - PRD

**Version:** 1.0.0
**Date:** 2025-01-10
**Phase:** 1 of 6
**Duration:** 5 days (1 week)
**Status:** READY TO EXECUTE

---

## Executive Summary

Phase 1 establishes the foundation for Candle embedding integration by implementing a basic, working provider with the MiniLM model. This phase focuses on getting the core inference pipeline working correctly before optimizing for performance.

**Goal:** Working Candle provider that generates embeddings with <20ms latency per text

**Key Deliverables:**
- ✅ `CandleEmbeddingProvider` implementation (~400 lines)
- ✅ 15+ unit tests (100% passing)
- ✅ EmbeddingProvider trait integration
- ✅ Feature flag (`candle`)
- ✅ CI pipeline support

**Success Criteria:**
- ✅ Generates embeddings for single text (<20ms)
- ✅ Generates embeddings for batch (1-32 texts)
- ✅ All 15 unit tests pass
- ✅ Similarity to MLX embeddings >90%
- ✅ Works on macOS ARM + Linux x86_64

---

## Problem Statement

### Current State

**MLX Provider Issues:**
- Python GIL limits concurrency (5.5 QPS max)
- 182ms latency per request
- Cannot deploy in Docker/Kubernetes
- Requires Python runtime + dependencies

**Why Phase 1 Matters:**
- Establishes technical feasibility of Candle
- Validates performance assumptions
- Creates foundation for future phases
- Enables parallel development (Phase 2+ can start)

---

## Goals & Non-Goals

### Goals

1. **Core Functionality**
   - ✅ Load MiniLM model from Hugging Face Hub
   - ✅ Tokenize text inputs (single + batch)
   - ✅ Run inference on GPU (Metal/CUDA) or CPU
   - ✅ Generate 384-dimensional embeddings

2. **Quality**
   - ✅ Unit test coverage >80%
   - ✅ Embedding similarity >90% vs MLX
   - ✅ Error handling for common failures

3. **Integration**
   - ✅ Implement `EmbeddingProvider` trait
   - ✅ Feature flag for easy A/B testing
   - ✅ No breaking changes to existing API

### Non-Goals

1. **Performance Optimization** (Phase 2)
   - Multi-threaded inference
   - Dynamic batching
   - Sub-50ms latency

2. **Production Hardening** (Phase 3)
   - Prometheus metrics
   - Retry logic
   - Circuit breakers

3. **Multiple Models** (Phase 4)
   - BGE model support
   - Qwen2 model support
   - Model switching API

4. **Cloud Deployment** (Phase 5)
   - Docker images
   - Kubernetes manifests

---

## Technical Design

### Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│          REST API (existing, no changes)            │
└──────────────────┬──────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────┐
│        EmbeddingManager (existing)                  │
│  ├─ MLX provider (existing)                        │
│  └─ Candle provider (NEW) ⭐                       │
└──────────────────┬──────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────┐
│        CandleEmbeddingProvider                      │
│  ├─ Model: Arc<BertModel>                          │
│  ├─ Tokenizer: Arc<Tokenizer>                      │
│  ├─ Device: Metal | CUDA | CPU                     │
│  └─ embed_batch() → Vec<Vec<f32>>                  │
└──────────────────┬──────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────┐
│             Candle Core                             │
│  ├─ BertModel::load() - Load from safetensors     │
│  ├─ Tokenizer::encode() - Tokenization            │
│  ├─ Tensor::forward() - GPU/CPU inference         │
│  └─ Mean pooling - Extract embeddings             │
└─────────────────────────────────────────────────────┘
```

### Component Design

#### 1. CandleEmbeddingProvider Struct

```rust
pub struct CandleEmbeddingProvider {
    /// BERT model (thread-safe)
    model: Arc<BertModel>,

    /// Tokenizer (thread-safe)
    tokenizer: Arc<Tokenizer>,

    /// Device (Metal, CUDA, or CPU)
    device: Device,

    /// Model name (e.g., "sentence-transformers/all-MiniLM-L6-v2")
    model_name: String,

    /// Embedding dimension (384 for MiniLM)
    dimension: u32,
}
```

**Key Design Decisions:**
- `Arc<T>` for thread-safe sharing (needed for multi-threading in Phase 2)
- `Device` abstraction for GPU/CPU flexibility
- Simple, flat structure (no complex state)

#### 2. Initialization Flow

```
User calls new("model-name")
    ↓
Download from Hugging Face Hub
    ├─ model.safetensors (90MB)
    ├─ config.json (2KB)
    └─ tokenizer.json (500KB)
    ↓
Select Device
    ├─ Try Metal (macOS) first
    ├─ Try CUDA (Linux) second
    └─ Fallback to CPU
    ↓
Load Model Weights
    ├─ Memory-map safetensors file
    ├─ Load into GPU/CPU
    └─ Initialize BERT layers
    ↓
Create Tokenizer
    ├─ Load vocab.txt
    ├─ Configure padding/truncation
    └─ Set max_length=512
    ↓
Return Provider
```

**Time Breakdown:**
- Download: 5-30 seconds (one-time, cached)
- Model loading: 1-2 seconds
- Total cold start: 6-32 seconds
- Warm start (cached): 1-2 seconds

#### 3. Inference Pipeline

```
User calls embed_batch(["text1", "text2"])
    ↓
Tokenize Texts
    ├─ Convert to token IDs
    ├─ Add [CLS], [SEP] tokens
    ├─ Pad to max_length
    └─ Create attention masks
    ↓
Convert to Tensor
    ├─ Shape: [batch_size, seq_len]
    ├─ Device: Metal/CUDA/CPU
    └─ Dtype: i64 (token IDs)
    ↓
Forward Pass (GPU/CPU)
    ├─ BERT encoder (12 layers)
    ├─ Self-attention
    └─ Output: [batch_size, seq_len, 384]
    ↓
Mean Pooling
    ├─ Average across sequence dimension
    └─ Output: [batch_size, 384]
    ↓
Convert to Vec<Vec<f32>>
    ├─ Copy from GPU to CPU
    └─ Return embeddings
```

**Performance (Phase 1 Target):**
- Single text: <20ms
- Batch of 8: <40ms
- Batch of 32: <80ms

### Model Selection: MiniLM-L6-v2

**Why This Model:**
```
✅ Small size: 22M parameters (90MB on disk)
✅ Fast inference: ~10ms per text (M1 Pro)
✅ Good quality: 95%+ similarity to larger models
✅ Well-supported: 100M+ downloads on HF Hub
✅ Battle-tested: Used in production by thousands of companies
```

**Alternatives Considered:**
1. **BGE-Small-EN** - Slightly better quality, 50% slower
2. **Qwen2-0.5B** - Same quality as MLX, but 27x larger
3. **E5-Small** - Similar to MiniLM, less popular

**Decision:** Start with MiniLM, add others in Phase 4.

### Feature Flag Design

```toml
# Cargo.toml
[features]
default = ["mlx"]              # Keep MLX default for now
mlx = ["pyo3"]                  # Python MLX provider
candle = [                      # Rust Candle provider (NEW)
    "candle-core",
    "candle-nn",
    "candle-transformers",
    "tokenizers",
    "hf-hub"
]
```

**Build Commands:**
```bash
# Build with MLX (default)
cargo build

# Build with Candle
cargo build --no-default-features --features candle

# Build with both (for testing)
cargo build --features mlx,candle
```

**Runtime Configuration:**
```toml
# config.toml
[embedding]
provider = "candle"  # "mlx" | "candle"
model = "sentence-transformers/all-MiniLM-L6-v2"
device = "auto"      # "auto" | "cpu" | "metal" | "cuda"
cache_dir = "~/.cache/akidb/models"
```

---

## Implementation Details

### File Structure

```
crates/akidb-embedding/
├── Cargo.toml                    # Add Candle dependencies
├── src/
│   ├── lib.rs                    # Export CandleEmbeddingProvider
│   ├── provider.rs               # EmbeddingProvider trait (no changes)
│   ├── types.rs                  # Request/response types (no changes)
│   ├── mlx.rs                    # MLX provider (existing)
│   ├── mock.rs                   # Mock provider (existing)
│   └── candle.rs                 # NEW: Candle provider ⭐
└── tests/
    └── candle_tests.rs           # NEW: 15+ unit tests ⭐
```

### Dependencies to Add

```toml
# crates/akidb-embedding/Cargo.toml

[dependencies]
# ... existing dependencies ...

# Candle (optional, behind feature flag)
candle-core = { version = "0.8", optional = true, features = ["metal"] }
candle-nn = { version = "0.8", optional = true }
candle-transformers = { version = "0.8", optional = true }
tokenizers = { version = "0.15", optional = true }
hf-hub = { version = "0.3", optional = true, default-features = false, features = ["tokio"] }

[features]
default = ["mlx"]
mlx = ["pyo3"]
candle = ["candle-core", "candle-nn", "candle-transformers", "tokenizers", "hf-hub"]
```

### Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum CandleError {
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Model loading failed: {0}")]
    ModelLoadFailed(String),

    #[error("Tokenization failed: {0}")]
    TokenizationFailed(String),

    #[error("Inference failed: {0}")]
    InferenceFailed(String),

    #[error("Device not available: {0}")]
    DeviceNotAvailable(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

// Convert to EmbeddingError
impl From<CandleError> for EmbeddingError {
    fn from(e: CandleError) -> Self {
        match e {
            CandleError::ModelNotFound(msg) =>
                EmbeddingError::ModelNotFound(msg),
            CandleError::InvalidInput(msg) =>
                EmbeddingError::InvalidInput(msg),
            _ => EmbeddingError::Internal(e.to_string()),
        }
    }
}
```

---

## Testing Strategy

### Unit Tests (15+ tests)

**Category 1: Initialization (3 tests)**
1. `test_model_loading` - Load MiniLM model
2. `test_device_selection` - Metal > CUDA > CPU priority
3. `test_invalid_model_name` - Handle 404 from HF Hub

**Category 2: Tokenization (3 tests)**
4. `test_single_text_tokenization` - Basic tokenization
5. `test_batch_tokenization` - Batch processing
6. `test_empty_text` - Error handling
7. `test_very_long_text` - Truncation (>512 tokens)

**Category 3: Inference (4 tests)**
8. `test_single_embedding` - Generate 1 embedding
9. `test_batch_embedding` - Generate 8 embeddings
10. `test_large_batch` - Generate 32 embeddings
11. `test_embedding_dimension` - Output is 384-dim

**Category 4: Quality (3 tests)**
12. `test_embedding_consistency` - Same text → same embedding
13. `test_embedding_similarity` - Similar texts → similar embeddings
14. `test_cosine_similarity` - Validate using known examples

**Category 5: Integration (2 tests)**
15. `test_embedding_provider_trait` - Implements trait correctly
16. `test_error_handling` - Graceful failures

### Test Data

```rust
const TEST_TEXTS: &[&str] = &[
    "Machine learning is fascinating",
    "Deep learning is a subset of machine learning",
    "The quick brown fox jumps over the lazy dog",
    "Python is a popular programming language",
    "Rust is a systems programming language",
];

const EXPECTED_DIMENSION: usize = 384;
const MIN_SIMILARITY: f32 = 0.90;  // 90% similarity threshold
```

### Quality Validation

**Benchmark against MLX:**
```rust
#[tokio::test]
async fn test_candle_vs_mlx_similarity() {
    let mlx_provider = MlxEmbeddingProvider::new("qwen3-0.6b-4bit").await?;
    let candle_provider = CandleEmbeddingProvider::new(
        "sentence-transformers/all-MiniLM-L6-v2"
    ).await?;

    let test_text = "Machine learning";

    let mlx_emb = mlx_provider.embed_single(test_text).await?;
    let candle_emb = candle_provider.embed_single(test_text).await?;

    let similarity = cosine_similarity(&mlx_emb, &candle_emb);
    assert!(similarity > 0.85, "Similarity too low: {}", similarity);
}
```

**Note:** 85% threshold because different models (MiniLM vs Qwen) will have some divergence.

---

## Success Criteria

### Functional Requirements

✅ **FR1:** Load MiniLM model from Hugging Face Hub
- Metric: Download completes in <30s
- Validation: Model file exists in cache dir

✅ **FR2:** Generate embeddings for single text
- Metric: Latency <20ms per text
- Validation: Output is 384-dimensional vector

✅ **FR3:** Generate embeddings for batch
- Metric: Batch of 8 texts in <40ms
- Validation: Output has correct batch size

✅ **FR4:** GPU acceleration works
- Metric: Metal (macOS) or CUDA (Linux) detected
- Validation: `device.is_gpu()` returns true

✅ **FR5:** CPU fallback works
- Metric: Works on machines without GPU
- Validation: Tests pass with `CANDLE_DEVICE=cpu`

### Non-Functional Requirements

✅ **NFR1:** Unit test coverage >80%
- Metric: 15+ tests, all passing
- Validation: `cargo test --features candle`

✅ **NFR2:** No breaking changes
- Metric: Existing tests still pass
- Validation: `cargo test --features mlx`

✅ **NFR3:** Cross-platform support
- Metric: Works on macOS ARM + Linux x86_64
- Validation: CI passes on both platforms

✅ **NFR4:** Documentation complete
- Metric: Rustdoc for all public APIs
- Validation: `cargo doc --features candle` succeeds

✅ **NFR5:** Code quality
- Metric: Clippy passes with zero warnings
- Validation: `cargo clippy --features candle`

---

## Risks & Mitigation

### High Risks

**Risk 1: Candle API is unstable (v0.8.x)**
- **Impact:** Code breaks on minor version updates
- **Probability:** Medium (30%)
- **Mitigation:**
  - Pin exact version: `candle-core = "=0.8.0"`
  - Monitor GitHub releases weekly
  - Test before upgrading

**Risk 2: Performance doesn't meet targets (<20ms)**
- **Impact:** Phase 1 delays, replanning needed
- **Probability:** Low (10%)
- **Mitigation:**
  - Benchmark early (Day 3)
  - If slow, try lighter model (E5-Small)
  - Phase 2 optimizations can help

**Risk 3: Model compatibility issues**
- **Impact:** MiniLM doesn't work with Candle
- **Probability:** Very Low (5%)
- **Mitigation:**
  - MiniLM is well-tested with Candle
  - Fallback to all-distilroberta-v1 if needed

### Medium Risks

**Risk 4: GPU drivers not available**
- **Impact:** Slower CPU inference
- **Probability:** Medium (20%)
- **Mitigation:**
  - CPU fallback built-in
  - Document GPU setup clearly
  - Acceptable for Phase 1

**Risk 5: Embedding quality lower than expected**
- **Impact:** <85% similarity to MLX
- **Probability:** Low (15%)
- **Mitigation:**
  - Try BGE model instead
  - Document quality trade-offs
  - Offer multiple models in Phase 4

---

## Timeline & Milestones

### Day-by-Day Breakdown

**Day 1: Setup (Friday)**
- Hours: 4 hours
- Tasks: Dependencies, CI, project structure
- Deliverable: Compiles with `--features candle`

**Day 2: Model Loading (Monday)**
- Hours: 6 hours
- Tasks: HF Hub download, device selection, model loading
- Deliverable: Model loads successfully

**Day 3: Inference (Tuesday)**
- Hours: 6 hours
- Tasks: Tokenization, forward pass, mean pooling
- Deliverable: Generates embeddings

**Day 4: Testing (Wednesday)**
- Hours: 6 hours
- Tasks: Write 15+ unit tests
- Deliverable: All tests passing

**Day 5: Integration (Thursday)**
- Hours: 6 hours
- Tasks: EmbeddingProvider trait, docs, PR
- Deliverable: Ready for code review

### Milestones

**M1.1: Dependencies Added** (End of Day 1)
- ✅ Cargo.toml updated
- ✅ Builds successfully
- ✅ CI pipeline passing

**M1.2: Model Loading Works** (End of Day 2)
- ✅ Downloads from HF Hub
- ✅ Loads into GPU/CPU
- ✅ Device selection logic complete

**M1.3: Inference Pipeline Works** (End of Day 3)
- ✅ Tokenization complete
- ✅ Forward pass complete
- ✅ Generates correct embeddings

**M1.4: Tests Passing** (End of Day 4)
- ✅ 15+ unit tests written
- ✅ All tests pass
- ✅ Coverage >80%

**M1.5: Integration Complete** (End of Day 5)
- ✅ Implements EmbeddingProvider trait
- ✅ Documentation complete
- ✅ PR ready for review

---

## Deliverables

### Code Deliverables

1. **`crates/akidb-embedding/src/candle.rs`** (~400 lines)
   - `CandleEmbeddingProvider` struct
   - Model loading logic
   - Tokenization + inference
   - EmbeddingProvider trait impl

2. **`crates/akidb-embedding/Cargo.toml`** (~20 lines added)
   - Candle dependencies
   - Feature flag configuration

3. **`crates/akidb-embedding/tests/candle_tests.rs`** (~300 lines)
   - 15+ unit tests
   - Test utilities
   - Quality validation

4. **`crates/akidb-embedding/src/lib.rs`** (~5 lines added)
   - Export CandleEmbeddingProvider
   - Conditional compilation

### Documentation Deliverables

5. **Rustdoc comments** (~100 lines)
   - Public API documentation
   - Usage examples
   - Error handling docs

6. **CHANGELOG.md entry** (~20 lines)
   - Phase 1 changes
   - Breaking changes (none)
   - Migration notes

### Testing Deliverables

7. **CI Pipeline Updates** (.github/workflows)
   - Test Candle feature on macOS
   - Test Candle feature on Linux
   - Separate job from MLX tests

---

## Dependencies

### External Dependencies

**Rust Crates:**
```toml
candle-core = "0.8"           # Core tensor operations
candle-nn = "0.8"              # Neural network layers
candle-transformers = "0.8"    # BERT model implementation
tokenizers = "0.15"            # HF tokenizers (Rust bindings)
hf-hub = "0.3"                 # Download models from HF Hub
```

**System Requirements:**
- Rust 1.75+
- 4GB RAM minimum
- 1GB disk space (for model cache)
- GPU optional (Metal/CUDA)

### Internal Dependencies

**No changes required to:**
- `akidb-core` (types are compatible)
- `akidb-service` (uses EmbeddingProvider trait)
- `akidb-rest` (no direct dependency)
- `akidb-grpc` (no direct dependency)

**Changes required to:**
- `akidb-embedding/Cargo.toml` (add dependencies)
- CI pipeline (add Candle build job)

---

## Rollout Plan

### Development Phase (Days 1-4)
- Feature branch: `feature/candle-phase1-foundation`
- Local testing only
- No production deployment

### Code Review Phase (Day 5)
- Create PR with detailed description
- Request review from 2 engineers
- Address feedback within 1 day

### Merge Phase (Day 6-7, Week 2)
- Merge to `main` branch
- Feature flag keeps MLX as default
- No user impact

### Testing Phase (Week 2)
- Internal team tests Candle provider
- Compare performance vs MLX
- Collect feedback for Phase 2

---

## Success Metrics

### Phase 1 Completion Criteria

**Must Have:**
- ✅ All 15+ unit tests pass
- ✅ Generates embeddings for single text (<20ms)
- ✅ Generates embeddings for batch (1-32 texts)
- ✅ Works on macOS ARM + Linux x86_64
- ✅ Implements EmbeddingProvider trait
- ✅ Zero breaking changes
- ✅ Documentation complete
- ✅ CI pipeline passing

**Nice to Have:**
- ✅ Embedding similarity >90% vs MLX
- ✅ GPU acceleration verified
- ✅ Code coverage >85%
- ✅ Benchmark results documented

### Go/No-Go Decision (End of Week 1)

**GO if:**
- All "Must Have" criteria met
- Performance is acceptable (<30ms)
- Quality is acceptable (>85% similarity)

**NO-GO if:**
- Tests failing consistently
- Performance >50ms (too slow)
- Quality <80% similarity (too different)

**If NO-GO:**
- Investigate root cause (model? implementation?)
- Try alternative model (BGE, E5)
- Extend Phase 1 by 2-3 days

---

## Open Questions

### Q1: Should we support multiple models in Phase 1?
**Answer:** No, focus on MiniLM only. Add more models in Phase 4.

### Q2: What if GPU is not available?
**Answer:** CPU fallback is built-in and acceptable for Phase 1.

### Q3: How do we handle model updates?
**Answer:** Pin model version in config. Updates are manual in Phase 1.

### Q4: Should we cache models permanently?
**Answer:** Yes, cache in `~/.cache/akidb/models/` by default.

### Q5: What about model quantization (4-bit)?
**Answer:** Not in Phase 1. Add in Phase 7 (post-GA).

---

## Appendix

### A. Estimated Lines of Code

| File | Lines | Complexity |
|------|-------|-----------|
| `candle.rs` | 400 | Medium |
| `candle_tests.rs` | 300 | Low |
| `Cargo.toml` | 20 | Low |
| `lib.rs` | 5 | Low |
| Documentation | 100 | Low |
| **Total** | **825** | **Low-Medium** |

### B. References

- [Candle GitHub](https://github.com/huggingface/candle)
- [MiniLM Model Card](https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2)
- [Candle Examples](https://github.com/huggingface/candle/tree/main/candle-examples)
- [EmbeddingProvider Trait](../../crates/akidb-embedding/src/provider.rs)

### C. Team

**Assigned:**
- Primary: [Engineer Name] (Rust engineer)
- Reviewer: [Engineer Name] (Senior engineer)
- QA: [Engineer Name] (QA engineer, 50% time)

**Estimated Effort:**
- Development: 28 hours (3.5 days)
- Testing: 6 hours (0.75 days)
- Code Review: 4 hours (0.5 days)
- **Total: 38 hours (~1 engineer-week)**

---

**Document Version:** 1.0.0
**Status:** APPROVED
**Next Step:** Start Day 1 implementation
