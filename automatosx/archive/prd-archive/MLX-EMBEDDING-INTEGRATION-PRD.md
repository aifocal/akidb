# MLX Embedding Integration PRD for AkiDB 2.0

**Date:** 2025-11-08
**Status:** PLANNING
**Phase:** 2.5 (Embedding Service Implementation)
**Target:** v2.0.0-GA or v2.1.0
**Priority:** HIGH (Core Value Proposition)

---

## Executive Summary

AkiDB 2.0's core value proposition is **"RAM-first vector database for ARM edge devices with built-in embeddings"**. Currently, the embedding infrastructure exists (traits, mock provider) but **no actual ML backend** is implemented. This PRD defines the implementation of:

1. **Default MLX Backend**: Qwen3-Embedding-0.6B-4bit (Mac ARM, optimized)
2. **Alternative Models**: EmbeddingGemma-300M-4bit (ultra-low latency)
3. **User-Provided Mode**: Accept pre-embedded vectors (bypass ML)
4. **Config-Driven**: YAML-based model selection and parameters

**Timeline:** 2 weeks (10 working days)
**Scope:** Production-ready MLX embedding service for Mac ARM (M1/M2/M3)
**Risk:** Medium (ML integration complexity, model loading, performance tuning)

---

## Table of Contents

1. [Problem Statement](#problem-statement)
2. [Requirements](#requirements)
3. [Technical Architecture](#technical-architecture)
4. [Configuration Schema](#configuration-schema)
5. [Implementation Plan](#implementation-plan)
6. [Testing Strategy](#testing-strategy)
7. [Performance Targets](#performance-targets)
8. [Risk Assessment](#risk-assessment)
9. [Success Metrics](#success-metrics)

---

## Problem Statement

### Current State (v2.0.0-rc1)

**✅ What Exists:**
- `akidb-embedding` crate with traits (`EmbeddingProvider`)
- Mock implementation for testing (deterministic, hash-based)
- Collection metadata stores `embedding_model: String`
- API expects **users to provide pre-embedded vectors**

**❌ What's Missing:**
- No actual ML backend (MLX, ONNX, or other)
- No model loading or inference code
- No embedding generation capability
- No batching or optimization
- No model selection configuration

**Impact:**
- Users must run separate embedding service (e.g., Ollama, OpenAI API)
- No "out-of-the-box" experience for edge deployment
- Competitive disadvantage vs Milvus Lite, ChromaDB (both have built-in embeddings)

### Target State (This PRD)

**Users can:**
1. Deploy AkiDB with **zero external dependencies** for embeddings
2. Choose between **Qwen3-0.6B** (default, balanced) or **Gemma-300M** (fast)
3. Configure model via **YAML** (`embedding.model: qwen3-0.6b-4bit`)
4. Bypass ML and **provide pre-embedded vectors** (power users)
5. Get **<25ms P95 embedding latency** @ 50 QPS with batching

---

## Requirements

### Functional Requirements

#### FR-1: MLX Backend Implementation

**As a** system administrator
**I want** AkiDB to generate embeddings using Apple MLX
**So that** I can deploy on Mac ARM without external services

**Acceptance Criteria:**
- [ ] `MlxEmbeddingProvider` implements `EmbeddingProvider` trait
- [ ] Model loading from HuggingFace Hub on first use
- [ ] Lazy loading (load model on first request, cache in RAM)
- [ ] Batching support (process multiple texts in one inference call)
- [ ] L2 normalization option (configurable)
- [ ] Error handling for model load failures
- [ ] Graceful fallback to user-provided embeddings if model unavailable

#### FR-2: Qwen3-Embedding-0.6B Default Model

**As a** developer
**I want** Qwen3-0.6B-4bit as the default embedding model
**So that** I get good accuracy/speed balance out-of-the-box

**Acceptance Criteria:**
- [ ] Default model: `mlx-community/Qwen3-Embedding-0.6B-4bit-DWQ`
- [ ] Auto-download from HuggingFace Hub if not cached
- [ ] Cache location: `~/.cache/akidb/models/qwen3-0.6b-4bit/`
- [ ] Model info exposed: dimension=512, max_tokens=32768
- [ ] MTEB accuracy: 65-67% (validated via integration tests)

#### FR-3: Alternative Model Support (Gemma-300M)

**As a** performance engineer
**I want** to select EmbeddingGemma-300M for lower latency
**So that** I can optimize for speed-critical workloads

**Acceptance Criteria:**
- [ ] Alternative model: `mlx-community/embeddinggemma-300m-4bit`
- [ ] Configurable via `embedding.model: gemma-300m-4bit`
- [ ] Model info: dimension=768, max_tokens=2048
- [ ] 2x faster than Qwen3 (target: <15ms P95)
- [ ] Accuracy trade-off documented (61% vs 65-67%)

#### FR-4: User-Provided Embeddings (Bypass Mode)

**As a** power user
**I want** to provide my own embeddings
**So that** I can use custom models (OpenAI, Cohere, etc.)

**Acceptance Criteria:**
- [ ] Config option: `embedding.mode: user_provided`
- [ ] Skips model loading (no MLX dependency)
- [ ] API accepts vectors in `upsert` requests
- [ ] Dimension validation (matches collection schema)
- [ ] Mock provider still available for testing

#### FR-5: Configuration-Driven Model Selection

**As a** system administrator
**I want** to configure embedding model via YAML
**So that** I can tune deployment without code changes

**Acceptance Criteria:**
- [ ] Config file: `config.toml` or `config.yaml`
- [ ] Model selection: `qwen3-0.6b-4bit`, `gemma-300m-4bit`, `user_provided`
- [ ] Batching parameters: `batch_size`, `batch_timeout_ms`
- [ ] Cache settings: `model_cache_dir`, `max_cache_gb`
- [ ] Validation on startup (fail fast if model unavailable)

### Non-Functional Requirements

#### NFR-1: Performance

**Target Latency:**
- Single query: <100ms (cold start), <25ms (warm, batched)
- Batched (10 queries): <50ms total, <5ms per query
- Throughput: 50 QPS sustained

**Resource Limits:**
- RAM: <5GB total (1.2GB model + 3GB overhead)
- Model load time: <10s (first request)
- Disk: <2GB per model (cache)

#### NFR-2: Reliability

- 99.9% uptime (embedding service available)
- Zero data loss (failed embeddings logged, retryable)
- Graceful degradation (fallback to user-provided if model fails)

#### NFR-3: Compatibility

- **Mac ARM**: M1, M2, M3 (primary target)
- **MLX Version**: 0.20+ (latest stable)
- **Python**: 3.10-3.12 (MLX requirement)
- **Future**: NVIDIA Jetson (via ONNX), Oracle ARM Cloud (via ONNX)

---

## Technical Architecture

### Component Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                    AkiDB 2.0 - Embedding Layer              │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌───────────────────────────────────────────────────┐    │
│  │         REST API (Axum)                           │    │
│  │  POST /api/v1/collections/{id}/upsert             │    │
│  │  {                                                │    │
│  │    "text": "query text",                          │    │
│  │    "vector": [0.1, 0.2, ...] // optional         │    │
│  │  }                                                │    │
│  └──────────────┬────────────────────────────────────┘    │
│                 │                                           │
│                 ▼                                           │
│  ┌───────────────────────────────────────────────────┐    │
│  │       CollectionService                           │    │
│  │  ├─ Check if vector provided                     │    │
│  │  ├─ If not, call embedding service               │    │
│  │  └─ Insert into vector index                     │    │
│  └──────────────┬────────────────────────────────────┘    │
│                 │                                           │
│                 ▼                                           │
│  ┌───────────────────────────────────────────────────┐    │
│  │       EmbeddingRouter                             │    │
│  │  match config.embedding.mode:                     │    │
│  │    - MlxProvider => forward to MLX                │    │
│  │    - UserProvided => no-op (use provided vector) │    │
│  │    - Mock => MockEmbeddingProvider                │    │
│  └──────────────┬────────────────────────────────────┘    │
│                 │                                           │
│                 ▼                                           │
│  ┌───────────────────────────────────────────────────┐    │
│  │       MlxEmbeddingProvider (NEW)                  │    │
│  │  ┌─────────────────────────────────────────────┐  │    │
│  │  │  Python MLX Bridge (PyO3)                   │  │    │
│  │  │  ├─ Load model (lazy, cached)               │  │    │
│  │  │  ├─ Tokenize input                          │  │    │
│  │  │  ├─ Run inference (MLX accelerated)         │  │    │
│  │  │  └─ L2 normalize output                     │  │    │
│  │  └─────────────────────────────────────────────┘  │    │
│  │                                                     │    │
│  │  Model Cache:                                      │    │
│  │  ~/.cache/akidb/models/qwen3-0.6b-4bit/           │    │
│  │  ├─ model.safetensors (1.2GB)                     │    │
│  │  ├─ tokenizer.json                                │    │
│  │  └─ config.json                                   │    │
│  └───────────────────────────────────────────────────┘    │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Data Flow

```
User Request (Text)
      │
      ▼
┌──────────────────┐
│ 1. Validate      │  Check: text not empty, length < max_tokens
└────┬─────────────┘
     │
     ▼
┌──────────────────┐
│ 2. Batch Queue   │  Accumulate N requests (up to batch_size or timeout)
└────┬─────────────┘
     │
     ▼
┌──────────────────┐
│ 3. MLX Inference │  [text1, text2, ...] => [[vec1], [vec2], ...]
│  (Python)        │  - Tokenize with Qwen3 tokenizer
│                  │  - Forward pass through model
│                  │  - Extract [CLS] embeddings
│                  │  - L2 normalize (optional)
└────┬─────────────┘
     │
     ▼
┌──────────────────┐
│ 4. Return        │  BatchEmbeddingResponse {
│                  │    model: "qwen3-0.6b-4bit",
│                  │    embeddings: Vec<Vec<f32>>,
│                  │    usage: { total_tokens, duration_ms }
│                  │  }
└──────────────────┘
```

---

## Configuration Schema

### YAML Configuration (`config.yaml`)

```yaml
# AkiDB 2.0 Configuration

embedding:
  # Provider mode: mlx | user_provided | mock
  mode: mlx

  # MLX-specific configuration
  mlx:
    # Model selection (default: qwen3-0.6b-4bit)
    model: qwen3-0.6b-4bit  # or gemma-300m-4bit

    # Model cache directory (default: ~/.cache/akidb/models)
    cache_dir: /path/to/cache

    # Maximum cache size in GB (default: 10GB, auto-cleanup oldest)
    max_cache_gb: 10

    # Batching parameters
    batch_size: 10           # Max queries per batch
    batch_timeout_ms: 50     # Max wait time for batch accumulation

    # Normalization (default: true for cosine similarity)
    normalize: true

    # Device selection (default: auto)
    device: auto  # auto | gpu | cpu

    # Quantization (auto-detected from model name)
    # qwen3-0.6b-4bit => INT4 (1.2GB -> 600MB)
    # gemma-300m-4bit => INT4 (600MB -> <200MB)

# Collection defaults (if embedding used)
collection:
  default_dimension: 512  # Match Qwen3 default output
  default_metric: cosine  # Recommended for embeddings
```

### Environment Variables (Overrides)

```bash
# Embedding mode
export AKIDB_EMBEDDING_MODE=mlx

# Model selection
export AKIDB_EMBEDDING_MODEL=qwen3-0.6b-4bit

# Cache directory
export AKIDB_EMBEDDING_CACHE_DIR=/custom/cache

# Batching
export AKIDB_EMBEDDING_BATCH_SIZE=10
export AKIDB_EMBEDDING_BATCH_TIMEOUT_MS=50

# Device
export AKIDB_EMBEDDING_DEVICE=gpu
```

### Model Registry

```yaml
# Built-in model definitions (hardcoded in Rust)
models:
  qwen3-0.6b-4bit:
    huggingface_id: mlx-community/Qwen3-Embedding-0.6B-4bit-DWQ
    dimension: 512
    max_tokens: 32768
    size_mb: 600  # INT4 quantized
    mteb_score: 65-67
    recommended_for: "balanced accuracy/speed"

  gemma-300m-4bit:
    huggingface_id: mlx-community/embeddinggemma-300m-4bit
    dimension: 768
    max_tokens: 2048
    size_mb: 200  # INT4 quantized
    mteb_score: 61-65
    recommended_for: "ultra-low latency (<15ms)"
```

---

## Implementation Plan

### Week 1: MLX Bridge + Qwen3 Integration (Days 1-5)

**Day 1: Setup Python-Rust Bridge (PyO3)**
- Add `pyo3` dependency to `akidb-embedding` crate
- Create Python module structure for MLX inference
- Implement basic Python<->Rust communication
- Write "hello world" test (call Python from Rust)
- **Deliverable**: PyO3 bridge working

**Day 2: MLX Model Loader**
- Python: Implement Qwen3-0.6B-4bit model loading
- Use HuggingFace `huggingface_hub` for auto-download
- Cache model in `~/.cache/akidb/models/`
- Handle cache hits (instant load)
- Error handling for network failures
- **Deliverable**: Model loading working (Python standalone script)

**Day 3: Embedding Inference (Python)**
- Python: Implement `embed_batch(texts: List[str]) -> List[List[float]]`
- Tokenization with Qwen3 tokenizer
- MLX inference (forward pass)
- Extract [CLS] embeddings
- L2 normalization (optional)
- **Deliverable**: Python embedding script working

**Day 4: MlxEmbeddingProvider (Rust)**
- Rust: Create `MlxEmbeddingProvider` struct
- Implement `EmbeddingProvider` trait
- Call Python embedding function via PyO3
- Convert Python arrays to Rust `Vec<Vec<f32>>`
- Batching logic (accumulate requests)
- **Deliverable**: Rust provider calling Python successfully

**Day 5: Configuration + Integration**
- Add YAML config parsing (`embedding.mlx.model`)
- Implement `EmbeddingRouter` (mode selection logic)
- Integrate with `CollectionService`
- Write integration tests (E2E: text -> embedding -> insert)
- **Deliverable**: Week 1 complete, Qwen3 working end-to-end

### Week 2: Gemma + Optimization + Testing (Days 6-10)

**Day 6: EmbeddingGemma-300M Support**
- Add Gemma model to registry
- Python: Implement Gemma loading (same pattern as Qwen3)
- Config selector: `embedding.mlx.model: gemma-300m-4bit`
- Dimension validation (Gemma=768, Qwen3=512)
- **Deliverable**: Gemma model working

**Day 7: Batch Optimization**
- Implement batch queue with timeout
- Benchmark batch sizes (1, 5, 10, 20, 50)
- Tune `batch_timeout_ms` for P95 <25ms target
- Prometheus metrics: `embedding_batch_size`, `embedding_latency_ms`
- **Deliverable**: Optimized batching

**Day 8: User-Provided Mode + Fallback**
- Config: `embedding.mode: user_provided`
- Skip MLX initialization if mode != mlx
- API: Accept optional `vector` field in upsert
- Validation: vector dimension matches collection
- **Deliverable**: User-provided mode working

**Day 9: Performance Testing**
- Load test: 50 QPS sustained for 10 minutes
- Measure: P50, P95, P99 latency
- Measure: RAM usage (model + batching overhead)
- Measure: Model load time (cold start)
- CPU/memory profiling (flamegraph)
- **Deliverable**: Performance report

**Day 10: Documentation + Completion**
- Update API-TUTORIAL.md (embedding examples)
- Create EMBEDDING-GUIDE.md (model selection, tuning)
- Update DEPLOYMENT-GUIDE.md (MLX setup, cache)
- Create README in `akidb-embedding/` crate
- Completion report
- **Deliverable**: Week 2 complete ✅

---

## Testing Strategy

### Unit Tests (Target: 20 tests)

**MlxEmbeddingProvider Tests (10 tests):**
- `test_model_loading` - Model loads successfully
- `test_model_caching` - Second load is instant
- `test_embed_single_text` - Single text embedding
- `test_embed_batch` - Batch embedding (10 texts)
- `test_normalize_enabled` - L2 normalization working
- `test_normalize_disabled` - Raw embeddings
- `test_dimension_validation` - Output dimension correct
- `test_invalid_model` - Error handling for bad model
- `test_empty_input` - Error on empty text
- `test_max_tokens_exceeded` - Error on too-long text

**Configuration Tests (5 tests):**
- `test_config_parse_qwen3` - YAML parsing for Qwen3
- `test_config_parse_gemma` - YAML parsing for Gemma
- `test_config_user_provided` - User-provided mode config
- `test_config_invalid_model` - Error on unknown model
- `test_env_override` - Environment variables override YAML

**Router Tests (5 tests):**
- `test_router_mlx_mode` - Routes to MLX provider
- `test_router_user_provided_mode` - Routes to no-op
- `test_router_mock_mode` - Routes to mock (testing)
- `test_router_fallback` - Fallback on MLX failure
- `test_router_dimension_validation` - Validate vector dimensions

### Integration Tests (Target: 10 tests)

**E2E Embedding Tests (6 tests):**
- `test_e2e_qwen3_embedding` - Text -> Qwen3 -> Vector -> Insert
- `test_e2e_gemma_embedding` - Text -> Gemma -> Vector -> Insert
- `test_e2e_user_provided` - User vector -> Insert
- `test_e2e_batch_embedding` - 10 texts -> Batch -> Insert
- `test_e2e_search_after_embed` - Embed + Insert + Search + Verify recall
- `test_e2e_collection_dimension_match` - Collection dim = embedding dim

**Error Handling Tests (4 tests):**
- `test_e2e_model_not_found` - Graceful error if model unavailable
- `test_e2e_network_failure` - Retry on HuggingFace download failure
- `test_e2e_dimension_mismatch` - Error if vector dim != collection dim
- `test_e2e_fallback_to_user_provided` - Use user vector if MLX fails

### Performance Tests (Manual)

**Latency Benchmark:**
```bash
# Single query latency
vegeta attack -rate=1 -duration=60s | vegeta report

# Batch latency (10 QPS)
vegeta attack -rate=10 -duration=600s | vegeta report

# Sustained load (50 QPS)
vegeta attack -rate=50 -duration=600s | vegeta report
```

**Target Results:**
- P50: <15ms
- P95: <25ms
- P99: <50ms
- Throughput: 50 QPS sustained

---

## Performance Targets

### Latency Targets

| Metric | Qwen3-0.6B-4bit | Gemma-300M-4bit | Acceptable? |
|--------|----------------|-----------------|-------------|
| **Single Query (Cold)** | <100ms | <50ms | ⚠️ One-time cost |
| **Single Query (Warm)** | <50ms | <30ms | ✅ OK |
| **Batch (10) Total** | <100ms | <50ms | ✅ OK |
| **Batch (10) Per Query** | <10ms | <5ms | ✅ Excellent |
| **P50 @ 50 QPS** | <15ms | <10ms | ✅ Target met |
| **P95 @ 50 QPS** | <25ms | <15ms | ✅ Target met |
| **P99 @ 50 QPS** | <50ms | <25ms | ✅ Acceptable |

### Resource Targets

| Resource | Qwen3-0.6B-4bit | Gemma-300M-4bit | Limit |
|----------|----------------|-----------------|-------|
| **Model Size (Disk)** | 600MB (INT4) | <200MB (INT4) | <2GB ✅ |
| **RAM (Idle)** | 1.2GB | 600MB | <5GB ✅ |
| **RAM (Active)** | 3-5GB | 2-3GB | <5GB ⚠️ |
| **Model Load Time** | <10s | <5s | <10s ✅ |
| **Throughput** | 50 QPS | 100 QPS | ≥50 QPS ✅ |

### Accuracy Targets

| Benchmark | Qwen3-0.6B | Gemma-300M | Target |
|-----------|------------|------------|--------|
| **MTEB Multilingual** | 65-67% | 61-65% | ≥60% ✅ |
| **MTEB English** | 68-70% | 63% | ≥60% ✅ |
| **Recall @ k=10** | >95% | >90% | ≥90% ✅ |

---

## Risk Assessment

### High Risk

**Risk 1: PyO3 Integration Complexity**
- **Probability:** Medium (40%)
- **Impact:** HIGH (blocks entire feature)
- **Scenario:** Python<->Rust bridge unstable, crashes, memory leaks
- **Mitigation:**
  - Use stable PyO3 version (0.20+)
  - Comprehensive error handling (catch Python exceptions)
  - Resource cleanup (Python GIL management)
  - Fallback to user-provided mode if PyO3 fails
- **Contingency:** Ship with user-provided mode only, defer MLX to v2.1

**Risk 2: Model Download Failures**
- **Probability:** Medium (30%)
- **Impact:** MEDIUM (runtime errors for users)
- **Scenario:** HuggingFace Hub unreachable, network timeouts
- **Mitigation:**
  - Cache models locally (never re-download)
  - Retry logic with exponential backoff (3 retries)
  - Clear error messages (guide users to manual download)
  - Offline mode (pre-download models during setup)
- **Contingency:** Provide model download script (manual fallback)

**Risk 3: Performance Not Meeting Targets**
- **Probability:** Medium (35%)
- **Impact:** MEDIUM (user dissatisfaction)
- **Scenario:** P95 >25ms even with batching
- **Mitigation:**
  - Benchmark early (Day 7, before final integration)
  - Tune batch_size and batch_timeout_ms aggressively
  - Profile Python code (remove bottlenecks)
  - Document trade-offs (accuracy vs speed)
- **Contingency:** Recommend Gemma-300M for speed-critical workloads

### Medium Risk

**Risk 4: RAM Usage Exceeds 5GB**
- **Probability:** Low (25%)
- **Impact:** MEDIUM (deployment constraints)
- **Scenario:** Model + batching overhead exceeds budget
- **Mitigation:**
  - Monitor RAM with profiler (Day 9)
  - Reduce batch_size if needed (trade throughput for RAM)
  - Document RAM requirements clearly
- **Contingency:** Offer INT4 quantization (lower RAM, slight accuracy loss)

**Risk 5: Dimension Mismatch Errors**
- **Probability:** Low (20%)
- **Impact:** LOW (runtime errors, easy to fix)
- **Scenario:** User creates collection with wrong dimension
- **Mitigation:**
  - Auto-detect dimension from model (Qwen3=512, Gemma=768)
  - Validate at collection creation time
  - Clear error messages ("Expected 512, got 768")
- **Contingency:** Allow dimension override in config

---

## Success Metrics

### Code Quality Metrics

**Target:**
- 30+ tests passing (20 unit + 10 integration)
- Zero compiler errors
- Zero critical warnings
- Clippy clean

**Measurement:**
```bash
cargo test -p akidb-embedding
cargo clippy -p akidb-embedding -- -D warnings
```

### Performance Metrics

**Target:**
- P95 latency <25ms @ 50 QPS
- Model load time <10s
- RAM usage <5GB (active)
- Throughput ≥50 QPS

**Measurement:**
```bash
# Load test
vegeta attack -rate=50 -duration=600s | vegeta report

# RAM profiling
valgrind --tool=massif ./target/release/akidb-rest
```

### Accuracy Metrics

**Target:**
- MTEB score ≥65% (Qwen3)
- Recall @ k=10 ≥95% (HNSW)
- Embedding quality comparable to OpenAI text-embedding-3-small

**Measurement:**
- Run MTEB benchmark suite (Python)
- Compare with baseline (OpenAI, Cohere)

### Adoption Metrics (Post-Release)

**Target (3 months):**
- 80% of users enable built-in embeddings (vs user-provided)
- 70% use default Qwen3 model
- 30% use Gemma for speed
- <5% report embedding-related errors

**Measurement:**
- Telemetry (opt-in): `embedding_mode` distribution
- GitHub issues tagged "embedding"
- User survey responses

---

## Alternatives Considered

### Alternative 1: ONNX Runtime Instead of MLX

**Pros:**
- Cross-platform (Mac ARM, Jetson, Oracle ARM)
- Mature ecosystem (Microsoft-backed)

**Cons:**
- Slower than MLX on Apple Silicon (3-5x)
- Larger memory footprint
- No Apple Neural Engine acceleration

**Verdict:** REJECT for initial implementation. Consider for Phase 9 (multi-platform).

### Alternative 2: External Embedding Service (Ollama, llama.cpp)

**Pros:**
- No Python dependency
- Mature inference engines
- Easy to swap models

**Cons:**
- Users must run separate service (not "out-of-the-box")
- Network overhead (localhost HTTP calls)
- Complexity (two processes to manage)

**Verdict:** REJECT. Defeats "built-in embeddings" value proposition.

### Alternative 3: Ship Models in Binary (Embedded)

**Pros:**
- Zero download time
- Offline-first

**Cons:**
- Binary size 600MB+ (unacceptable)
- Hard to update models (need new release)
- Legal issues (model license distribution)

**Verdict:** REJECT. Use download-on-first-use pattern instead.

---

## Appendix A: HuggingFace Model URLs

### Qwen3-Embedding-0.6B-4bit

**HuggingFace:** https://huggingface.co/mlx-community/Qwen3-Embedding-0.6B-4bit-DWQ

**Files:**
- `model.safetensors` (600MB, INT4 quantized)
- `tokenizer.json` (2MB)
- `config.json` (1KB)

**License:** Apache 2.0 (commercial use allowed)

### EmbeddingGemma-300M-4bit

**HuggingFace:** https://huggingface.co/mlx-community/embeddinggemma-300m-4bit

**Files:**
- `model.safetensors` (<200MB, INT4 quantized)
- `tokenizer.json` (2MB)
- `config.json` (1KB)

**License:** Gemma License (Google, commercial use with restrictions)

---

## Appendix B: Python Dependencies

### Required Packages

```txt
# MLX ecosystem
mlx>=0.20.0
mlx-lm>=0.18.0

# HuggingFace
transformers>=4.45.0
huggingface-hub>=0.24.0

# Utilities
numpy>=1.26.0
safetensors>=0.4.0
```

### Installation

```bash
# Create virtual environment
python3 -m venv .venv
source .venv/bin/activate

# Install dependencies
pip install mlx mlx-lm transformers huggingface-hub numpy safetensors
```

---

## Appendix C: API Examples

### Example 1: Upsert with Auto-Embedding (Qwen3)

```bash
# POST /api/v1/collections/{id}/upsert
curl -X POST http://localhost:8080/api/v1/collections/123/upsert \
  -H "Content-Type: application/json" \
  -d '{
    "documents": [
      {
        "text": "AkiDB is a RAM-first vector database for ARM edge devices",
        "metadata": {"source": "docs"}
      }
    ]
  }'

# Response
{
  "inserted": 1,
  "embeddings_generated": true,
  "model": "qwen3-0.6b-4bit",
  "usage": {
    "total_tokens": 12,
    "duration_ms": 18
  }
}
```

### Example 2: Upsert with User-Provided Embedding

```bash
# POST /api/v1/collections/{id}/upsert
curl -X POST http://localhost:8080/api/v1/collections/123/upsert \
  -H "Content-Type: application/json" \
  -d '{
    "documents": [
      {
        "text": "Optional metadata",
        "vector": [0.1, 0.2, ..., 0.512],  # 512-dim vector
        "metadata": {"source": "custom"}
      }
    ]
  }'

# Response
{
  "inserted": 1,
  "embeddings_generated": false,
  "usage": {
    "duration_ms": 2
  }
}
```

### Example 3: Switch to Gemma Model

```yaml
# config.yaml
embedding:
  mode: mlx
  mlx:
    model: gemma-300m-4bit  # Switch from qwen3
```

```bash
# Restart AkiDB
# All subsequent embeddings use Gemma (768-dim, faster)
```

---

**Document Version:** 1.0
**Last Updated:** 2025-11-08
**Status:** PLANNING - Ready for Review ✅
**Next Step:** Stakeholder approval → Begin Week 1 implementation
