# Candle Migration: 6-Week Action Plan

**Version:** 1.0.0
**Date:** 2025-01-10
**Owner:** AkiDB Core Team
**Status:** READY TO EXECUTE

---

## Overview

This document provides a detailed, day-by-day action plan for migrating from Python MLX to Rust Candle embedding provider. The plan spans 6 weeks (30 working days) with clear deliverables, tasks, and success criteria for each phase.

**Total Effort:** 6 weeks
**Team Size:** 1-2 Rust engineers + QA + DevOps
**Risk Level:** Medium
**Business Impact:** High (36x performance improvement)

---

## Phase 1: Foundation (Week 1, Days 1-5)

**Goal:** Implement basic Candle provider with MiniLM model support

### Day 1: Project Setup & Dependencies

**Tasks:**
1. ☐ Create feature branch: `feature/candle-embedding`
2. ☐ Add Candle dependencies to `akidb-embedding/Cargo.toml`
3. ☐ Add feature flags: `candle = ["candle-core", "candle-nn", "candle-transformers"]`
4. ☐ Create `crates/akidb-embedding/src/candle.rs` skeleton
5. ☐ Configure CI for Candle builds

**Deliverables:**
```toml
# crates/akidb-embedding/Cargo.toml
[features]
default = ["mlx"]  # Keep MLX as default for now
mlx = ["pyo3"]
candle = [
    "candle-core",
    "candle-nn",
    "candle-transformers",
    "tokenizers",
    "hf-hub"
]

[dependencies]
candle-core = { version = "0.8", optional = true, features = ["metal"] }
candle-nn = { version = "0.8", optional = true }
candle-transformers = { version = "0.8", optional = true }
tokenizers = { version = "0.15", optional = true }
hf-hub = { version = "0.3", optional = true }
```

**Success Criteria:**
- ✅ `cargo build --features candle` succeeds
- ✅ `cargo test --features candle` compiles (no tests yet)
- ✅ CI pipeline runs Candle builds

**Estimated Time:** 4 hours

---

### Day 2: Model Download & Loading

**Tasks:**
1. ☐ Implement `CandleEmbeddingProvider` struct
2. ☐ Implement model download from Hugging Face Hub
3. ☐ Implement model loading (safetensors format)
4. ☐ Implement device selection (CPU/Metal/CUDA)
5. ☐ Add error handling for model loading

**Code Skeleton:**
```rust
// crates/akidb-embedding/src/candle.rs

use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use hf_hub::{api::sync::Api, Repo, RepoType};

pub struct CandleEmbeddingProvider {
    model: Arc<BertModel>,
    tokenizer: Arc<Tokenizer>,
    device: Device,
    model_name: String,
    dimension: u32,
}

impl CandleEmbeddingProvider {
    /// Create new Candle embedding provider
    pub async fn new(model_name: &str) -> EmbeddingResult<Self> {
        // 1. Download model from Hugging Face
        let api = Api::new()?;
        let repo = api.repo(Repo::new(
            model_name.to_string(),
            RepoType::Model,
        ));

        let model_path = repo.get("model.safetensors")?;
        let config_path = repo.get("config.json")?;
        let tokenizer_path = repo.get("tokenizer.json")?;

        // 2. Load config
        let config: Config = serde_json::from_str(
            &std::fs::read_to_string(config_path)?
        )?;

        // 3. Select device
        let device = Self::select_device()?;

        // 4. Load model weights
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(
                &[model_path],
                candle_core::DType::F32,
                &device,
            )?
        };

        let model = BertModel::load(vb, &config)?;
        let tokenizer = Tokenizer::from_file(tokenizer_path)?;

        Ok(Self {
            model: Arc::new(model),
            tokenizer: Arc::new(tokenizer),
            device,
            model_name: model_name.to_string(),
            dimension: config.hidden_size as u32,
        })
    }

    fn select_device() -> EmbeddingResult<Device> {
        // Try Metal (macOS) first
        if let Ok(device) = Device::new_metal(0) {
            return Ok(device);
        }

        // Try CUDA (Linux) second
        if let Ok(device) = Device::new_cuda(0) {
            return Ok(device);
        }

        // Fallback to CPU
        Ok(Device::Cpu)
    }
}
```

**Deliverables:**
- `CandleEmbeddingProvider::new()` implementation
- Model download from HF Hub
- Device selection logic

**Success Criteria:**
- ✅ Downloads MiniLM model from Hugging Face
- ✅ Loads model into Metal (M1) or CUDA (Linux)
- ✅ Model size ~90MB (safetensors)
- ✅ Loading time <2 seconds

**Estimated Time:** 6 hours

---

### Day 3: Tokenization & Inference

**Tasks:**
1. ☐ Implement tokenization logic
2. ☐ Implement forward pass (single text)
3. ☐ Implement mean pooling
4. ☐ Implement batch inference
5. ☐ Add basic error handling

**Code:**
```rust
impl CandleEmbeddingProvider {
    /// Generate embeddings for batch of texts
    async fn embed_batch_internal(
        &self,
        texts: Vec<String>
    ) -> EmbeddingResult<Vec<Vec<f32>>> {
        // 1. Tokenize texts
        let encodings = self.tokenizer
            .encode_batch(texts, true)
            .map_err(|e| EmbeddingError::Tokenization(e.to_string()))?;

        let input_ids: Vec<Vec<u32>> = encodings
            .iter()
            .map(|e| e.get_ids().to_vec())
            .collect();

        // 2. Convert to tensor
        let input_ids = Tensor::new(input_ids, &self.device)
            .map_err(|e| EmbeddingError::Internal(e.to_string()))?;

        // 3. Run inference in blocking thread pool (GPU work)
        let embeddings = tokio::task::spawn_blocking({
            let model = Arc::clone(&self.model);
            let input_ids = input_ids.clone();
            move || -> EmbeddingResult<Vec<Vec<f32>>> {
                // Forward pass
                let outputs = model.forward(&input_ids)
                    .map_err(|e| EmbeddingError::Internal(e.to_string()))?;

                // Mean pooling
                let embeddings = outputs
                    .mean(1)  // Average across sequence dimension
                    .map_err(|e| EmbeddingError::Internal(e.to_string()))?
                    .to_vec2()
                    .map_err(|e| EmbeddingError::Internal(e.to_string()))?;

                Ok(embeddings)
            }
        })
        .await
        .map_err(|e| EmbeddingError::Internal(e.to_string()))??;

        Ok(embeddings)
    }
}
```

**Deliverables:**
- Tokenization implementation
- Inference pipeline (forward + pooling)
- Batch processing

**Success Criteria:**
- ✅ Generates embeddings for single text
- ✅ Generates embeddings for batch (1-32 texts)
- ✅ Output dimension matches config (384 for MiniLM)
- ✅ Latency <20ms per text (M1 Pro)

**Estimated Time:** 6 hours

---

### Day 4: Unit Tests

**Tasks:**
1. ☐ Write tests for model loading
2. ☐ Write tests for tokenization
3. ☐ Write tests for inference (single + batch)
4. ☐ Write tests for error handling
5. ☐ Write tests for device selection

**Test Suite:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_model_loading() {
        let provider = CandleEmbeddingProvider::new(
            "sentence-transformers/all-MiniLM-L6-v2"
        ).await;

        assert!(provider.is_ok());
        let provider = provider.unwrap();
        assert_eq!(provider.dimension, 384);
    }

    #[tokio::test]
    async fn test_single_text_embedding() {
        let provider = CandleEmbeddingProvider::new(
            "sentence-transformers/all-MiniLM-L6-v2"
        ).await.unwrap();

        let embeddings = provider.embed_batch_internal(
            vec!["Hello world".to_string()]
        ).await;

        assert!(embeddings.is_ok());
        let embeddings = embeddings.unwrap();
        assert_eq!(embeddings.len(), 1);
        assert_eq!(embeddings[0].len(), 384);
    }

    #[tokio::test]
    async fn test_batch_embedding() {
        let provider = CandleEmbeddingProvider::new(
            "sentence-transformers/all-MiniLM-L6-v2"
        ).await.unwrap();

        let texts = vec![
            "Machine learning".to_string(),
            "Deep learning".to_string(),
            "Neural networks".to_string(),
        ];

        let embeddings = provider.embed_batch_internal(texts).await;

        assert!(embeddings.is_ok());
        let embeddings = embeddings.unwrap();
        assert_eq!(embeddings.len(), 3);
        assert_eq!(embeddings[0].len(), 384);
    }

    #[tokio::test]
    async fn test_empty_input() {
        let provider = CandleEmbeddingProvider::new(
            "sentence-transformers/all-MiniLM-L6-v2"
        ).await.unwrap();

        let result = provider.embed_batch_internal(vec![]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_very_long_text() {
        let provider = CandleEmbeddingProvider::new(
            "sentence-transformers/all-MiniLM-L6-v2"
        ).await.unwrap();

        let long_text = "word ".repeat(1000);  // 5000+ chars
        let result = provider.embed_batch_internal(
            vec![long_text]
        ).await;

        assert!(result.is_ok());  // Should truncate, not fail
    }

    // TODO: Add 10 more tests (device selection, error cases, etc.)
}
```

**Deliverables:**
- 15+ unit tests
- Test coverage >80%
- All tests passing

**Success Criteria:**
- ✅ All 15 tests pass
- ✅ Tests run in <10 seconds total
- ✅ CI pipeline runs tests automatically

**Estimated Time:** 6 hours

---

### Day 5: EmbeddingProvider Trait Implementation

**Tasks:**
1. ☐ Implement `EmbeddingProvider` trait for `CandleEmbeddingProvider`
2. ☐ Implement `embed_batch()` method
3. ☐ Implement `model_info()` method
4. ☐ Implement `health_check()` method
5. ☐ Add to `EmbeddingManager` as optional provider

**Code:**
```rust
#[async_trait]
impl EmbeddingProvider for CandleEmbeddingProvider {
    async fn embed_batch(
        &self,
        request: BatchEmbeddingRequest,
    ) -> EmbeddingResult<BatchEmbeddingResponse> {
        // Validate input
        if request.inputs.is_empty() {
            return Err(EmbeddingError::InvalidInput(
                "Empty input".to_string()
            ));
        }

        // Generate embeddings
        let embeddings = self.embed_batch_internal(request.inputs.clone()).await?;

        // Build response
        Ok(BatchEmbeddingResponse {
            embeddings,
            model: self.model_name.clone(),
            usage: Usage {
                prompt_tokens: request.inputs.len(),
                total_tokens: request.inputs.len(),
            },
        })
    }

    async fn model_info(&self) -> EmbeddingResult<ModelInfo> {
        Ok(ModelInfo {
            name: self.model_name.clone(),
            dimension: self.dimension,
            max_tokens: 512,  // BERT max sequence length
        })
    }

    async fn health_check(&self) -> EmbeddingResult<()> {
        // Try to generate a test embedding
        let test_result = self.embed_batch_internal(
            vec!["health check".to_string()]
        ).await;

        match test_result {
            Ok(_) => Ok(()),
            Err(e) => Err(EmbeddingError::ServiceUnavailable(
                format!("Health check failed: {}", e)
            )),
        }
    }
}
```

**Deliverables:**
- Full `EmbeddingProvider` trait implementation
- Integration with `EmbeddingManager`
- Configuration support

**Success Criteria:**
- ✅ Implements all trait methods
- ✅ Backward compatible with existing API
- ✅ Can be swapped with MLX provider

**Estimated Time:** 6 hours

---

### Week 1 Summary

**Total Deliverables:**
- ✅ `candle.rs` (~400 lines)
- ✅ 15+ unit tests
- ✅ Feature flag (`candle`)
- ✅ EmbeddingProvider implementation
- ✅ CI pipeline updated

**Success Criteria:**
- ✅ All tests pass (15/15)
- ✅ Latency <20ms per text
- ✅ Works on macOS ARM + Linux
- ✅ No breaking changes

**Estimated Total Time:** 28 hours (1 engineer-week)

---

## Phase 2: Performance Optimization (Week 2, Days 6-10)

**Goal:** Multi-threaded inference, <50ms P95 latency @ 200 QPS

### Day 6: Thread Pool Implementation

**Tasks:**
1. ☐ Design thread pool architecture
2. ☐ Implement inference worker pool
3. ☐ Add request queuing (bounded queue)
4. ☐ Add load balancing (round-robin)
5. ☐ Add backpressure handling

**Architecture:**
```rust
pub struct InferenceThreadPool {
    workers: Vec<InferenceWorker>,
    request_tx: mpsc::Sender<InferenceRequest>,
    config: PoolConfig,
}

struct InferenceWorker {
    id: usize,
    model: Arc<BertModel>,
    device: Device,
    request_rx: mpsc::Receiver<InferenceRequest>,
}

pub struct PoolConfig {
    num_workers: usize,      // 8 for M1 Pro
    queue_size: usize,       // 1000 pending requests
    timeout: Duration,       // 30s max wait
}
```

**Deliverables:**
- Thread pool implementation (~200 lines)
- Worker management
- Backpressure handling

**Success Criteria:**
- ✅ 8 concurrent workers
- ✅ Queue depth monitoring
- ✅ Graceful degradation under load

**Estimated Time:** 8 hours

---

### Day 7: Batch Processing Optimization

**Tasks:**
1. ☐ Implement dynamic batching
2. ☐ Add batch timeout (collect for 10ms max)
3. ☐ Optimize padding strategy
4. ☐ Add batch size tuning
5. ☐ Benchmark batch sizes (1, 8, 16, 32)

**Code:**
```rust
pub struct DynamicBatcher {
    pending: Vec<(String, oneshot::Sender<Vec<f32>>)>,
    batch_size: usize,     // 32 max
    timeout: Duration,     // 10ms
    last_flush: Instant,
}

impl DynamicBatcher {
    async fn add_request(&mut self, text: String) -> Vec<f32> {
        let (tx, rx) = oneshot::channel();
        self.pending.push((text, tx));

        // Flush if batch full or timeout
        if self.pending.len() >= self.batch_size
            || self.last_flush.elapsed() > self.timeout
        {
            self.flush().await;
        }

        rx.await
    }

    async fn flush(&mut self) {
        let batch = std::mem::take(&mut self.pending);
        let texts: Vec<String> = batch.iter()
            .map(|(t, _)| t.clone())
            .collect();

        let embeddings = self.model.forward_batch(texts).await;

        for ((_, tx), emb) in batch.into_iter().zip(embeddings) {
            let _ = tx.send(emb);
        }

        self.last_flush = Instant::now();
    }
}
```

**Deliverables:**
- Dynamic batcher (~150 lines)
- Batch size optimization
- Benchmark results

**Success Criteria:**
- ✅ Batch size 32 achieves best throughput
- ✅ Latency penalty <10ms for batching
- ✅ Throughput increases 3-4x

**Estimated Time:** 8 hours

---

### Day 8: Model Caching & Warmup

**Tasks:**
1. ☐ Implement model cache
2. ☐ Add cache eviction (LRU)
3. ☐ Add model preloading on startup
4. ☐ Add warmup requests (prime GPU)
5. ☐ Add cache hit/miss metrics

**Code:**
```rust
pub struct ModelCache {
    cache_dir: PathBuf,
    models: Arc<RwLock<HashMap<String, Arc<BertModel>>>>,
    max_models: usize,  // 3 models max
}

impl ModelCache {
    pub async fn get_or_load(
        &self,
        model_name: &str
    ) -> EmbeddingResult<Arc<BertModel>> {
        // Check cache first
        {
            let models = self.models.read();
            if let Some(model) = models.get(model_name) {
                return Ok(Arc::clone(model));
            }
        }

        // Cache miss - load model
        let model = self.load_model(model_name).await?;

        // Update cache
        {
            let mut models = self.models.write();
            models.insert(model_name.to_string(), Arc::clone(&model));

            // Evict if over limit
            if models.len() > self.max_models {
                self.evict_lru(&mut models);
            }
        }

        Ok(model)
    }

    pub async fn preload(&self, models: &[&str]) -> EmbeddingResult<()> {
        for model_name in models {
            self.get_or_load(model_name).await?;

            // Warmup with dummy inference
            self.warmup(model_name).await?;
        }
        Ok(())
    }
}
```

**Deliverables:**
- Model cache (~150 lines)
- Preload + warmup logic
- Cache metrics

**Success Criteria:**
- ✅ First request: ~2s (cold start)
- ✅ Subsequent requests: <20ms (cache hit)
- ✅ Cache holds 3 models max

**Estimated Time:** 6 hours

---

### Day 9: GPU Memory Optimization

**Tasks:**
1. ☐ Profile GPU memory usage
2. ☐ Optimize tensor allocations
3. ☐ Add gradient clipping (if training)
4. ☐ Add OOM handling
5. ☐ Add memory metrics

**Deliverables:**
- Memory profiling report
- Optimization patches
- OOM recovery

**Success Criteria:**
- ✅ Memory usage <500MB
- ✅ No memory leaks
- ✅ Graceful OOM handling

**Estimated Time:** 6 hours

---

### Day 10: Benchmarking Suite

**Tasks:**
1. ☐ Create wrk load test script
2. ☐ Create Criterion benchmarks
3. ☐ Run sequential benchmarks
4. ☐ Run concurrent benchmarks
5. ☐ Create performance report

**Benchmarks:**
```bash
# Sequential (single connection)
wrk -t 1 -c 1 -d 30s -s scripts/wrk-candle-embed.lua \
    http://localhost:8080/api/v1/embed

Target: 28 QPS, P95 <40ms

# Concurrent (100 connections)
wrk -t 8 -c 100 -d 30s -s scripts/wrk-candle-embed.lua \
    http://localhost:8080/api/v1/embed

Target: 200 QPS, P95 <50ms, >99% success rate
```

**Deliverables:**
- Load test scripts
- Criterion benchmarks
- Performance report

**Success Criteria:**
- ✅ Sequential: 28 QPS @ <40ms P95
- ✅ Concurrent: 200 QPS @ <50ms P95
- ✅ Success rate: >99.9%

**Estimated Time:** 6 hours

---

### Week 2 Summary

**Total Deliverables:**
- ✅ Thread pool (~200 lines)
- ✅ Dynamic batcher (~150 lines)
- ✅ Model cache (~150 lines)
- ✅ Performance benchmarks
- ✅ Performance report

**Success Criteria:**
- ✅ 200 QPS @ <50ms P95
- ✅ 99.9% success rate
- ✅ Memory <500MB

**Estimated Total Time:** 34 hours (1.7 engineer-weeks)

---

## Phase 3: Production Hardening (Week 3, Days 11-15)

### Day 11: Error Handling & Retries

**Tasks:**
1. ☐ Add retry logic (exponential backoff)
2. ☐ Add circuit breaker
3. ☐ Add timeout handling
4. ☐ Add error classification
5. ☐ Add error metrics

**Deliverables:**
- Error handling (~100 lines)
- Retry logic
- Circuit breaker

**Success Criteria:**
- ✅ Retries 3 times before failing
- ✅ Circuit opens after 5 consecutive failures
- ✅ Timeout after 30s

**Estimated Time:** 6 hours

---

### Day 12: Prometheus Metrics

**Tasks:**
1. ☐ Add request counter
2. ☐ Add latency histogram
3. ☐ Add error counter
4. ☐ Add cache hit/miss counter
5. ☐ Add GPU memory gauge

**Metrics:**
```rust
// Counters
candle_requests_total{model="minilm"} - Counter
candle_errors_total{model="minilm",error_type="..."} - Counter
candle_cache_hits_total - Counter
candle_cache_misses_total - Counter

// Histograms
candle_request_duration_seconds{model="minilm"} - Histogram
candle_batch_size{model="minilm"} - Histogram

// Gauges
candle_model_memory_bytes{model="minilm"} - Gauge
candle_queue_depth - Gauge
candle_active_workers - Gauge
```

**Deliverables:**
- 10 metrics
- Prometheus scrape endpoint

**Success Criteria:**
- ✅ All metrics exposed via `/metrics`
- ✅ Metrics scrape <5ms

**Estimated Time:** 4 hours

---

### Day 13: Health Checks & Readiness Probes

**Tasks:**
1. ☐ Implement liveness probe
2. ☐ Implement readiness probe
3. ☐ Add model health check
4. ☐ Add GPU health check
5. ☐ Add integration tests

**Deliverables:**
- Health check endpoints
- Readiness probe logic

**Success Criteria:**
- ✅ `/health` returns 200 if alive
- ✅ `/ready` returns 200 if ready
- ✅ Health check <10ms

**Estimated Time:** 4 hours

---

### Day 14: Configuration Validation

**Tasks:**
1. ☐ Add config schema
2. ☐ Add validation rules
3. ☐ Add config loading tests
4. ☐ Add environment variable support
5. ☐ Add config examples

**Deliverables:**
- Config validation
- Example configs
- Documentation

**Success Criteria:**
- ✅ Invalid config fails fast
- ✅ Environment overrides work
- ✅ Examples in docs

**Estimated Time:** 4 hours

---

### Day 15: Integration Tests

**Tasks:**
1. ☐ REST API integration tests (10 tests)
2. ☐ gRPC API integration tests (10 tests)
3. ☐ End-to-end tests (5 tests)
4. ☐ Stress tests (3 scenarios)
5. ☐ CI integration

**Test Scenarios:**
```rust
#[tokio::test]
async fn test_rest_api_embed() {
    // Start server with Candle provider
    let server = start_test_server().await;

    // Send embed request
    let response = reqwest::post("http://localhost:8080/api/v1/embed")
        .json(&json!({
            "texts": ["Hello world"]
        }))
        .send()
        .await;

    assert_eq!(response.status(), 200);
    let body: BatchEmbeddingResponse = response.json().await;
    assert_eq!(body.embeddings.len(), 1);
    assert_eq!(body.embeddings[0].len(), 384);
}

// TODO: 24 more integration tests
```

**Deliverables:**
- 25 integration tests
- CI pipeline integration

**Success Criteria:**
- ✅ All 25 tests pass
- ✅ Tests run in <60 seconds
- ✅ CI runs tests automatically

**Estimated Time:** 8 hours

---

### Week 3 Summary

**Total Deliverables:**
- ✅ Error handling (~100 lines)
- ✅ 10 Prometheus metrics
- ✅ Health checks
- ✅ Config validation
- ✅ 25 integration tests

**Success Criteria:**
- ✅ All tests pass
- ✅ Metrics exposed
- ✅ Production-ready code quality

**Estimated Total Time:** 26 hours (1.3 engineer-weeks)

---

## Phase 4-6: Detailed Plans

Due to length constraints, here are abbreviated plans for remaining phases:

### Phase 4: Multi-Model Support (Week 4, Days 16-20)

**Goal:** Support MiniLM, BGE, Qwen2

**Key Tasks:**
- Generic model loader
- BGE model support
- Qwen2 model support
- Model comparison benchmarks

**Deliverables:** 3 model implementations, comparison report

---

### Phase 5: Docker & Kubernetes (Week 5, Days 21-25)

**Goal:** Cloud-native deployment

**Key Tasks:**
- Dockerfile with GPU support
- Kubernetes manifests
- Helm chart
- Deployment guide

**Deliverables:** Docker image (<100MB), K8s manifests, Helm chart

---

### Phase 6: Migration & Deprecation (Week 6, Days 26-30)

**Goal:** MLX → Candle migration, deprecate MLX

**Key Tasks:**
- Migration guide
- Backward compatibility testing
- Performance comparison
- Deprecation notices

**Deliverables:** Migration guide, comparison report, release notes

---

## Success Metrics Summary

**Week 1:** Basic provider working
**Week 2:** 200 QPS @ <50ms P95
**Week 3:** Production-ready quality
**Week 4:** 3 models supported
**Week 5:** Docker/K8s deployment
**Week 6:** Migration complete

---

## Risk Mitigation

**High Risks:**
1. **Candle API changes** → Pin version, monitor releases
2. **Performance regression** → Comprehensive benchmarks before merge
3. **Embedding quality drift** → Similarity validation (>95%)

**Medium Risks:**
1. **GPU driver issues** → CPU fallback, extensive testing
2. **Model compatibility** → Test before adding to supported list
3. **Breaking API changes** → Feature flag, dual-provider support

---

## Team & Timeline

**Optimal Team:**
- 2 Rust engineers (full-time)
- 1 QA engineer (full-time)
- 1 DevOps engineer (50% time)

**Timeline:** 6 weeks (30 working days)

**Critical Path:** Phase 1 → Phase 2 → Phase 3 (Weeks 1-3 must complete before cloud deployment)

---

**Document Status:** READY TO EXECUTE
**Next Action:** Get approval from tech lead, assign engineers, start Phase 1
