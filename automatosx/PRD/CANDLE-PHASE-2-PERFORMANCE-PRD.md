# Phase 2: Performance Optimization PRD
## Candle Embedding Migration - Week 2

**Version:** 1.0
**Date:** 2025-11-10
**Status:** Ready for Implementation
**Owner:** Backend Team
**Timeline:** 5 days (Week 2, Monday-Friday)

---

## Executive Summary

**Goal:** Optimize Candle embedding provider to achieve production performance targets: **200+ QPS throughput** and **P95 <35ms latency** with concurrent request handling.

**Phase 2 Context:** Building on Phase 1's foundation (working Candle provider), this phase focuses on **performance optimization** to unlock the full potential of Rust-native ML inference. We'll implement dynamic batching, model caching, GPU optimization, and comprehensive benchmarking to achieve 36x throughput improvement over MLX.

**Success Criteria:**
- ✅ Throughput: 200+ QPS (vs 5.5 QPS MLX baseline)
- ✅ Latency: P95 <35ms @ 100 QPS (vs 182ms MLX baseline)
- ✅ Concurrent success rate: >99% (vs 0.02% MLX baseline)
- ✅ Memory efficiency: <500MB model footprint
- ✅ GPU utilization: >70% on Metal/CUDA

---

## Table of Contents

1. [Problem Statement](#problem-statement)
2. [Goals & Non-Goals](#goals--non-goals)
3. [Technical Design](#technical-design)
4. [Performance Optimizations](#performance-optimizations)
5. [Dynamic Batching Strategy](#dynamic-batching-strategy)
6. [GPU Optimization](#gpu-optimization)
7. [Benchmarking Framework](#benchmarking-framework)
8. [Testing Strategy](#testing-strategy)
9. [Success Criteria](#success-criteria)
10. [Risks & Mitigation](#risks--mitigation)
11. [Timeline & Milestones](#timeline--milestones)
12. [Dependencies](#dependencies)
13. [Deliverables](#deliverables)

---

## Problem Statement

### Current State (Post Phase 1)

Phase 1 delivered a **working Candle provider** with:
- ✅ Basic BERT model inference (MiniLM-L6-v2)
- ✅ Single-threaded tokenization and embedding generation
- ✅ P50 ~13ms latency for single requests
- ✅ 15 unit tests passing
- ✅ EmbeddingProvider trait implementation

**However**, the Phase 1 implementation is **not optimized for production workloads**:

| Metric | Phase 1 (Current) | Production Target | Gap |
|--------|-------------------|-------------------|-----|
| **Throughput** | ~50 QPS | 200+ QPS | **4x improvement needed** |
| **Latency (P95)** | ~80ms | <35ms | **2.3x improvement needed** |
| **Concurrent handling** | Sequential | 100+ concurrent | **Batching required** |
| **GPU utilization** | ~30% | >70% | **Optimization needed** |
| **Memory per request** | ~100MB | <10MB | **Caching required** |

### Why Performance Matters

**Business Impact:**
- **Cost Efficiency:** Higher QPS → fewer server instances → lower cloud costs
- **User Experience:** Lower latency → faster search results → better satisfaction
- **Scalability:** Handle 10,000+ daily users with 2-3 server instances
- **Competitive Advantage:** Match or exceed Pinecone/Weaviate embedding performance

**Technical Impact:**
- **Unlock Candle potential:** Phase 1 baseline is naive, Candle can do 10x better
- **Validate migration ROI:** Prove 36x improvement claim from PRD
- **Enable production deployment:** Meet SLA requirements for RC2 release

---

## Goals & Non-Goals

### Goals (In Scope)

**Primary Goals:**
1. ✅ **Dynamic Batching:** Aggregate concurrent requests into batches (2-32 texts)
2. ✅ **Model Caching:** Eliminate redundant model loads (memory-efficient singleton)
3. ✅ **GPU Optimization:** Maximize Metal/CUDA utilization (>70%)
4. ✅ **Comprehensive Benchmarks:** Criterion benchmarks for latency/throughput
5. ✅ **Load Testing:** wrk-based load tests validating 200 QPS target

**Secondary Goals:**
6. ✅ **Thread Pool Tuning:** Optimize Tokio spawn_blocking for inference
7. ✅ **Memory Profiling:** Measure and optimize memory footprint
8. ✅ **Concurrency Testing:** Validate 100+ concurrent requests
9. ✅ **Performance Documentation:** Benchmark results and tuning guide

### Non-Goals (Out of Scope)

**Deferred to Later Phases:**
- ❌ Multi-model support (Phase 4)
- ❌ Quantization (INT8/INT4) (Phase 4)
- ❌ Model warm-up strategies (Phase 3)
- ❌ Distributed inference (Future)
- ❌ Custom BERT architectures (Future)
- ❌ Production deployment (Phase 5)
- ❌ Monitoring/observability (Phase 3)

**Explicitly Out of Scope:**
- ❌ MLX performance improvements (deprecated path)
- ❌ API changes (maintain EmbeddingProvider trait)
- ❌ Breaking changes to Phase 1 implementation

---

## Technical Design

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    REST/gRPC API Layer                       │
│                   (Concurrent Requests)                      │
└────────────────────────┬────────────────────────────────────┘
                         │ 100+ concurrent requests
                         ▼
┌─────────────────────────────────────────────────────────────┐
│              Dynamic Batching Layer (NEW)                    │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ BatchAggregator                                       │  │
│  │ - Collect requests over 10ms window                   │  │
│  │ - Aggregate 2-32 texts per batch                      │  │
│  │ - Distribute results back to callers                  │  │
│  └──────────────────────────────────────────────────────┘  │
└────────────────────────┬────────────────────────────────────┘
                         │ Batched requests
                         ▼
┌─────────────────────────────────────────────────────────────┐
│         CandleEmbeddingProvider (Optimized)                  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ ModelCache (NEW)                                      │  │
│  │ - Singleton Arc<BertModel>                            │  │
│  │ - Lazy initialization                                 │  │
│  │ - Thread-safe access                                  │  │
│  └──────────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ Optimized Inference Pipeline                          │  │
│  │ 1. Parallel tokenization (rayon)                      │  │
│  │ 2. GPU-optimized forward pass                         │  │
│  │ 3. Efficient mean pooling                             │  │
│  │ 4. Zero-copy tensor conversion                        │  │
│  └──────────────────────────────────────────────────────┘  │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│              GPU Backend (Metal/CUDA)                        │
│  - Batch inference (2-32 texts)                              │
│  - Kernel fusion optimization                                │
│  - >70% GPU utilization                                      │
└─────────────────────────────────────────────────────────────┘
```

### Component Design

#### 1. Dynamic Batching Layer

**Purpose:** Aggregate concurrent requests to maximize GPU efficiency

```rust
/// Dynamic batching coordinator
pub struct BatchAggregator {
    /// Pending requests waiting for batch
    pending: Arc<Mutex<Vec<PendingRequest>>>,
    /// Batch configuration
    config: BatchConfig,
    /// Background worker handle
    worker: Option<JoinHandle<()>>,
}

pub struct BatchConfig {
    /// Maximum batch size (default: 32)
    pub max_batch_size: usize,
    /// Maximum wait time before flushing (default: 10ms)
    pub max_wait_ms: u64,
    /// Minimum batch size to process (default: 2)
    pub min_batch_size: usize,
}

struct PendingRequest {
    texts: Vec<String>,
    response_tx: oneshot::Sender<EmbeddingResult<Vec<Vec<f32>>>>,
    submitted_at: Instant,
}

impl BatchAggregator {
    /// Submit request for batching
    pub async fn submit(
        &self,
        texts: Vec<String>,
    ) -> EmbeddingResult<Vec<Vec<f32>>> {
        let (tx, rx) = oneshot::channel();

        {
            let mut pending = self.pending.lock().await;
            pending.push(PendingRequest {
                texts,
                response_tx: tx,
                submitted_at: Instant::now(),
            });
        }

        // Wait for response (with timeout)
        tokio::time::timeout(
            Duration::from_millis(100),
            rx
        ).await??
    }

    /// Background worker that flushes batches
    async fn worker_loop(&self, provider: Arc<CandleEmbeddingProvider>) {
        loop {
            tokio::time::sleep(Duration::from_millis(self.config.max_wait_ms)).await;
            self.flush_batch(&provider).await;
        }
    }

    /// Flush pending requests as a batch
    async fn flush_batch(&self, provider: &CandleEmbeddingProvider) {
        let batch = {
            let mut pending = self.pending.lock().await;
            if pending.len() < self.config.min_batch_size {
                return; // Wait for more requests
            }
            pending.drain(..self.config.max_batch_size.min(pending.len())).collect()
        };

        // Aggregate all texts
        let all_texts: Vec<String> = batch.iter()
            .flat_map(|req| req.texts.clone())
            .collect();

        // Single batch inference
        match provider.embed_batch_internal(all_texts).await {
            Ok(embeddings) => {
                // Distribute results back to callers
                self.distribute_results(batch, embeddings);
            }
            Err(e) => {
                // Notify all callers of error
                for req in batch {
                    let _ = req.response_tx.send(Err(e.clone()));
                }
            }
        }
    }
}
```

**Key Design Decisions:**
- **10ms batching window:** Balance latency vs throughput
- **2-32 batch size:** Optimal for GPU utilization without memory pressure
- **Timeout handling:** 100ms max wait to prevent deadlocks
- **Error propagation:** All requests in batch fail together (simplicity)

#### 2. Model Cache (Memory Optimization)

**Purpose:** Share single model instance across all requests

```rust
/// Thread-safe model cache singleton
pub struct ModelCache {
    models: Arc<RwLock<HashMap<String, Arc<ModelState>>>>,
    cache_dir: PathBuf,
}

struct ModelState {
    model: Arc<BertModel>,
    tokenizer: Arc<Tokenizer>,
    device: Device,
    dimension: u32,
    loaded_at: Instant,
}

impl ModelCache {
    /// Get or load model (lazy initialization)
    pub async fn get_or_load(
        &self,
        model_name: &str,
    ) -> EmbeddingResult<Arc<ModelState>> {
        // Fast path: read lock
        {
            let models = self.models.read().await;
            if let Some(state) = models.get(model_name) {
                return Ok(Arc::clone(state));
            }
        }

        // Slow path: write lock + load
        let mut models = self.models.write().await;

        // Double-check (another thread may have loaded)
        if let Some(state) = models.get(model_name) {
            return Ok(Arc::clone(state));
        }

        // Load model
        let state = Arc::new(Self::load_model_internal(model_name).await?);
        models.insert(model_name.to_string(), Arc::clone(&state));

        Ok(state)
    }

    /// Preload models on startup
    pub async fn preload(&self, model_names: &[&str]) -> EmbeddingResult<()> {
        for name in model_names {
            self.get_or_load(name).await?;
        }
        Ok(())
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let models = self.models.read().await;
        CacheStats {
            loaded_models: models.len(),
            memory_mb: self.estimate_memory(&models),
        }
    }
}
```

**Memory Savings:**
- Before: 90MB per request × 100 concurrent = **9,000 MB**
- After: 90MB shared singleton = **90 MB**
- **Savings: 99% reduction in memory usage**

#### 3. Optimized Inference Pipeline

**Purpose:** Maximize GPU utilization and minimize CPU overhead

```rust
impl CandleEmbeddingProvider {
    /// Optimized batch embedding with GPU acceleration
    async fn embed_batch_internal(
        &self,
        texts: Vec<String>,
    ) -> EmbeddingResult<Vec<Vec<f32>>> {
        let start = Instant::now();

        // Step 1: Parallel tokenization (CPU-bound, use rayon)
        let input_ids = tokio::task::spawn_blocking({
            let tokenizer = Arc::clone(&self.tokenizer);
            let texts = texts.clone();
            move || Self::tokenize_batch_parallel(&tokenizer, &texts)
        }).await??;

        tracing::debug!(
            "Tokenized {} texts in {:?}",
            texts.len(),
            start.elapsed()
        );

        // Step 2: GPU inference (GPU-bound, minimize CPU overhead)
        let embeddings = tokio::task::spawn_blocking({
            let model = Arc::clone(&self.model);
            let device = self.device.clone();
            move || {
                // Transfer to GPU
                let input_ids_gpu = input_ids.to_device(&device)?;

                // Forward pass (GPU-accelerated)
                let outputs = model.forward(&input_ids_gpu)?;

                // Mean pooling (GPU-accelerated)
                let embeddings = outputs.mean(1)?;

                // L2 normalization (GPU-accelerated)
                let norms = embeddings.sqr()?.sum_keepdim(1)?.sqrt()?;
                let normalized = embeddings.broadcast_div(&norms)?;

                // Transfer back to CPU
                normalized.to_vec2()
            }
        }).await??;

        tracing::debug!(
            "Generated {} embeddings in {:?}",
            embeddings.len(),
            start.elapsed()
        );

        Ok(embeddings)
    }

    /// Parallel tokenization using rayon
    fn tokenize_batch_parallel(
        tokenizer: &Tokenizer,
        texts: &[String],
    ) -> EmbeddingResult<Tensor> {
        use rayon::prelude::*;

        // Parallel tokenization
        let encodings: Vec<_> = texts
            .par_iter()
            .map(|text| {
                tokenizer
                    .encode(text.as_str(), true)
                    .map_err(|e| EmbeddingError::TokenizationFailed(e.to_string()))
            })
            .collect::<EmbeddingResult<Vec<_>>>()?;

        // Pad to same length
        let max_len = encodings.iter()
            .map(|e| e.len())
            .max()
            .unwrap_or(0);

        let mut padded = Vec::new();
        for enc in encodings {
            let mut ids = enc.get_ids().to_vec();
            ids.resize(max_len, 0); // Pad with 0
            padded.push(ids);
        }

        // Convert to tensor
        Tensor::new(padded, &Device::Cpu)
            .map_err(|e| EmbeddingError::InferenceFailed(e.to_string()))
    }
}
```

**Optimizations:**
1. **Parallel tokenization:** Use rayon for CPU-parallelism (4x faster)
2. **GPU kernel fusion:** Mean pooling + L2 norm in single GPU pass
3. **Minimize CPU↔GPU transfers:** One transfer in, one transfer out
4. **Zero-copy tensor ops:** Use Candle's in-place operations

---

## Performance Optimizations

### Optimization 1: Dynamic Batching

**Problem:** Single requests underutilize GPU (30% utilization)

**Solution:** Aggregate 2-32 requests into batches

**Impact:**
- GPU utilization: 30% → 75%
- Throughput: 50 QPS → 180 QPS (3.6x)
- Latency: +10ms p95 (acceptable trade-off)

**Implementation:**
```rust
// Before (Phase 1): Sequential processing
for request in requests {
    let embedding = provider.embed_single(request).await?;
}

// After (Phase 2): Dynamic batching
let embeddings = batch_aggregator
    .submit_many(requests)
    .await?;
```

### Optimization 2: Model Caching

**Problem:** Loading model per request wastes 50ms + 90MB

**Solution:** Singleton Arc<BertModel> shared across threads

**Impact:**
- First request: 50ms load time
- Subsequent requests: 0ms load time
- Memory: 9,000MB → 90MB (100x reduction)

### Optimization 3: Parallel Tokenization

**Problem:** Sequential tokenization is CPU bottleneck

**Solution:** Use rayon for parallel tokenization

**Impact:**
- Tokenization: 5ms → 1.2ms (4x faster)
- CPU utilization: 25% → 80% (better utilization)

### Optimization 4: GPU Kernel Fusion

**Problem:** Multiple GPU operations have transfer overhead

**Solution:** Fuse mean pooling + L2 norm into single pass

**Impact:**
- GPU operations: 3 passes → 1 pass
- Latency: 15ms → 10ms (1.5x faster)

---

## Dynamic Batching Strategy

### Batching Configuration

```rust
pub struct BatchConfig {
    /// Maximum batch size (tuned for GPU memory)
    pub max_batch_size: usize,        // Default: 32

    /// Maximum wait time before flushing
    pub max_wait_ms: u64,             // Default: 10ms

    /// Minimum batch size to process
    pub min_batch_size: usize,        // Default: 2

    /// Batch timeout (prevent deadlocks)
    pub batch_timeout_ms: u64,        // Default: 100ms
}
```

### Batching Algorithm

```
1. Request arrives → Add to pending queue
2. Start timer (10ms)
3. IF queue size >= max_batch_size OR timer expires:
   a. Drain up to max_batch_size requests
   b. Aggregate all texts into single batch
   c. Run single embed_batch_internal()
   d. Distribute results to oneshot channels
4. Repeat
```

### Batching Trade-offs

| Metric | Small Batches (2-8) | Medium Batches (8-16) | Large Batches (16-32) |
|--------|---------------------|------------------------|------------------------|
| **Latency** | +5ms p95 | +10ms p95 | +15ms p95 |
| **Throughput** | 100 QPS | 180 QPS | 220 QPS |
| **GPU Util** | 50% | 75% | 85% |
| **Memory** | Low | Medium | High |
| **Recommendation** | Low traffic | **Production (best balance)** | High traffic |

**Default: 8-16 batch size (medium) for production**

---

## GPU Optimization

### Device Selection Strategy

```rust
impl CandleEmbeddingProvider {
    /// Select best available device
    fn select_device() -> EmbeddingResult<Device> {
        // Priority: Metal > CUDA > CPU

        #[cfg(target_os = "macos")]
        if Device::metal_if_available(0).is_ok() {
            tracing::info!("Using Metal GPU (Apple Silicon)");
            return Ok(Device::new_metal(0)?);
        }

        #[cfg(feature = "cuda")]
        if Device::cuda_if_available(0).is_ok() {
            tracing::info!("Using CUDA GPU (NVIDIA)");
            return Ok(Device::new_cuda(0)?);
        }

        tracing::warn!("No GPU available, falling back to CPU");
        Ok(Device::Cpu)
    }
}
```

### GPU Optimization Techniques

**1. Batch Size Tuning:**
- Small batches (2-8): Lower latency, lower GPU utilization
- Large batches (16-32): Higher throughput, higher GPU utilization
- **Recommendation:** 8-16 for production (balance)

**2. Kernel Fusion:**
```rust
// Before: Multiple GPU operations
let mean = outputs.mean(1)?;           // GPU op 1
let squared = mean.sqr()?;             // GPU op 2
let sum = squared.sum_keepdim(1)?;     // GPU op 3
let norm = sum.sqrt()?;                // GPU op 4
let normalized = mean.broadcast_div(&norm)?; // GPU op 5

// After: Fused operations
let normalized = outputs
    .mean(1)?
    .normalize(1)?;  // Single fused kernel
```

**3. Memory Layout Optimization:**
```rust
// Contiguous memory for efficient GPU transfer
let tensor = Tensor::new(data, &device)?
    .contiguous()?;  // Ensure contiguous layout
```

**4. GPU Warm-up:**
```rust
impl CandleEmbeddingProvider {
    /// Warm up GPU kernels on initialization
    async fn warmup(&self) -> EmbeddingResult<()> {
        // Run dummy inference to compile GPU kernels
        let _ = self.embed_batch_internal(
            vec!["warmup".to_string()]
        ).await?;

        Ok(())
    }
}
```

---

## Benchmarking Framework

### Criterion Benchmarks

**File:** `crates/akidb-embedding/benches/candle_bench.rs`

```rust
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use akidb_embedding::candle::CandleEmbeddingProvider;

fn bench_latency(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let provider = runtime.block_on(async {
        CandleEmbeddingProvider::new("sentence-transformers/all-MiniLM-L6-v2")
            .await
            .unwrap()
    });

    let mut group = c.benchmark_group("candle_latency");

    // Single text
    group.bench_function("single_text", |b| {
        b.to_async(&runtime).iter(|| async {
            provider.embed_batch_internal(vec![
                "The quick brown fox jumps over the lazy dog".to_string()
            ]).await.unwrap()
        })
    });

    // Batch sizes: 2, 4, 8, 16, 32
    for batch_size in [2, 4, 8, 16, 32] {
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            &batch_size,
            |b, &size| {
                let texts = vec![
                    "Sample text for embedding generation".to_string();
                    size
                ];
                b.to_async(&runtime).iter(|| async {
                    provider.embed_batch_internal(texts.clone()).await.unwrap()
                })
            },
        );
    }

    group.finish();
}

fn bench_throughput(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let provider = Arc::new(runtime.block_on(async {
        CandleEmbeddingProvider::new("sentence-transformers/all-MiniLM-L6-v2")
            .await
            .unwrap()
    }));

    let mut group = c.benchmark_group("candle_throughput");
    group.sample_size(50);

    // Concurrent requests: 10, 50, 100, 200
    for concurrency in [10, 50, 100, 200] {
        group.throughput(Throughput::Elements(concurrency as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(concurrency),
            &concurrency,
            |b, &conc| {
                b.to_async(&runtime).iter(|| async {
                    let tasks: Vec<_> = (0..conc)
                        .map(|_| {
                            let provider = Arc::clone(&provider);
                            tokio::spawn(async move {
                                provider.embed_batch_internal(vec![
                                    "Benchmark text".to_string()
                                ]).await.unwrap()
                            })
                        })
                        .collect();

                    for task in tasks {
                        task.await.unwrap();
                    }
                })
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_latency, bench_throughput);
criterion_main!(benches);
```

### Load Testing with wrk

**File:** `scripts/wrk-candle-load-test.lua`

```lua
-- Load test for Candle embedding endpoint

wrk.method = "POST"
wrk.headers["Content-Type"] = "application/json"

-- Test data
local texts = {
    "The quick brown fox jumps over the lazy dog",
    "Machine learning is transforming software development",
    "Vector databases enable semantic search at scale",
    "Rust provides memory safety without garbage collection",
}

-- Generate request body
function request()
    local text = texts[math.random(#texts)]
    wrk.body = string.format('{"texts": ["%s"]}', text)
    return wrk.format()
end

-- Track response times
local latencies = {}
function response(status, headers, body)
    if status == 200 then
        table.insert(latencies, wrk.latency)
    end
end

-- Print statistics
function done(summary, latency, requests)
    io.write("------------------------------\n")
    io.write(string.format("Requests:      %d\n", summary.requests))
    io.write(string.format("Duration:      %.2fs\n", summary.duration / 1000000))
    io.write(string.format("QPS:           %.2f\n", summary.requests / (summary.duration / 1000000)))
    io.write(string.format("Latency (avg): %.2fms\n", latency.mean / 1000))
    io.write(string.format("Latency (p50): %.2fms\n", latency:percentile(50) / 1000))
    io.write(string.format("Latency (p95): %.2fms\n", latency:percentile(95) / 1000))
    io.write(string.format("Latency (p99): %.2fms\n", latency:percentile(99) / 1000))
    io.write("------------------------------\n")
end
```

**Run Load Tests:**
```bash
# Start server
cargo run -p akidb-rest --features candle &

# Wait for startup
sleep 5

# Test 1: Low load (10 connections, 20s)
wrk -t 2 -c 10 -d 20s -s scripts/wrk-candle-load-test.lua \
    http://localhost:8080/api/v1/embed

# Test 2: Medium load (50 connections, 30s)
wrk -t 4 -c 50 -d 30s -s scripts/wrk-candle-load-test.lua \
    http://localhost:8080/api/v1/embed

# Test 3: High load (100 connections, 30s)
wrk -t 8 -c 100 -d 30s -s scripts/wrk-candle-load-test.lua \
    http://localhost:8080/api/v1/embed

# Test 4: Stress test (200 connections, 60s)
wrk -t 8 -c 200 -d 60s -s scripts/wrk-candle-load-test.lua \
    http://localhost:8080/api/v1/embed
```

### Expected Benchmark Results

| Test | Metric | Target | Baseline (Phase 1) |
|------|--------|--------|--------------------|
| **Latency** | Single text | <15ms | ~13ms ✅ |
| | Batch 8 | <25ms | N/A |
| | Batch 32 | <50ms | N/A |
| **Throughput** | 10 concurrent | 80+ QPS | ~45 QPS |
| | 50 concurrent | 180+ QPS | N/A |
| | 100 concurrent | 200+ QPS | N/A |
| **Load Test** | P50 latency | <20ms | N/A |
| | P95 latency | <35ms | N/A |
| | P99 latency | <50ms | N/A |
| | Error rate | <0.1% | 0% (sequential) |

---

## Testing Strategy

### Test Categories

**1. Performance Regression Tests (5 tests)**
```rust
#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_latency_regression() {
        let provider = CandleEmbeddingProvider::new(
            "sentence-transformers/all-MiniLM-L6-v2"
        ).await.unwrap();

        let start = Instant::now();
        let _ = provider.embed_batch_internal(vec![
            "Test text".to_string()
        ]).await.unwrap();
        let elapsed = start.elapsed();

        assert!(
            elapsed < Duration::from_millis(20),
            "Latency regression: {:?} > 20ms",
            elapsed
        );
    }

    #[tokio::test]
    async fn test_batch_efficiency() {
        let provider = Arc::new(CandleEmbeddingProvider::new(
            "sentence-transformers/all-MiniLM-L6-v2"
        ).await.unwrap());

        // Measure single request
        let start_single = Instant::now();
        let _ = provider.embed_batch_internal(vec![
            "Text 1".to_string()
        ]).await.unwrap();
        let single_time = start_single.elapsed();

        // Measure batch of 8
        let start_batch = Instant::now();
        let _ = provider.embed_batch_internal(vec![
            "Text 1".to_string(),
            "Text 2".to_string(),
            "Text 3".to_string(),
            "Text 4".to_string(),
            "Text 5".to_string(),
            "Text 6".to_string(),
            "Text 7".to_string(),
            "Text 8".to_string(),
        ]).await.unwrap();
        let batch_time = start_batch.elapsed();

        // Batch should be <3x single request (not 8x)
        assert!(
            batch_time < single_time * 3,
            "Batch inefficient: {:?} >= 3 * {:?}",
            batch_time,
            single_time
        );
    }

    #[tokio::test]
    async fn test_concurrent_throughput() {
        let provider = Arc::new(CandleEmbeddingProvider::new(
            "sentence-transformers/all-MiniLM-L6-v2"
        ).await.unwrap());

        let start = Instant::now();

        // 50 concurrent requests
        let tasks: Vec<_> = (0..50)
            .map(|i| {
                let provider = Arc::clone(&provider);
                tokio::spawn(async move {
                    provider.embed_batch_internal(vec![
                        format!("Test text {}", i)
                    ]).await.unwrap()
                })
            })
            .collect();

        for task in tasks {
            task.await.unwrap();
        }

        let elapsed = start.elapsed();
        let qps = 50.0 / elapsed.as_secs_f64();

        assert!(
            qps >= 80.0,
            "Throughput regression: {:.2} QPS < 80 QPS",
            qps
        );
    }

    #[tokio::test]
    async fn test_memory_efficiency() {
        use sysinfo::{System, SystemExt};

        let mut sys = System::new_all();
        sys.refresh_memory();
        let mem_before = sys.used_memory();

        // Create 10 providers (should share model)
        let providers: Vec<_> = (0..10)
            .map(|_| {
                CandleEmbeddingProvider::new(
                    "sentence-transformers/all-MiniLM-L6-v2"
                )
            })
            .collect::<Vec<_>>();

        futures::future::join_all(providers).await;

        sys.refresh_memory();
        let mem_after = sys.used_memory();
        let mem_increase_mb = (mem_after - mem_before) / 1024;

        // Should use <200MB (not 900MB for 10 instances)
        assert!(
            mem_increase_mb < 200,
            "Memory leak: {} MB > 200 MB",
            mem_increase_mb
        );
    }

    #[tokio::test]
    async fn test_gpu_utilization() {
        // Note: Requires GPU monitoring tools
        // This is a manual test placeholder

        let provider = Arc::new(CandleEmbeddingProvider::new(
            "sentence-transformers/all-MiniLM-L6-v2"
        ).await.unwrap());

        // Run sustained load for 10 seconds
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(10) {
            let tasks: Vec<_> = (0..16)
                .map(|_| {
                    let provider = Arc::clone(&provider);
                    tokio::spawn(async move {
                        provider.embed_batch_internal(vec![
                            "GPU utilization test".to_string()
                        ]).await.unwrap()
                    })
                })
                .collect();

            for task in tasks {
                task.await.unwrap();
            }
        }

        // Manual verification:
        // macOS: sudo powermetrics --samplers gpu_power
        // Linux: nvidia-smi dmon
        // Expected: >70% GPU utilization
    }
}
```

**2. Dynamic Batching Tests (8 tests)**
```rust
#[cfg(test)]
mod batching_tests {
    use super::*;

    #[tokio::test]
    async fn test_batch_aggregation() {
        let provider = Arc::new(CandleEmbeddingProvider::new(
            "sentence-transformers/all-MiniLM-L6-v2"
        ).await.unwrap());

        let aggregator = BatchAggregator::new(
            BatchConfig {
                max_batch_size: 8,
                max_wait_ms: 10,
                min_batch_size: 2,
                batch_timeout_ms: 100,
            },
            provider,
        );

        // Submit 5 requests concurrently
        let tasks: Vec<_> = (0..5)
            .map(|i| {
                let agg = aggregator.clone();
                tokio::spawn(async move {
                    agg.submit(vec![format!("Text {}", i)]).await
                })
            })
            .collect();

        // All should succeed
        for task in tasks {
            let result = task.await.unwrap();
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_batch_timeout() {
        let provider = Arc::new(CandleEmbeddingProvider::new(
            "sentence-transformers/all-MiniLM-L6-v2"
        ).await.unwrap());

        let aggregator = BatchAggregator::new(
            BatchConfig {
                max_batch_size: 100,  // Large batch size
                max_wait_ms: 20,      // 20ms timeout
                min_batch_size: 2,
                batch_timeout_ms: 100,
            },
            provider,
        );

        let start = Instant::now();

        // Submit single request (won't reach min_batch_size)
        let result = aggregator.submit(vec!["Test".to_string()]).await;

        let elapsed = start.elapsed();

        // Should timeout after ~20ms (not wait forever)
        assert!(
            elapsed < Duration::from_millis(50),
            "Batch timeout not working: {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn test_batch_size_limit() {
        let provider = Arc::new(CandleEmbeddingProvider::new(
            "sentence-transformers/all-MiniLM-L6-v2"
        ).await.unwrap());

        let aggregator = BatchAggregator::new(
            BatchConfig {
                max_batch_size: 4,    // Small batch size
                max_wait_ms: 5,
                min_batch_size: 2,
                batch_timeout_ms: 100,
            },
            provider,
        );

        // Submit 10 requests (should split into batches of 4)
        let tasks: Vec<_> = (0..10)
            .map(|i| {
                let agg = aggregator.clone();
                tokio::spawn(async move {
                    agg.submit(vec![format!("Text {}", i)]).await
                })
            })
            .collect();

        // All should succeed despite batching
        for task in tasks {
            let result = task.await.unwrap();
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_batch_error_propagation() {
        // Test that errors in batched requests propagate correctly
        // (Implementation depends on error handling strategy)
    }

    // TODO: Add 4 more batching tests
    // - test_batch_fairness (FIFO ordering)
    // - test_batch_distribution (result distribution)
    // - test_batch_concurrent_flush
    // - test_batch_shutdown
}
```

**3. Model Cache Tests (5 tests)**
```rust
#[cfg(test)]
mod cache_tests {
    use super::*;

    #[tokio::test]
    async fn test_model_cache_hit() {
        let cache = ModelCache::new("/tmp/akidb-test-cache");

        // First load (cache miss)
        let start_miss = Instant::now();
        let model1 = cache.get_or_load(
            "sentence-transformers/all-MiniLM-L6-v2"
        ).await.unwrap();
        let miss_time = start_miss.elapsed();

        // Second load (cache hit)
        let start_hit = Instant::now();
        let model2 = cache.get_or_load(
            "sentence-transformers/all-MiniLM-L6-v2"
        ).await.unwrap();
        let hit_time = start_hit.elapsed();

        // Cache hit should be >10x faster
        assert!(
            hit_time < miss_time / 10,
            "Cache not working: {:?} >= {:?} / 10",
            hit_time,
            miss_time
        );

        // Should be same Arc pointer
        assert!(Arc::ptr_eq(&model1.model, &model2.model));
    }

    #[tokio::test]
    async fn test_model_preload() {
        let cache = ModelCache::new("/tmp/akidb-test-cache");

        // Preload model
        cache.preload(&["sentence-transformers/all-MiniLM-L6-v2"])
            .await
            .unwrap();

        // Access should be instant
        let start = Instant::now();
        let _ = cache.get_or_load(
            "sentence-transformers/all-MiniLM-L6-v2"
        ).await.unwrap();
        let elapsed = start.elapsed();

        assert!(
            elapsed < Duration::from_millis(10),
            "Preload not working: {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn test_concurrent_cache_load() {
        let cache = Arc::new(ModelCache::new("/tmp/akidb-test-cache"));

        // Multiple threads try to load same model
        let tasks: Vec<_> = (0..10)
            .map(|_| {
                let cache = Arc::clone(&cache);
                tokio::spawn(async move {
                    cache.get_or_load(
                        "sentence-transformers/all-MiniLM-L6-v2"
                    ).await.unwrap()
                })
            })
            .collect();

        let models: Vec<_> = futures::future::join_all(tasks)
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        // All should share same model instance
        for i in 1..models.len() {
            assert!(Arc::ptr_eq(&models[0].model, &models[i].model));
        }
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache = ModelCache::new("/tmp/akidb-test-cache");

        cache.preload(&[
            "sentence-transformers/all-MiniLM-L6-v2",
        ]).await.unwrap();

        let stats = cache.stats().await;

        assert_eq!(stats.loaded_models, 1);
        assert!(stats.memory_mb > 80 && stats.memory_mb < 120);
    }

    // TODO: Add 1 more cache test
    // - test_cache_eviction (if implementing LRU)
}
```

**4. GPU Optimization Tests (3 tests)**
```rust
#[cfg(test)]
mod gpu_tests {
    use super::*;

    #[tokio::test]
    async fn test_device_selection() {
        let device = CandleEmbeddingProvider::select_device().unwrap();

        #[cfg(target_os = "macos")]
        assert!(matches!(device, Device::Metal(_)));

        #[cfg(all(feature = "cuda", not(target_os = "macos")))]
        assert!(matches!(device, Device::Cuda(_)));
    }

    #[tokio::test]
    async fn test_gpu_memory_management() {
        let provider = CandleEmbeddingProvider::new(
            "sentence-transformers/all-MiniLM-L6-v2"
        ).await.unwrap();

        // Process 100 batches (should not leak GPU memory)
        for _ in 0..100 {
            let _ = provider.embed_batch_internal(vec![
                "Test".to_string(); 8
            ]).await.unwrap();
        }

        // Manual verification: GPU memory should be stable
    }

    #[tokio::test]
    async fn test_cpu_fallback() {
        // Force CPU device
        let provider = CandleEmbeddingProvider::new_with_device(
            "sentence-transformers/all-MiniLM-L6-v2",
            Device::Cpu,
        ).await.unwrap();

        let result = provider.embed_batch_internal(vec![
            "CPU fallback test".to_string()
        ]).await;

        assert!(result.is_ok());
    }
}
```

### Test Execution Plan

**Day 4 Testing (6 hours):**
1. **Hour 1-2:** Implement 5 performance regression tests
2. **Hour 3-4:** Implement 8 dynamic batching tests
3. **Hour 5:** Implement 5 model cache tests
4. **Hour 6:** Implement 3 GPU optimization tests

**Total:** 21 new tests (Phase 1: 15 tests → Phase 2: 36 tests)

---

## Success Criteria

### Functional Requirements

✅ **FR1:** Dynamic batching aggregates 2-32 requests with <10ms latency overhead
✅ **FR2:** Model cache shares singleton across all requests
✅ **FR3:** GPU utilization >70% under sustained load
✅ **FR4:** Parallel tokenization using rayon
✅ **FR5:** Zero-copy tensor operations where possible
✅ **FR6:** Comprehensive Criterion benchmarks
✅ **FR7:** wrk load tests validating 200 QPS

### Non-Functional Requirements

✅ **NFR1: Performance**
- Throughput: 200+ QPS @ 100 concurrent requests
- Latency: P95 <35ms, P99 <50ms
- GPU utilization: >70%

✅ **NFR2: Reliability**
- Error rate: <0.1% under load
- No memory leaks over 1 hour sustained load
- Graceful degradation under overload

✅ **NFR3: Efficiency**
- Memory footprint: <500MB total (not per request)
- CPU utilization: 60-80% (not bottleneck)
- GPU utilization: 70-90% (maximize)

✅ **NFR4: Quality**
- All Phase 1 tests still passing (no regression)
- 21 new tests passing
- Code coverage >80%

### Performance Targets Summary

| Metric | Phase 1 Baseline | Phase 2 Target | Improvement |
|--------|------------------|----------------|-------------|
| **Throughput** | ~50 QPS | 200+ QPS | **4x** |
| **P50 Latency** | ~13ms | <20ms | Maintained |
| **P95 Latency** | ~80ms | <35ms | **2.3x faster** |
| **P99 Latency** | ~150ms | <50ms | **3x faster** |
| **GPU Utilization** | ~30% | >70% | **2.3x higher** |
| **Memory** | ~9,000MB | <500MB | **18x less** |
| **Concurrent Success** | Sequential | >99% @ 100 concurrent | Unlimited |

---

## Risks & Mitigation

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| **Dynamic batching adds >20ms latency** | Medium | High | • Tune batch window to 5-10ms<br>• Add adaptive batching (adjust window based on load)<br>• Benchmark different configs |
| **Model cache race conditions** | Low | High | • Use RwLock for thread-safety<br>• Double-check locking pattern<br>• Comprehensive concurrency tests |
| **GPU memory leaks** | Medium | High | • Profile with Instruments/nvidia-smi<br>• Add memory tests<br>• Ensure tensors are dropped |
| **Performance targets not met** | Medium | High | • Start optimizing Day 1 (not Day 4)<br>• Daily benchmark checks<br>• Fallback: extend Phase 2 by 2 days |
| **Candle API instability** | Low | Medium | • Pin to 0.8.x<br>• Test upgrades in isolation<br>• Keep Phase 1 code as fallback |
| **Benchmark flakiness** | High | Low | • Use statistical tests (multiple runs)<br>• Increase sample size<br>• Run on dedicated hardware |

---

## Timeline & Milestones

### Week 2 Schedule (5 days, Monday-Friday)

#### **Day 1 (Monday): Dynamic Batching (6 hours)**

**Tasks:**
1. ☐ Implement `BatchAggregator` struct (2 hours)
2. ☐ Implement batching logic (flush, distribution) (2 hours)
3. ☐ Add background worker (1 hour)
4. ☐ Unit tests for batching (1 hour)

**Deliverables:**
- `src/batching.rs` (~250 lines)
- 8 batching tests passing
- Benchmark: batching adds <15ms p95

**Success Criteria:**
- ✅ Batching aggregates 2-32 requests
- ✅ Timeout handling works (<100ms)
- ✅ All tests passing

#### **Day 2 (Tuesday): Model Caching (6 hours)**

**Tasks:**
1. ☐ Implement `ModelCache` struct (2 hours)
2. ☐ Add lazy loading + thread-safety (2 hours)
3. ☐ Implement preload + stats (1 hour)
4. ☐ Cache tests (1 hour)

**Deliverables:**
- `src/model_cache.rs` (~200 lines)
- 5 cache tests passing
- Memory test: 10 instances use <200MB

**Success Criteria:**
- ✅ Cache hit is >10x faster than miss
- ✅ Concurrent loads are safe
- ✅ Preload works correctly

#### **Day 3 (Wednesday): GPU Optimization (6 hours)**

**Tasks:**
1. ☐ Implement parallel tokenization with rayon (2 hours)
2. ☐ Optimize GPU operations (kernel fusion) (2 hours)
3. ☐ Add GPU warm-up (1 hour)
4. ☐ GPU tests + profiling (1 hour)

**Deliverables:**
- Updated `src/candle.rs` (~100 lines changed)
- 3 GPU tests passing
- Profiling: GPU utilization >70%

**Success Criteria:**
- ✅ Tokenization is 4x faster (parallel)
- ✅ GPU utilization >70%
- ✅ No GPU memory leaks

#### **Day 4 (Thursday): Testing & Benchmarking (6 hours)**

**Tasks:**
1. ☐ Implement 5 performance regression tests (2 hours)
2. ☐ Add Criterion benchmarks (2 hours)
3. ☐ Run comprehensive benchmark suite (1 hour)
4. ☐ Document results (1 hour)

**Deliverables:**
- `benches/candle_bench.rs` (~300 lines)
- 21 total tests passing (15 Phase 1 + 21 Phase 2 - 15 = 21 new)
- Benchmark report with charts

**Success Criteria:**
- ✅ All tests passing
- ✅ Benchmarks show 4x throughput improvement
- ✅ No performance regressions

#### **Day 5 (Friday): Load Testing & Documentation (6 hours)**

**Tasks:**
1. ☐ Create wrk load test script (1 hour)
2. ☐ Run load tests (4 scenarios) (2 hours)
3. ☐ Analyze results + fixes (2 hours)
4. ☐ Write performance documentation (1 hour)

**Deliverables:**
- `scripts/wrk-candle-load-test.lua` (~100 lines)
- Load test results (4 scenarios)
- `docs/CANDLE-PERFORMANCE.md` documentation
- Phase 2 completion report

**Success Criteria:**
- ✅ 200+ QPS @ 100 concurrent connections
- ✅ P95 <35ms latency
- ✅ Error rate <0.1%
- ✅ All documentation complete

### Phase 2 Milestones

- **M1 (Day 1 EOD):** Dynamic batching implemented and tested
- **M2 (Day 2 EOD):** Model cache implemented and tested
- **M3 (Day 3 EOD):** GPU optimization complete
- **M4 (Day 4 EOD):** All tests passing + benchmarks complete
- **M5 (Day 5 EOD):** Load tests passing + Phase 2 COMPLETE 🎉

---

## Dependencies

### Internal Dependencies

**From Phase 1:**
- ✅ CandleEmbeddingProvider (working implementation)
- ✅ EmbeddingProvider trait (interface)
- ✅ Basic BERT model inference
- ✅ Device selection logic
- ✅ 15 unit tests passing

**Blockers:**
- ❌ None (Phase 1 complete)

### External Dependencies

**Rust Crates (Already in Cargo.toml):**
```toml
[dependencies]
candle-core = { version = "0.8", features = ["metal"] }
candle-nn = "0.8"
candle-transformers = "0.8"
tokenizers = "0.15"
hf-hub = "0.3"
tokio = { version = "1.0", features = ["full"] }
rayon = "1.8"  # NEW for parallel tokenization

[dev-dependencies]
criterion = { version = "0.5", features = ["async_tokio", "html_reports"] }
```

**System Tools:**
```bash
# Benchmarking
cargo install cargo-criterion

# Load testing
brew install wrk  # macOS
apt install wrk   # Linux

# GPU monitoring
# macOS: sudo powermetrics --samplers gpu_power
# Linux: nvidia-smi dmon
```

---

## Deliverables

### Code Deliverables

| File | Lines | Description |
|------|-------|-------------|
| `src/batching.rs` | ~250 | Dynamic batching implementation |
| `src/model_cache.rs` | ~200 | Model cache singleton |
| `src/candle.rs` (updated) | +100 | GPU optimizations |
| `benches/candle_bench.rs` | ~300 | Criterion benchmarks |
| `tests/performance_tests.rs` | ~200 | Performance regression tests |
| `tests/batching_tests.rs` | ~150 | Batching tests |
| `tests/cache_tests.rs` | ~100 | Cache tests |
| `tests/gpu_tests.rs` | ~80 | GPU tests |
| `scripts/wrk-candle-load-test.lua` | ~100 | Load test script |
| **Total** | **~1,480 lines** | |

### Documentation Deliverables

1. **`docs/CANDLE-PERFORMANCE.md`** - Performance tuning guide
   - Benchmarking methodology
   - Load test results
   - Performance troubleshooting
   - Tuning recommendations

2. **Phase 2 Completion Report** - `automatosx/tmp/PHASE-2-COMPLETION-REPORT.md`
   - Summary of accomplishments
   - Performance metrics achieved
   - Lessons learned
   - Next steps (Phase 3)

### Test Deliverables

- **21 new tests** (Phase 1: 15 tests → Phase 2: 36 tests total)
- **Benchmark suite** with latency + throughput tests
- **Load test suite** with 4 scenarios
- **Performance profiling** results

### Performance Deliverables

**Validated Targets:**
- ✅ Throughput: 200+ QPS @ 100 concurrent
- ✅ Latency: P95 <35ms
- ✅ GPU Utilization: >70%
- ✅ Memory: <500MB total
- ✅ Error rate: <0.1%

---

## Appendix

### A. Performance Tuning Knobs

```rust
// Batching configuration
pub struct BatchConfig {
    pub max_batch_size: usize,     // 8-32 (default: 16)
    pub max_wait_ms: u64,           // 5-20ms (default: 10ms)
    pub min_batch_size: usize,      // 1-4 (default: 2)
    pub batch_timeout_ms: u64,      // 50-200ms (default: 100ms)
}

// Thread pool configuration
tokio::runtime::Builder::new_multi_thread()
    .worker_threads(8)              // Match CPU cores
    .max_blocking_threads(16)       // 2x worker threads
    .build()

// Rayon configuration
rayon::ThreadPoolBuilder::new()
    .num_threads(4)                 // Half CPU cores (for tokenization)
    .build_global()
```

### B. Troubleshooting Guide

**Problem: Throughput <150 QPS**
- Check: GPU utilization (should be >70%)
- Fix: Increase batch size (try 16-24)
- Fix: Reduce batch wait time (try 5ms)

**Problem: P95 latency >50ms**
- Check: Batch wait time (should be <10ms)
- Fix: Decrease batch size (try 8-12)
- Fix: Decrease batch wait time (try 5ms)

**Problem: GPU utilization <50%**
- Check: Batch size (should be >8)
- Fix: Increase concurrent requests
- Fix: Reduce CPU bottlenecks (tokenization)

**Problem: Memory leak**
- Check: Model cache is singleton
- Fix: Ensure tensors are dropped after use
- Fix: Profile with `cargo-instruments` (macOS)

### C. Comparison with MLX (Phase 0)

| Aspect | MLX (Python) | Candle (Rust Phase 2) | Improvement |
|--------|--------------|------------------------|-------------|
| **Throughput** | 5.5 QPS | 200+ QPS | **36x** |
| **Latency (P95)** | 182ms | <35ms | **5.2x** |
| **Concurrency** | 0.02% success | >99% success | **4950x** |
| **Memory** | 90MB per request | 90MB shared | **100x (at 100 concurrent)** |
| **GPU Utilization** | ~25% | >70% | **2.8x** |
| **Deployment** | macOS only | Docker/K8s/Native | Universal |

---

## Sign-Off

**Phase 2 PRD Version:** 1.0
**Status:** ✅ Ready for Implementation
**Estimated Effort:** 30 development hours (5 days × 6 hours)
**Expected Completion:** End of Week 2

**Next Phase:** [Phase 3: Production Hardening](CANDLE-PHASE-3-PRODUCTION-PRD.md)

---

**Document End**
