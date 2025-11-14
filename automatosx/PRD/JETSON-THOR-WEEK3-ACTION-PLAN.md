# Jetson Thor Week 3: Performance Optimization - 5-Day Action Plan

**Status:** Ready to Execute
**Timeline:** 5 days
**Team:** 1 backend engineer
**Goal:** Optimize from 80ms P95 → <30ms P95, 15 QPS → >50 QPS

---

## Day-by-Day Summary

| Day | Focus | Key Tasks | Expected Outcome | Time |
|-----|-------|-----------|------------------|------|
| **Mon** | TensorRT Profiles | Enable FP8, create 3 profiles, benchmark | 40ms → ~25ms | 7h |
| **Tue** | TensorRT Tuning | Tactics, workspace, Nsight profiling | 25ms → ~20ms | 7h |
| **Wed** | Memory Optimization | Pinned memory, CUDA streams, buffer pooling | 20ms → ~18ms | 7h |
| **Thu** | Dynamic Batching | Queue, processor, metrics | 15 QPS → >50 QPS | 7h |
| **Fri** | Testing & Documentation | E2E tests, completion report, tuning guide | Validation | 7h |

**Total:** 35 hours (1 engineer × 5 days)

---

## Day 1: TensorRT Profile Optimization (Monday)

### Morning (3 hours)

**Task 1.1: Enable FP8 Tensor Cores** [2 hours]
```bash
cd ~/akidb2

# 1. Patch ONNX provider to force FP8
cat > /tmp/enable_fp8.patch << 'EOF'
--- a/crates/akidb-embedding/src/onnx.rs
+++ b/crates/akidb-embedding/src/onnx.rs
@@ -120,6 +120,9 @@ impl OnnxEmbeddingProvider {
             trt_options.device_id = *device_id;
+            trt_options.fp16_enable = false;
+            trt_options.int8_enable = false;
             trt_options.fp8_enable = *fp8_enable;
+            trt_options.max_workspace_size = 2_000_000_000;
             trt_options.builder_optimization_level = 5;
EOF

patch -p1 < /tmp/enable_fp8.patch

# 2. Rebuild with FP8
cargo build --release -p akidb-embedding --features onnx

# 3. Delete old TensorRT cache (force rebuild)
rm -rf /tmp/akidb_trt_cache/*

# 4. Test FP8 usage (will rebuild TensorRT engine, ~5 min)
RUST_LOG=debug cargo test -p akidb-embedding --features onnx --release \
  --test qwen3_integration_test test_single_embedding -- --nocapture 2>&1 | \
  tee /tmp/fp8_test.log

# 5. Verify FP8 kernels used (not FP16 fallback)
grep -i "fp8\|precision" /tmp/fp8_test.log
```

**Success Metric:** Latency reduces from 80ms → ~50-60ms

**Task 1.2: Morning Checkpoint** [1 hour]
```bash
# Quick benchmark to confirm FP8 working
cargo test -p akidb-embedding --features onnx --release \
  --test qwen3_integration_test test_single_embedding -- --nocapture | \
  grep "Duration:"

# Expected: ~50-60ms (down from 80ms)
```

### Afternoon (4 hours)

**Task 1.3: Generate TensorRT Optimization Profiles** [3 hours]
```bash
cd /opt/akidb

# Create profile generation script
cat > scripts/generate_trt_profiles.py << 'SCRIPT'
#!/usr/bin/env python3
import onnxruntime as ort
from pathlib import Path

profiles = [
    {"name": "low_latency", "min_batch": 1, "opt_batch": 1, "max_batch": 2},
    {"name": "balanced", "min_batch": 2, "opt_batch": 4, "max_batch": 8},
    {"name": "high_throughput", "min_batch": 8, "opt_batch": 16, "max_batch": 32}
]

model_path = "/opt/akidb/models/qwen3-4b-onnx-fp8/model.onnx"

for profile in profiles:
    print(f"\nBuilding profile: {profile['name']} (takes ~5 min)...")
    cache_path = f"/opt/akidb/trt_cache/{profile['name']}"
    Path(cache_path).mkdir(parents=True, exist_ok=True)

    trt_options = {
        'device_id': 0,
        'trt_fp8_enable': True,
        'trt_max_workspace_size': 2_000_000_000,
        'trt_engine_cache_enable': True,
        'trt_engine_cache_path': cache_path,
        'trt_builder_optimization_level': 5,
        'trt_profile_min_shapes': f'input_ids:{profile["min_batch"]}x512',
        'trt_profile_opt_shapes': f'input_ids:{profile["opt_batch"]}x512',
        'trt_profile_max_shapes': f'input_ids:{profile["max_batch"]}x512',
    }

    sess = ort.InferenceSession(
        model_path,
        providers=[('TensorrtExecutionProvider', trt_options)]
    )

    print(f"✅ Profile created: {cache_path}")
SCRIPT

chmod +x scripts/generate_trt_profiles.py

# Generate all 3 profiles (~15 min total)
python3 scripts/generate_trt_profiles.py 2>&1 | tee /tmp/profile_generation.log
```

**Success Metric:** 3 TensorRT profiles created successfully

**Task 1.4: Benchmark Profiles** [1 hour]
```bash
# Benchmark each profile
cat > /tmp/benchmark_profiles.sh << 'SCRIPT'
#!/bin/bash
for profile in low_latency balanced high_throughput; do
  echo "Benchmarking $profile..."
  TENSORRT_PROFILE=$profile cargo bench --bench qwen3_bench -- \
    --warm-up-time 1 --measurement-time 10 batch_sizes/1 2>&1 | \
    grep "time:" | tee -a /tmp/profile_benchmarks.txt
done
SCRIPT

chmod +x /tmp/benchmark_profiles.sh
./benchmark_profiles.sh

# Analyze results
echo "Profile comparison:"
cat /tmp/profile_benchmarks.txt
```

**Success Metric:** `low_latency` profile achieves ~25-30ms P95

### End of Day 1

**Deliverables:**
- ✅ FP8 Tensor Cores enabled (verified in logs)
- ✅ 3 TensorRT profiles created
- ✅ Best profile identified: `low_latency`
- ✅ Latency improved: 80ms → ~25-30ms

**Expected Latency:** ~25-30ms P95 (2.7-3.2x improvement)

---

## Day 2: TensorRT Tactics & Workspace Tuning (Tuesday)

### Morning (4 hours)

**Task 2.1: Tactic Source Selection** [2 hours]
```bash
cd ~/akidb2

# Update onnx.rs to accept tactic sources as config
cat > /tmp/tactics.patch << 'EOF'
pub struct TensorRTConfig {
    pub tactic_sources: String,  // NEW
}

impl Default for TensorRTConfig {
    fn default() -> Self {
        Self {
            tactic_sources: "CUBLAS,CUDNN,EDGE_MASK_CONVOLUTIONS".to_string(),
        }
    }
}
EOF

# Test 3 tactic combinations
for tactics in "CUBLAS" "CUBLAS,CUDNN" "CUBLAS,CUDNN,EDGE_MASK_CONVOLUTIONS"; do
  echo "Testing tactics: $tactics"
  rm -rf /tmp/tactics_test_cache
  TACTIC_SOURCES="$tactics" cargo bench --bench qwen3_bench -- \
    --measurement-time 15 batch_sizes/1 2>&1 | grep "time:" | \
    tee -a /tmp/tactics_results.txt
done

# Compare results
echo "Tactics comparison:"
cat /tmp/tactics_results.txt
```

**Success Metric:** Identify fastest tactic combination (~5-10% improvement)

**Task 2.2: Workspace Size Tuning** [2 hours]
```bash
# Test workspace sizes: 512MB, 1GB, 2GB, 4GB
cat > /tmp/test_workspace.py << 'SCRIPT'
#!/usr/bin/env python3
import onnxruntime as ort
import numpy as np
import time

model_path = "/opt/akidb/models/qwen3-4b-onnx-fp8/model.onnx"

for workspace_mb in [512, 1024, 2048, 4096]:
    print(f"\nTesting workspace: {workspace_mb}MB")
    cache_path = f"/tmp/workspace_{workspace_mb}mb"

    trt_options = {
        'device_id': 0,
        'trt_fp8_enable': True,
        'trt_max_workspace_size': workspace_mb * 1024 * 1024,
        'trt_engine_cache_enable': True,
        'trt_engine_cache_path': cache_path,
        'trt_builder_optimization_level': 5,
    }

    sess = ort.InferenceSession(model_path, providers=[('TensorrtExecutionProvider', trt_options)])

    # Benchmark
    input_ids = np.random.randint(0, 151936, size=(1, 512), dtype=np.int64)
    attention_mask = np.ones((1, 512), dtype=np.int64)

    # Warmup
    for _ in range(10):
        sess.run(None, {'input_ids': input_ids, 'attention_mask': attention_mask})

    # Measure
    latencies = []
    for _ in range(100):
        start = time.perf_counter()
        sess.run(None, {'input_ids': input_ids, 'attention_mask': attention_mask})
        latencies.append((time.perf_counter() - start) * 1000)

    p95 = np.percentile(latencies, 95)
    print(f"  P95: {p95:.2f}ms")
SCRIPT

python3 /tmp/test_workspace.py 2>&1 | tee /tmp/workspace_results.txt
```

**Success Metric:** Identify optimal workspace size (likely 2GB)

### Afternoon (3 hours)

**Task 2.3: Nsight Systems Deep Profiling** [3 hours]
```bash
# Profile optimized ONNX inference
nsys profile \
  --trace=cuda,nvtx,osrt \
  --cuda-memory-usage=true \
  --output=/tmp/qwen3_day2.nsys-rep \
  --force-overwrite=true \
  --stats=true \
  cargo test -p akidb-embedding --features onnx --release \
    --test qwen3_integration_test test_single_embedding -- --nocapture

# Analyze in GUI
nsys-ui /tmp/qwen3_day2.nsys-rep &

# Export key statistics
nsys stats /tmp/qwen3_day2.nsys-rep --report cuda_gpu_kern_sum > /tmp/kernel_stats.txt
nsys stats /tmp/qwen3_day2.nsys-rep --report cuda_api_sum > /tmp/api_stats.txt

# Check for bottlenecks
echo "Top 10 GPU kernels:"
head -20 /tmp/kernel_stats.txt

echo "Memory transfer overhead:"
grep "memcpy" /tmp/api_stats.txt
```

**Success Metric:**
- GPU utilization >80%
- FP8 GEMM kernels dominate execution time
- Memory copy <5% of total time

### End of Day 2

**Deliverables:**
- ✅ Optimal tactic sources identified
- ✅ Optimal workspace size: 2GB
- ✅ Nsight Systems profile completed
- ✅ Bottlenecks documented

**Expected Latency:** ~20-25ms P95 (minor improvement from tactics/workspace)

---

## Day 3: Memory Optimization (Wednesday)

### Morning (4 hours)

**Task 3.1: Pinned Memory Implementation** [2 hours]
```rust
// Add to crates/akidb-embedding/src/memory.rs

use std::alloc::Layout;

pub struct PinnedBuffer<T> {
    ptr: *mut T,
    len: usize,
}

impl<T> PinnedBuffer<T> {
    pub fn new(capacity: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let layout = Layout::array::<T>(capacity)?;
        let mut ptr: *mut T = std::ptr::null_mut();

        unsafe {
            let status = cuda_sys::cudaMallocHost(
                &mut ptr as *mut *mut T as *mut *mut std::ffi::c_void,
                layout.size()
            );

            if status != cuda_sys::cudaError_t::cudaSuccess {
                return Err(format!("cudaMallocHost failed: {:?}", status).into());
            }
        }

        Ok(Self { ptr, len: capacity })
    }

    pub fn as_slice(&self) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl<T> Drop for PinnedBuffer<T> {
    fn drop(&mut self) {
        unsafe { cuda_sys::cudaFreeHost(self.ptr as *mut std::ffi::c_void); }
    }
}
```

**Test pinned memory:**
```bash
cargo test -p akidb-embedding --features onnx --release \
  --test memory_test test_pinned_buffer -- --nocapture
```

**Success Metric:** Memory copy latency reduces from 5ms → ~1-2ms

**Task 3.2: CUDA Streams** [2 hours]
```rust
// Add to crates/akidb-embedding/src/memory.rs

use cuda_sys::{cudaStream_t, cudaStreamCreate, cudaStreamSynchronize};

pub struct CudaStreamPool {
    streams: Vec<cudaStream_t>,
}

impl CudaStreamPool {
    pub fn new(num_streams: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let mut streams = Vec::with_capacity(num_streams);
        for _ in 0..num_streams {
            let mut stream: cudaStream_t = std::ptr::null_mut();
            unsafe {
                let status = cudaStreamCreate(&mut stream);
                if status != cuda_sys::cudaError_t::cudaSuccess {
                    return Err(format!("cudaStreamCreate failed: {:?}", status).into());
                }
            }
            streams.push(stream);
        }
        Ok(Self { streams })
    }
}

// Use 4 streams: tokenization, H2D, inference, D2H
let stream_pool = CudaStreamPool::new(4)?;
```

**Test CUDA streams:**
```bash
cargo test -p akidb-embedding --features onnx --release \
  --test memory_test test_cuda_streams -- --nocapture
```

**Success Metric:** Overlap CPU/GPU operations (~10% latency reduction)

### Afternoon (3 hours)

**Task 3.3: Buffer Pooling** [2 hours]
```rust
// Add to crates/akidb-embedding/src/memory.rs

pub struct BufferPool {
    buffers: Vec<Arc<Mutex<PinnedBuffer<f32>>>>,
    available: Arc<Mutex<Vec<usize>>>,
}

impl BufferPool {
    pub fn new(pool_size: usize, buffer_capacity: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let mut buffers = Vec::with_capacity(pool_size);
        for _ in 0..pool_size {
            let buffer = PinnedBuffer::new(buffer_capacity)?;
            buffers.push(Arc::new(Mutex::new(buffer)));
        }

        let available: Vec<usize> = (0..pool_size).collect();

        Ok(Self {
            buffers,
            available: Arc::new(Mutex::new(available)),
        })
    }

    pub async fn acquire(&self) -> Option<Arc<Mutex<PinnedBuffer<f32>>>> {
        let mut available = self.available.lock().await;
        available.pop().map(|idx| self.buffers[idx].clone())
    }

    pub async fn release(&self, buffer: Arc<Mutex<PinnedBuffer<f32>>>) {
        let mut available = self.available.lock().await;
        for (idx, buf) in self.buffers.iter().enumerate() {
            if Arc::ptr_eq(buf, &buffer) {
                available.push(idx);
                break;
            }
        }
    }
}
```

**Task 3.4: Integration Test** [1 hour]
```bash
# Test memory optimizations end-to-end
cargo test -p akidb-embedding --features onnx --release \
  --test qwen3_integration_test test_single_embedding -- --nocapture

# Verify latency reduction
# Expected: 20-25ms → ~18-20ms
```

**Success Metric:** Total memory copy overhead <1ms

### End of Day 3

**Deliverables:**
- ✅ Pinned memory allocator implemented
- ✅ 4 CUDA streams created
- ✅ Buffer pooling (8 buffers)
- ✅ Memory optimization tested

**Expected Latency:** ~18-20ms P95 (small improvement from memory optimization)

---

## Day 4: Dynamic Batching (Thursday)

### Morning (4 hours)

**Task 4.1: Batching Queue** [2 hours]
```rust
// Add to crates/akidb-embedding/src/batching.rs

use tokio::sync::mpsc;
use std::time::{Duration, Instant};

pub struct BatchingQueue {
    sender: mpsc::UnboundedSender<BatchRequest>,
    receiver: Arc<Mutex<mpsc::UnboundedReceiver<BatchRequest>>>,
    config: BatchingConfig,
}

pub struct BatchingConfig {
    pub max_batch_size: usize,    // 32
    pub batch_timeout: Duration,  // 5ms
    pub max_queue_size: usize,    // 500
}

pub struct BatchRequest {
    pub texts: Vec<String>,
    pub response_tx: oneshot::Sender<Result<Vec<Vec<f32>>, EmbeddingError>>,
}

impl BatchingQueue {
    pub async fn collect_batch(&self) -> Vec<BatchRequest> {
        let mut batch = Vec::new();
        let deadline = Instant::now() + self.config.batch_timeout;

        let mut receiver = self.receiver.lock().await;

        loop {
            let timeout = deadline.saturating_duration_since(Instant::now());

            match tokio::time::timeout(timeout, receiver.recv()).await {
                Ok(Some(request)) => {
                    batch.push(request);
                    if batch.len() >= self.config.max_batch_size {
                        break;
                    }
                }
                Ok(None) => break,
                Err(_) => break,  // Timeout
            }
        }

        batch
    }
}
```

**Task 4.2: Batch Processor** [2 hours]
```rust
// Add to crates/akidb-embedding/src/batching.rs

pub struct BatchProcessor {
    provider: Arc<OnnxEmbeddingProvider>,
    queue: Arc<BatchingQueue>,
    metrics: Arc<BatchMetrics>,
}

impl BatchProcessor {
    pub async fn start(&self) {
        loop {
            let requests = self.queue.collect_batch().await;

            if requests.is_empty() {
                continue;
            }

            // Merge all texts
            let texts: Vec<String> = requests.iter()
                .flat_map(|req| req.texts.clone())
                .collect();

            let batch_size = texts.len();
            let start = Instant::now();

            // Process batch
            match self.provider.embed_batch_internal(texts).await {
                Ok(embeddings) => {
                    // Split embeddings back to requests
                    let mut offset = 0;
                    for request in requests {
                        let count = request.texts.len();
                        let batch_emb = embeddings[offset..offset + count].to_vec();
                        offset += count;

                        let _ = request.response_tx.send(Ok(batch_emb));
                    }

                    let duration = start.elapsed();
                    self.metrics.record_batch(batch_size, duration);
                }
                Err(err) => {
                    for request in requests {
                        let _ = request.response_tx.send(Err(err.clone()));
                    }
                }
            }
        }
    }

    pub async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let (tx, rx) = oneshot::channel();
        self.queue.submit(BatchRequest { texts, response_tx: tx }).await?;
        rx.await.map_err(|_| EmbeddingError::Internal("Channel closed".to_string()))?
    }
}
```

**Test batching:**
```bash
cargo test -p akidb-embedding --features onnx --release \
  --test batching_test test_dynamic_batching -- --nocapture
```

**Success Metric:** Dynamic batching working, avg batch size >4

### Afternoon (3 hours)

**Task 4.3: Metrics Collection** [1 hour]
```rust
use std::sync::atomic::{AtomicU64, Ordering};

pub struct BatchMetrics {
    pub total_batches: AtomicU64,
    pub total_requests: AtomicU64,
    pub avg_batch_size: AtomicU64,
    pub avg_latency_us: AtomicU64,
}

impl BatchMetrics {
    pub fn record_batch(&self, batch_size: usize, duration: Duration) {
        self.total_batches.fetch_add(1, Ordering::Relaxed);
        self.total_requests.fetch_add(batch_size as u64, Ordering::Relaxed);

        let avg_batch_size = self.total_requests.load(Ordering::Relaxed)
            / self.total_batches.load(Ordering::Relaxed);
        self.avg_batch_size.store(avg_batch_size, Ordering::Relaxed);

        let latency_us = duration.as_micros() as u64;
        self.avg_latency_us.store(latency_us, Ordering::Relaxed);
    }
}
```

**Task 4.4: Throughput Benchmarking** [2 hours]
```bash
# Benchmark throughput with dynamic batching
cat > /tmp/benchmark_throughput.sh << 'SCRIPT'
#!/bin/bash

echo "Benchmarking throughput with dynamic batching..."

# Single-threaded
cargo bench --bench qwen3_bench -- throughput_single_threaded --measurement-time 30 | \
  grep "QPS" | tee /tmp/throughput_single.txt

# Concurrent (4 threads)
cargo bench --bench qwen3_bench -- throughput_concurrent --measurement-time 30 | \
  grep "QPS" | tee /tmp/throughput_concurrent.txt

echo "Results:"
echo "Single-threaded:"
cat /tmp/throughput_single.txt
echo "Concurrent (4 threads):"
cat /tmp/throughput_concurrent.txt
SCRIPT

chmod +x /tmp/benchmark_throughput.sh
./benchmark_throughput.sh
```

**Success Metric:** Throughput >50 QPS (single), >150 QPS (concurrent)

### End of Day 4

**Deliverables:**
- ✅ Batching queue with timeout dispatch
- ✅ Batch processor with request merging
- ✅ Metrics collection
- ✅ Throughput benchmarks

**Expected Throughput:** 15 QPS → >50 QPS (3.3x improvement)

---

## Day 5: Testing & Documentation (Friday)

### Morning (3 hours)

**Task 5.1: End-to-End Performance Testing** [3 hours]
```bash
cat > ~/akidb2/scripts/week3_e2e_test.sh << 'SCRIPT'
#!/bin/bash
set -e

echo "🚀 Week 3 End-to-End Performance Testing"
echo "========================================="
echo

# Test 1: Latency (P95 <30ms)
echo "📊 Test 1: Single Embedding Latency"
cargo test -p akidb-embedding --features onnx --release \
  --test qwen3_integration_test test_single_embedding -- --nocapture 2>&1 | \
  grep "Duration:" | tee -a /tmp/week3_results.txt

# Test 2: Throughput (>50 QPS)
echo
echo "📊 Test 2: Throughput"
cargo bench --bench qwen3_bench -- throughput_single --measurement-time 30 2>&1 | \
  grep "QPS" | tee -a /tmp/week3_results.txt

# Test 3: Concurrent throughput (>150 QPS)
echo
echo "📊 Test 3: Concurrent Throughput (4 threads)"
cargo bench --bench qwen3_bench -- throughput_concurrent --measurement-time 30 2>&1 | \
  grep "QPS" | tee -a /tmp/week3_results.txt

# Test 4: GPU memory usage (<4GB)
echo
echo "📊 Test 4: GPU Memory Usage"
nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits | \
  tee -a /tmp/week3_results.txt

# Test 5: Quality validation (>0.99 cosine similarity)
echo
echo "📊 Test 5: Embedding Quality"
python3 /opt/akidb/scripts/validate_quality.py 2>&1 | \
  grep "Cosine similarity" | tee -a /tmp/week3_results.txt

echo
echo "✅ Week 3 testing complete!"
echo "Results: /tmp/week3_results.txt"
SCRIPT

chmod +x ~/akidb2/scripts/week3_e2e_test.sh
./week3_e2e_test.sh
```

**Success Metrics:**
- ✅ P95 latency <30ms
- ✅ Throughput >50 QPS (single), >150 QPS (concurrent)
- ✅ GPU memory <4GB
- ✅ Quality >0.99 cosine similarity

### Afternoon (4 hours)

**Task 5.2: Completion Report** [2 hours]
```bash
# Fill in Week 3 completion report with actual results
cd ~/akidb2

# Extract results from testing
LATENCY_P95=$(grep "Duration:" /tmp/week3_results.txt | awk '{print $2}')
THROUGHPUT_SINGLE=$(grep "QPS" /tmp/week3_results.txt | head -1 | awk '{print $2}')
THROUGHPUT_CONCURRENT=$(grep "QPS" /tmp/week3_results.txt | tail -1 | awk '{print $2}')
GPU_MEMORY=$(grep -v "memory.used" /tmp/week3_results.txt | tail -1)
QUALITY=$(grep "Cosine similarity" /tmp/week3_results.txt | awk '{print $3}')

# Update completion report
sed -i "s/\[INSERT\]ms/$LATENCY_P95/g" automatosx/tmp/JETSON-THOR-WEEK3-COMPLETION-REPORT.md
sed -i "s/\[INSERT\] QPS/$THROUGHPUT_SINGLE/g" automatosx/tmp/JETSON-THOR-WEEK3-COMPLETION-REPORT.md
# ... (update all placeholders)

echo "✅ Completion report updated"
```

**Task 5.3: Performance Tuning Guide** [2 hours]
```bash
# Create operator guide for performance tuning
# (Already created in PRD, review and update with actual findings)

cd ~/akidb2/docs
vim JETSON-THOR-PERFORMANCE-TUNING-GUIDE.md

# Add sections:
# - Optimal configuration discovered
# - Troubleshooting based on actual issues encountered
# - Performance comparison table (Week 2 vs Week 3)
```

### End of Day 5

**Deliverables:**
- ✅ End-to-end performance tests passing
- ✅ Week 3 completion report (with actual results)
- ✅ Performance tuning guide
- ✅ All documentation updated

**Final Status:** Week 3 COMPLETE ✅

---

## Quick Command Reference

```bash
# Day 1: Enable FP8 and create profiles
rm -rf /tmp/akidb_trt_cache/*
cargo build --release -p akidb-embedding --features onnx
python3 /opt/akidb/scripts/generate_trt_profiles.py

# Day 2: Test tactics and workspace
python3 /tmp/test_workspace.py
nsys profile --trace=cuda cargo test ... test_single_embedding

# Day 3: Build memory optimizations
cargo build --release -p akidb-embedding --features onnx,cuda
cargo test -p akidb-embedding --features onnx --test memory_test

# Day 4: Test dynamic batching
cargo test -p akidb-embedding --features onnx --test batching_test
cargo bench --bench qwen3_bench -- throughput

# Day 5: Run E2E tests
./scripts/week3_e2e_test.sh
```

---

## Success Criteria Summary

| Metric | Week 2 Baseline | Week 3 Target | Pass/Fail |
|--------|-----------------|---------------|-----------|
| **Latency (P95, batch=1)** | ~80ms | <30ms | TBD |
| **Throughput (single)** | ~15 QPS | >50 QPS | TBD |
| **Throughput (concurrent)** | ~40 QPS | >150 QPS | TBD |
| **GPU Memory** | ~3.5GB | <4GB | TBD |
| **Quality (cosine sim)** | >0.99 | >0.99 | TBD |

**Week 3 Status:** 🚧 **READY TO EXECUTE**

---

## Risk Mitigation Checklist

**Before Starting:**
- [ ] Week 2 baseline performance documented (80ms, 15 QPS)
- [ ] Jetson Thor has JetPack 6.0+
- [ ] TensorRT 9.0+ installed
- [ ] Sufficient GPU memory (64GB unified RAM)
- [ ] Backup of Week 2 code (git tag `week2-baseline`)

**During Execution:**
- [ ] Benchmark after each optimization (incremental validation)
- [ ] Keep TensorRT cache backups (rollback if needed)
- [ ] Monitor GPU memory (avoid OOM)
- [ ] Test quality after each optimization (maintain >0.99)

**Rollback Plan:**
If Week 3 optimizations fail to meet targets:
1. Revert to Week 2 baseline (`git checkout week2-baseline`)
2. Identify which optimization caused regression
3. Apply optimizations individually (isolate issue)
4. Document findings and adjust targets

---

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>
