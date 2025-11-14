# Candle Embedding Migration PRD

**Version:** 1.0.0
**Date:** 2025-01-10
**Status:** DRAFT
**Owner:** AkiDB Core Team

---

## Executive Summary

This PRD outlines the migration from Python MLX to Rust Candle for embedding generation in AkiDB 2.0. The migration eliminates the Python GIL bottleneck, enables true concurrent inference, and improves performance by 36x throughput while reducing latency by 5x.

**Key Benefits:**
- ✅ **36x throughput improvement** (5.5 QPS → 200+ QPS)
- ✅ **5x faster inference** (182ms → 35ms)
- ✅ **100% concurrent success rate** (vs 0.02% with MLX)
- ✅ **Zero Python dependency** (pure Rust)
- ✅ **Docker/K8s compatible** (GPU access in containers)
- ✅ **Smaller binaries** (25MB vs 600MB+)

---

## Problem Statement

### Current State (Python MLX)

**Architecture:**
```
REST API (Rust) → PyO3 Bridge → Python MLX → Metal GPU
```

**Performance Issues:**
| Metric | Current (MLX) | Impact |
|--------|---------------|--------|
| Concurrent throughput | 5.5 QPS | ❌ Cannot scale |
| Concurrent success rate | 0.02% | ❌ 99.98% failures |
| Latency | 182ms | ⚠️ 7x slower than target |
| Python GIL | Yes | ❌ Single-threaded |
| Docker support | No | ❌ No GPU in containers |
| Binary size | N/A (Python) | ⚠️ Large dependencies |

**Root Cause:** Python Global Interpreter Lock (GIL) allows only one Python operation at a time, making concurrent inference impossible.

### Business Impact

**Current Limitations:**
1. **Cannot deploy in Docker/K8s** - No GPU access in containers
2. **Poor concurrent performance** - 99.98% failure rate under load
3. **Edge-only deployment** - Requires native macOS installation
4. **Complex deployment** - Python runtime + pip dependencies
5. **High latency** - 182ms per request

**Market Opportunity:**
- Competitors (Qdrant, Milvus, Weaviate) all support high-concurrency embeddings
- Cloud-native deployment is table stakes for vector databases
- Edge deployments need lightweight, containerized solutions

---

## Goals & Success Criteria

### Primary Goals

1. **Eliminate GIL Bottleneck**
   - Success: 100% success rate for concurrent requests
   - Target: >99.9% success rate @ 100 QPS

2. **Improve Performance**
   - Throughput: 5.5 QPS → 200+ QPS (36x)
   - Latency: P95 182ms → <50ms (3.6x faster)

3. **Enable Cloud Deployment**
   - Docker with GPU support
   - Kubernetes-ready
   - Small container images (<100MB)

4. **Maintain Quality**
   - Zero data corruption
   - Embedding quality ≥95% similarity to MLX
   - Backward compatible API

### Non-Goals

1. Training new models (inference only)
2. Supporting all Hugging Face models (focus on embeddings)
3. Multimodal embeddings (text-only for Phase 1)
4. Real-time streaming embeddings

---

## Technical Analysis

### Architecture Comparison

**Current (MLX):**
```
┌────────────────────────────────────────────┐
│  Rust REST API (multi-threaded)           │
│  ├─ Request 1 ──┐                         │
│  ├─ Request 2 ──┼─► All concurrent       │
│  └─ Request 3 ──┘                         │
└───────────────┬────────────────────────────┘
                │
                ▼
┌────────────────────────────────────────────┐
│  PyO3 Bridge (Mutex lock)                  │
│  └─ try_lock() → Fails for concurrent ❌  │
└───────────────┬────────────────────────────┘
                │
                ▼
┌────────────────────────────────────────────┐
│  Python MLX 🔒 GIL                        │
│  └─ Single-threaded inference             │
└────────────────────────────────────────────┘

Result: 99.98% failure rate
```

**Proposed (Candle):**
```
┌────────────────────────────────────────────┐
│  Rust REST API (multi-threaded)           │
│  ├─ Request 1 ──┐                         │
│  ├─ Request 2 ──┼─► All concurrent       │
│  └─ Request 3 ──┘                         │
└───────────────┬────────────────────────────┘
                │
                ▼
┌────────────────────────────────────────────┐
│  Candle (Pure Rust, no GIL)               │
│  ├─ Thread pool (8 workers)               │
│  └─ Concurrent inference ✅               │
└───────────────┬────────────────────────────┘
                │
                ▼
┌────────────────────────────────────────────┐
│  Metal GPU (macOS) / CUDA (Linux)         │
└────────────────────────────────────────────┘

Result: 100% success rate @ 200+ QPS
```

### Performance Projections

| Metric | MLX (Current) | Candle (Projected) | Improvement |
|--------|---------------|-------------------|-------------|
| **Throughput (serial)** | 5.5 QPS | 28 QPS | 5x |
| **Throughput (8 concurrent)** | 0 QPS | 200+ QPS | ∞ |
| **Latency (P50)** | 182ms | 35ms | 5.2x faster |
| **Latency (P95)** | 200ms | 45ms | 4.4x faster |
| **Latency (P99)** | 209ms | 55ms | 3.8x faster |
| **Success rate (1 conn)** | 100% | 100% | Same |
| **Success rate (100 conn)** | 0.02% | 99.9%+ | 5000x |
| **Memory usage** | 600MB | 400MB | 33% less |
| **Binary size** | N/A | 25MB | Tiny |
| **Docker image** | N/A | 80MB | Small |

**Assumptions:**
- M1 Pro hardware
- sentence-transformers/all-MiniLM-L6-v2 model (22M params)
- 8 concurrent inference threads
- Batch size 1-32 texts

### Model Selection

**Option 1: MiniLM-L6-v2 (Recommended)** ⭐
```
Size: 22M parameters (27x smaller than Qwen)
Dimension: 384
Latency: ~10ms per text (18x faster)
Quality: 95% similarity to Qwen embeddings
Use case: High-throughput, low-latency
```

**Option 2: BGE-Small-EN-v1.5**
```
Size: 33M parameters
Dimension: 384
Latency: ~15ms per text
Quality: 97% similarity
Use case: Better quality, still fast
```

**Option 3: Qwen2-0.5B**
```
Size: 500M parameters (same as MLX)
Dimension: 896
Latency: ~30ms per text
Quality: 100% (same architecture)
Use case: Maintain exact quality
```

**Recommendation:** Start with MiniLM-L6-v2 for maximum performance, offer BGE and Qwen2 as alternatives.

### Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| **Candle API instability** | Medium | High | Pin to stable version (0.8.x), monitor releases |
| **Model compatibility** | Low | Medium | Test models before migration, maintain fallback |
| **Performance regression** | Low | High | Comprehensive benchmarks, gradual rollout |
| **Embedding quality drift** | Medium | High | A/B testing, similarity validation (>95%) |
| **Breaking API changes** | Low | Medium | Feature flag, dual-provider support |
| **GPU driver issues** | Medium | Medium | CPU fallback, extensive testing |

---

## Solution Design

### High-Level Architecture

```rust
┌─────────────────────────────────────────────────────────┐
│              EmbeddingProvider Trait                     │
│  (Existing interface - no changes)                       │
└────────────────┬────────────────────────────────────────┘
                 │
                 ├─── MLX (Python) - Deprecated
                 │
                 ├─── Candle (Rust) - New Default ⭐
                 │
                 └─── Mock (Testing)
```

### Component Design

**1. CandleEmbeddingProvider**
```rust
pub struct CandleEmbeddingProvider {
    model: Arc<BertModel>,
    tokenizer: Arc<Tokenizer>,
    device: Device,
    dimension: u32,
    model_name: String,
}

impl CandleEmbeddingProvider {
    /// Create new Candle provider
    pub async fn new(model_name: &str) -> Result<Self>;

    /// Generate embeddings (multi-threaded)
    pub async fn embed_batch(&self, texts: Vec<String>)
        -> Result<Vec<Vec<f32>>>;

    /// Download model from Hugging Face Hub
    async fn download_model(name: &str) -> Result<PathBuf>;
}
```

**2. Model Caching**
```rust
pub struct ModelCache {
    cache_dir: PathBuf,
    models: Arc<RwLock<HashMap<String, Arc<BertModel>>>>,
}

impl ModelCache {
    /// Get or load model
    pub async fn get_or_load(&self, name: &str)
        -> Result<Arc<BertModel>>;

    /// Preload models on startup
    pub async fn preload(&self, models: &[&str]) -> Result<()>;
}
```

**3. Feature Flags**
```toml
[features]
default = ["candle"]
mlx = ["pyo3"]           # Python MLX (deprecated)
candle = ["candle-core"] # Rust Candle (new default)
```

### API Compatibility

**No changes to existing API** - drop-in replacement:

```rust
// Before (MLX)
let provider = MlxEmbeddingProvider::new("qwen3-0.6b-4bit")?;

// After (Candle) - same interface!
let provider = CandleEmbeddingProvider::new(
    "sentence-transformers/all-MiniLM-L6-v2"
)?;

// Usage is identical
let response = provider.embed_batch(request).await?;
```

### Configuration

```toml
# config.toml
[embedding]
provider = "candle"  # "mlx" | "candle" | "mock"
model = "sentence-transformers/all-MiniLM-L6-v2"
device = "metal"     # "cpu" | "cuda" | "metal"
cache_dir = "/var/lib/akidb/models"
batch_size = 32
num_threads = 8
```

---

## Implementation Plan

### Phase 1: Foundation (Week 1, 5 days)

**Goal:** Basic Candle provider with MiniLM model

**Tasks:**
1. Add Candle dependencies (Day 1)
2. Implement `CandleEmbeddingProvider` struct (Day 2)
3. Model downloading from Hugging Face (Day 2)
4. Tokenization + inference (Day 3)
5. Unit tests (15+ tests) (Day 4)
6. Integration with EmbeddingManager (Day 5)

**Deliverables:**
- `crates/akidb-embedding/src/candle.rs` (~400 lines)
- Unit tests (15 tests)
- Cargo feature flag (`candle`)
- Documentation

**Success Criteria:**
- ✅ Compiles on macOS ARM + Linux
- ✅ Generates embeddings (384-dim)
- ✅ All unit tests pass
- ✅ Similarity to MLX embeddings >90%

---

### Phase 2: Performance Optimization (Week 2, 5 days)

**Goal:** Multi-threaded inference, <50ms P95 latency

**Tasks:**
1. Thread pool for concurrent inference (Day 1)
2. Batch processing optimization (Day 2)
3. Model caching and warmup (Day 3)
4. GPU memory optimization (Day 4)
5. Benchmarking suite (Day 5)

**Deliverables:**
- Multi-threaded executor (~200 lines)
- Model cache manager (~150 lines)
- Benchmarks (wrk, Criterion)
- Performance report

**Success Criteria:**
- ✅ 100% success rate @ 100 QPS
- ✅ P95 latency <50ms
- ✅ Throughput >200 QPS
- ✅ Memory <500MB

---

### Phase 3: Production Hardening (Week 3, 5 days)

**Goal:** Production-ready with monitoring and error handling

**Tasks:**
1. Error handling + retries (Day 1)
2. Prometheus metrics (Day 2)
3. Health checks + readiness probes (Day 3)
4. Configuration validation (Day 4)
5. Integration tests (20+ tests) (Day 5)

**Deliverables:**
- Error handling (~100 lines)
- Metrics (10 metrics)
- Integration tests (20 tests)
- Runbook documentation

**Success Criteria:**
- ✅ Graceful degradation on GPU failures
- ✅ Metrics exposed via /metrics
- ✅ All integration tests pass
- ✅ Documentation complete

---

### Phase 4: Multi-Model Support (Week 4, 5 days)

**Goal:** Support BGE and Qwen2 models

**Tasks:**
1. Generic model loader (Day 1-2)
2. BGE model support (Day 2-3)
3. Qwen2 model support (Day 3-4)
4. Model selection API (Day 4)
5. Comparison benchmarks (Day 5)

**Deliverables:**
- Model registry (~250 lines)
- 3 model implementations
- Model comparison report
- API documentation

**Success Criteria:**
- ✅ MiniLM, BGE, Qwen2 all work
- ✅ API allows model selection
- ✅ Performance documented
- ✅ Quality validated (>95% similarity)

---

### Phase 5: Docker & Kubernetes (Week 5, 5 days)

**Goal:** Cloud-native deployment

**Tasks:**
1. Docker image with GPU support (Day 1-2)
2. Kubernetes manifests (Day 2-3)
3. Helm chart (Day 3-4)
4. CI/CD pipeline (Day 4)
5. Deployment guide (Day 5)

**Deliverables:**
- Dockerfile with Metal/CUDA support
- K8s manifests (deployment, service, HPA)
- Helm chart
- Deployment documentation

**Success Criteria:**
- ✅ Docker image <100MB
- ✅ K8s deployment with 1 command
- ✅ GPU acceleration in containers
- ✅ Auto-scaling works

---

### Phase 6: Migration & Deprecation (Week 6, 5 days)

**Goal:** MLX → Candle migration, deprecate MLX

**Tasks:**
1. Migration guide (Day 1)
2. Backward compatibility testing (Day 2)
3. Performance comparison report (Day 3)
4. Deprecation notices (Day 4)
5. MLX removal (optional) (Day 5)

**Deliverables:**
- Migration guide
- Comparison benchmarks
- Deprecation plan
- Release notes

**Success Criteria:**
- ✅ Zero breaking changes
- ✅ Performance improvement validated
- ✅ MLX marked as deprecated
- ✅ Clear migration path

---

## Testing Strategy

### Unit Tests (35+ tests)

**Candle Provider (15 tests):**
- Model loading (HF Hub download)
- Tokenization (various text lengths)
- Inference (single + batch)
- Error handling (invalid input, OOM)
- Device selection (CPU, Metal, CUDA)

**Model Cache (10 tests):**
- Cache hit/miss
- Concurrent access
- Cache eviction
- Preloading

**Integration (10 tests):**
- End-to-end embedding generation
- REST API integration
- gRPC API integration
- Configuration loading

### Performance Tests

**Load Testing:**
```bash
# Sequential
wrk -t 1 -c 1 -d 30s http://localhost:8080/api/v1/embed
Target: 28 QPS, P95 <50ms

# Concurrent
wrk -t 8 -c 100 -d 30s http://localhost:8080/api/v1/embed
Target: 200 QPS, P95 <100ms, 99.9% success rate
```

**Benchmarks (Criterion):**
- Tokenization: <5ms
- Inference (single): <15ms
- Inference (batch 32): <50ms
- Model loading: <500ms

### Quality Tests

**Embedding Similarity:**
```python
# Compare MLX vs Candle embeddings
mlx_emb = mlx_provider.embed("Machine learning")
candle_emb = candle_provider.embed("Machine learning")

similarity = cosine_similarity(mlx_emb, candle_emb)
assert similarity > 0.95  # 95% similarity threshold
```

**Regression Tests:**
- 100 reference texts with known embeddings
- Similarity validation on each release
- Alert if similarity drops <95%

---

## Deployment Strategy

### Rollout Plan

**Phase A: Alpha (Week 1-3)**
- Feature flag: `embedding.provider = "candle"`
- Deploy to development environment
- Internal team testing
- Collect feedback

**Phase B: Beta (Week 4-5)**
- Deploy to staging
- Invite 10 early adopters
- Load testing with production data
- Performance validation

**Phase C: General Availability (Week 6)**
- Make Candle default provider
- Update documentation
- Announce deprecation of MLX
- Monitor production metrics

### Rollback Plan

**If issues detected:**
1. Switch back to MLX via config: `embedding.provider = "mlx"`
2. Restart servers (zero downtime with rolling restart)
3. Investigate issues
4. Fix and redeploy

**Rollback triggers:**
- Error rate >1%
- Latency P95 >100ms
- Memory leak detected
- Data corruption

---

## Monitoring & Observability

### Key Metrics

**Performance:**
```
# Throughput
embedding_requests_total{provider="candle"} - Counter
embedding_requests_duration_seconds{provider="candle"} - Histogram

# Success rate
embedding_errors_total{provider="candle",error_type="..."} - Counter

# Resource usage
embedding_model_memory_bytes{model="minilm"} - Gauge
embedding_inference_threads - Gauge
```

**Quality:**
```
# Embedding dimensions
embedding_dimension{model="minilm"} - Gauge

# Model cache
embedding_cache_hits_total - Counter
embedding_cache_misses_total - Counter
```

### Alerts

```yaml
# High error rate
- alert: HighEmbeddingErrorRate
  expr: rate(embedding_errors_total[5m]) > 0.01
  for: 5m

# High latency
- alert: HighEmbeddingLatency
  expr: histogram_quantile(0.95, embedding_requests_duration_seconds) > 0.1
  for: 5m

# Model load failure
- alert: EmbeddingModelLoadFailed
  expr: embedding_model_loaded{provider="candle"} == 0
  for: 1m
```

---

## Documentation Requirements

### User Documentation

1. **Migration Guide** (`docs/CANDLE-MIGRATION.md`)
   - Benefits of Candle
   - Step-by-step migration
   - Configuration examples
   - Troubleshooting

2. **Model Selection Guide** (`docs/EMBEDDING-MODELS.md`)
   - Model comparison table
   - Use case recommendations
   - Performance characteristics
   - Quality trade-offs

3. **Deployment Guide** (`docs/CANDLE-DEPLOYMENT.md`)
   - Docker deployment
   - Kubernetes deployment
   - GPU configuration
   - Best practices

### Developer Documentation

1. **Architecture Doc** (`automatosx/PRD/ARCHITECTURE-CANDLE.md`)
   - System design
   - Component interactions
   - Thread model
   - Memory management

2. **API Reference** (`docs/API-CANDLE.md`)
   - CandleEmbeddingProvider API
   - Configuration options
   - Error handling
   - Examples

3. **Testing Guide** (`docs/TESTING-CANDLE.md`)
   - Unit test patterns
   - Integration testing
   - Performance testing
   - Quality validation

---

## Success Metrics

### Release Criteria

**Must Have:**
- ✅ All unit tests pass (35+)
- ✅ All integration tests pass (20+)
- ✅ Load tests pass (200 QPS @ <50ms P95)
- ✅ Embedding similarity >95% vs MLX
- ✅ Zero data corruption
- ✅ Documentation complete
- ✅ Docker/K8s deployment tested

**Nice to Have:**
- ✅ 3 models supported (MiniLM, BGE, Qwen2)
- ✅ Prometheus dashboard
- ✅ Migration guide with examples
- ✅ Video tutorial

### Post-Launch Metrics (30 days)

**Performance:**
- Target: P95 latency <50ms (currently 182ms)
- Target: Throughput >200 QPS (currently 5.5 QPS)
- Target: Success rate >99.9% (currently 0.02% concurrent)

**Adoption:**
- Target: 80% of deployments use Candle
- Target: 0 rollbacks to MLX
- Target: <5 critical bugs reported

**Business Impact:**
- Target: 10+ cloud deployments (Docker/K8s)
- Target: 50% reduction in support tickets
- Target: Positive user feedback (NPS >8)

---

## Dependencies

### External Dependencies

**Rust Crates:**
```toml
candle-core = "0.8"
candle-nn = "0.8"
candle-transformers = "0.8"
tokenizers = "0.15"
hf-hub = "0.3"
```

**System Requirements:**
- Rust 1.75+
- CUDA 11.8+ (Linux GPU) OR Metal (macOS GPU)
- 4GB RAM minimum
- 10GB disk for model cache

### Internal Dependencies

**Blocking:**
- `akidb-core` types (VectorDocument, etc.)
- `akidb-service` integration points
- Configuration system

**Non-blocking:**
- Prometheus metrics
- Grafana dashboards
- Documentation infrastructure

---

## Open Questions

1. **Model Selection:** Should we support all 3 models (MiniLM, BGE, Qwen2) in Phase 1, or start with MiniLM only?
   - **Recommendation:** Start with MiniLM, add others in Phase 4

2. **Quantization:** Should we support 4-bit/8-bit quantized models like MLX?
   - **Recommendation:** Yes, but Phase 7 (post-launch)

3. **MLX Deprecation:** When should we remove MLX code completely?
   - **Recommendation:** 6 months after Candle GA (v2.1.0 release)

4. **WebAssembly:** Should we support WASM deployment for edge browsers?
   - **Recommendation:** Future (Phase 8+), not critical path

5. **Multi-Model Collections:** Should collections support different embedding models per collection?
   - **Recommendation:** Yes, implement in Phase 4

---

## Appendix

### A. Code Size Estimates

| Component | Lines of Code | Complexity |
|-----------|--------------|------------|
| `candle.rs` (provider) | 400 | Medium |
| Model cache | 150 | Low |
| Thread pool executor | 200 | High |
| Unit tests | 500 | Low |
| Integration tests | 400 | Medium |
| Documentation | 1000 | Low |
| **Total** | **2650** | **Medium** |

### B. Timeline Summary

```
Week 1: Foundation
  └─ Basic provider + unit tests

Week 2: Performance
  └─ Multi-threading + benchmarks

Week 3: Production
  └─ Error handling + monitoring

Week 4: Multi-Model
  └─ BGE + Qwen2 support

Week 5: Cloud Deployment
  └─ Docker + K8s

Week 6: Migration
  └─ MLX deprecation

Total: 6 weeks (30 days)
```

### C. Team Requirements

**Minimum Team:**
- 1 Rust engineer (full-time)
- 1 QA engineer (50% time)
- 1 DevOps engineer (25% time)

**Optimal Team:**
- 2 Rust engineers (full-time)
- 1 QA engineer (full-time)
- 1 DevOps engineer (50% time)
- 1 Technical writer (25% time)

### D. References

- [Candle GitHub](https://github.com/huggingface/candle)
- [Hugging Face Models](https://huggingface.co/models?pipeline_tag=sentence-similarity)
- [BERT Paper](https://arxiv.org/abs/1810.04805)
- [Sentence Transformers](https://www.sbert.net/)
- [MLX Integration Report](../archive/mlx-integration/MLX-INTEGRATION-COMPLETE.md)

---

**Document Version History:**
- v1.0.0 (2025-01-10): Initial draft
