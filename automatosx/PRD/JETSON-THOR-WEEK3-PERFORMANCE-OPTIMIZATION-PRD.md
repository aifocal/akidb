# Jetson Thor Week 3: Performance Optimization & TensorRT Tuning PRD

**Status:** Ready to Execute
**Timeline:** 5 days (Week 3)
**Owner:** Backend Team + Performance Engineering
**Dependencies:** Week 1 (✅ Complete), Week 2 (✅ Complete)
**Target Platform:** NVIDIA Jetson Thor (Blackwell GPU, 2,000 TOPS)

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Goals & Non-Goals](#goals--non-goals)
3. [Baseline Analysis](#baseline-analysis)
4. [Optimization Strategy](#optimization-strategy)
5. [Day-by-Day Implementation Plan](#day-by-day-implementation-plan)
6. [TensorRT Optimization Techniques](#tensorrt-optimization-techniques)
7. [Performance Validation](#performance-validation)
8. [Risk Management](#risk-management)
9. [Success Criteria](#success-criteria)
10. [Appendix: Code Examples](#appendix-code-examples)

---

## Executive Summary

Week 3 focuses on **performance optimization** to achieve production-grade latency and throughput targets on Jetson Thor. Building on the baseline established in Week 2 (50-100ms P95), we will optimize TensorRT execution, implement dynamic batching, and tune inference parameters to achieve **<30ms P95 latency** and **>50 QPS throughput**.

### Key Objectives

1. **Latency Optimization:** Reduce P95 latency from ~80ms (Week 2 baseline) to <30ms
2. **Throughput Optimization:** Increase throughput from ~15 QPS to >50 QPS
3. **TensorRT Tuning:** Optimize TensorRT engine profiles, tactics, and workspace
4. **Dynamic Batching:** Implement request batching with timeout-based dispatch
5. **Memory Optimization:** Reduce GPU memory usage to enable concurrent inference

### Expected Outcomes

- ✅ P95 latency <30ms for single embedding (batch size 1)
- ✅ P95 latency <50ms for batch size 8
- ✅ Throughput >50 QPS (single-threaded), >150 QPS (concurrent)
- ✅ GPU memory usage <4GB (model + workspace)
- ✅ TensorRT engine optimized for Jetson Thor's Blackwell GPU
- ✅ Production-ready performance for automotive/robotics workloads

---

## Goals & Non-Goals

### Goals (Week 3)

**Primary Goals:**
1. ✅ **Reduce P95 latency to <30ms** (single embedding, batch size 1)
2. ✅ **Achieve >50 QPS throughput** (single-threaded)
3. ✅ **Optimize TensorRT engine** for Blackwell FP8 Tensor Cores
4. ✅ **Implement dynamic batching** with timeout-based dispatch
5. ✅ **Profile and optimize** memory usage (<4GB GPU memory)
6. ✅ **Document optimization techniques** for reproducibility

**Secondary Goals:**
- 📊 Achieve >150 QPS with concurrent requests (4 threads)
- 📊 Benchmark different TensorRT optimization profiles
- 📊 Compare FP8 vs FP16 vs FP32 performance/quality trade-offs
- 📊 Create performance tuning guide for operators

### Non-Goals (Deferred to Week 4+)

**Not in Scope for Week 3:**
- ❌ Multi-model support (different embedding models) - Week 4
- ❌ Kubernetes deployment - Week 5
- ❌ Production API server integration - Week 5
- ❌ Distributed inference (multi-GPU) - Week 6+
- ❌ Model quantization beyond FP8 (INT8, INT4) - Week 7+

---

## Baseline Analysis

### Week 2 Baseline Performance

Based on Week 2 integration testing, the baseline performance on Jetson Thor is:

**Latency (P95):**
- Batch size 1: **~80ms** (target: <30ms, **2.7x improvement needed**)
- Batch size 8: **~200ms** (target: <50ms, **4x improvement needed**)
- Batch size 32: **~500ms** (acceptable for batch workloads)

**Throughput:**
- Single-threaded: **~15 QPS** (target: >50 QPS, **3.3x improvement needed**)
- Concurrent (4 threads): **~40 QPS** (target: >150 QPS, **3.8x improvement needed**)

**Resource Usage:**
- GPU memory: **~3.5GB** (model 2GB + workspace 1.5GB, target: <4GB ✅)
- CPU memory: **~1.2GB** (tokenizer + buffers)
- TensorRT engine build time: **~4 minutes** (first run, cached after)

### Performance Bottlenecks Identified

From Week 2 profiling (NVIDIA Nsight Systems, TensorRT verbose logs):

1. **TensorRT Engine Not Optimized** 🔴 **HIGH IMPACT**
   - Default optimization level (no custom profiles)
   - Not using FP8 Tensor Cores (falling back to FP16)
   - Conservative workspace size limit (512MB)
   - Missing layer fusion opportunities

2. **Tokenization Overhead** 🟡 **MEDIUM IMPACT**
   - HuggingFace tokenizers crate: ~5-8ms per batch
   - CPU-bound operation (not using GPU)
   - Batching not optimized

3. **Memory Copy Overhead** 🟡 **MEDIUM IMPACT**
   - CPU → GPU memory transfer: ~3-5ms
   - Not using pinned memory
   - Not using CUDA streams for overlap

4. **No Dynamic Batching** 🟡 **MEDIUM IMPACT**
   - Single-request processing (no batching)
   - Missing throughput optimization opportunity
   - Underutilizing GPU (40% utilization observed)

5. **Inference Configuration** 🟢 **LOW IMPACT**
   - Using default thread count (4 threads)
   - No async inference
   - No request pipelining

### Optimization Potential

| Bottleneck | Current Cost | Optimized Cost | Improvement |
|------------|--------------|----------------|-------------|
| TensorRT engine | ~40ms | ~15ms | **2.7x** |
| Tokenization | ~8ms | ~2ms | **4x** |
| Memory copy | ~5ms | ~1ms | **5x** |
| Dynamic batching | N/A | +20% throughput | **1.2x** |
| **Total (P95)** | **~80ms** | **<25ms** | **3.2x** |

**Verdict:** ✅ **<30ms P95 target is achievable** with TensorRT optimization + memory copy reduction

---

## Optimization Strategy

### Three-Pillar Approach

```
┌────────────────────────────────────────────────────────────┐
│           Week 3 Performance Optimization                  │
│                                                            │
│  Pillar 1:          Pillar 2:          Pillar 3:          │
│  TensorRT          Memory              Batching           │
│  Optimization      Optimization        Optimization       │
│                                                            │
│  • FP8 Tensor      • Pinned memory    • Dynamic batching  │
│  • Profiles        • CUDA streams     • Timeout dispatch  │
│  • Tactics         • Zero-copy        • Load balancing    │
│  • Workspace       • Buffer pooling   • Backpressure      │
│                                                            │
│  Target: 40→15ms   Target: 5→1ms      Target: +30% QPS   │
└────────────────────────────────────────────────────────────┘
```

### Pillar 1: TensorRT Engine Optimization (Days 1-2)

**Goal:** Reduce inference time from ~40ms to ~15ms (2.7x)

**Techniques:**
1. **Enable FP8 Tensor Cores** (Blackwell GPU)
   - Use `trt_fp8_enable=true` flag
   - Verify FP8 kernels are selected (Nsight profiling)
   - Fallback to FP16 if quality drops

2. **Optimization Profiles**
   - Create custom TensorRT profiles for common batch sizes (1, 4, 8)
   - Specify min/opt/max shapes for dynamic batching
   - Allow TensorRT to specialize for Jetson Thor

3. **Tactic Selection**
   - Use `trt_tactic_sources=CUBLAS,CUDNN,EDGE_MASK` (all sources)
   - Enable timing cache for kernel selection
   - Benchmark different tactics (Day 2)

4. **Workspace Size**
   - Increase from 512MB to 2GB (more optimization headroom)
   - Balance memory vs performance trade-off
   - Monitor GPU memory usage

5. **Graph Optimization**
   - Verify layer fusion (Nsight Systems)
   - Check for suboptimal patterns (e.g., separate LayerNorm)
   - Rebuild engine with verbose logging

### Pillar 2: Memory Optimization (Day 3)

**Goal:** Reduce memory copy overhead from ~5ms to ~1ms (5x)

**Techniques:**
1. **Pinned Memory**
   - Allocate input/output buffers in pinned (page-locked) memory
   - Faster CPU ↔ GPU transfers (PCIe bandwidth)
   - Reduce copy latency by 3-4x

2. **CUDA Streams**
   - Use separate CUDA streams for:
     - Tokenization (CPU)
     - Memory copy (H2D)
     - Inference (GPU)
     - Memory copy (D2H)
   - Overlap CPU and GPU operations

3. **Zero-Copy Buffers**
   - Reuse pre-allocated buffers (no reallocation)
   - Buffer pooling for common batch sizes
   - Reduce allocation overhead

4. **Unified Memory** (Jetson Thor advantage)
   - Explore Jetson Thor's unified CPU+GPU memory
   - Potential to eliminate explicit copies
   - Test performance vs explicit management

### Pillar 3: Dynamic Batching (Day 4)

**Goal:** Increase throughput from ~15 QPS to >50 QPS (3.3x)

**Techniques:**
1. **Request Batching**
   - Collect incoming requests into batches
   - Timeout-based dispatch (e.g., 5ms wait for batch)
   - Max batch size: 32 (trade-off latency vs throughput)

2. **Adaptive Batching**
   - Adjust batch size based on load
   - Low load: batch size 1-4 (low latency)
   - High load: batch size 8-32 (high throughput)

3. **Priority Queue**
   - High-priority requests bypass batching (low latency)
   - Normal requests use batching (high throughput)
   - Configurable priority via API

4. **Backpressure Handling**
   - Queue size limits (prevent OOM)
   - Reject requests when queue full (fail fast)
   - Metrics: queue depth, wait time

---

## Day-by-Day Implementation Plan

### Day 1: TensorRT Profile Optimization (Monday)

**Objective:** Create optimized TensorRT engine with FP8 Tensor Cores and custom profiles.

**Tasks:**

#### 1.1: Enable FP8 Tensor Cores (2 hours)

**Background:** Week 2 baseline is using FP16 (not FP8), despite `fp8_enable=true` flag. TensorRT may be falling back to FP16 if FP8 kernels are not properly configured.

```bash
cd ~/akidb2

# Update ONNX provider to force FP8 usage
cat > /tmp/patch_fp8.patch << 'EOF'
--- a/crates/akidb-embedding/src/onnx.rs
+++ b/crates/akidb-embedding/src/onnx.rs
@@ -120,6 +120,12 @@ impl OnnxEmbeddingProvider {
             ExecutionProviderConfig::TensorRT { device_id, fp8_enable, engine_cache_path } => {
                 let mut trt_options = ort::TensorRTExecutionProviderOptions::default();
                 trt_options.device_id = *device_id;
+
+                // Force FP8 precision (Blackwell GPU)
+                trt_options.fp16_enable = false;  // Disable FP16
+                trt_options.int8_enable = false;  // Disable INT8
+                trt_options.fp8_enable = *fp8_enable;  // Enable FP8
+
                 trt_options.max_workspace_size = 2_000_000_000;  // 2GB workspace
                 trt_options.engine_cache_enable = true;
                 trt_options.timing_cache_enable = true;
@@ -128,6 +134,14 @@ impl OnnxEmbeddingProvider {
                     trt_options.engine_cache_path = path.to_string_lossy().to_string();
                 }

+                // Force strong types (FP8 only, no fallback)
+                trt_options.builder_optimization_level = 5;  // Max optimization
+                trt_options.trt_tactic_sources = "CUBLAS,CUDNN,EDGE_MASK_CONVOLUTIONS";
+
+                eprintln!("🔧 TensorRT EP Configuration:");
+                eprintln!("   FP8 enabled: {}", trt_options.fp8_enable);
+                eprintln!("   Workspace size: {} GB", trt_options.max_workspace_size as f64 / 1e9);
+
                 builder = builder.with_execution_provider(
                     ort::ExecutionProvider::TensorRT(trt_options)
                 )?;
EOF

patch -p1 < /tmp/patch_fp8.patch

# Rebuild and test
cargo build --release -p akidb-embedding --features onnx

# Delete old TensorRT cache (force rebuild)
rm -rf /tmp/akidb_trt_cache/*

# Run test to trigger TensorRT engine rebuild
RUST_LOG=debug cargo test -p akidb-embedding --features onnx --release \
  --test qwen3_integration_test test_single_embedding -- --nocapture 2>&1 | tee /tmp/tensorrt_rebuild.log

# Check for FP8 usage in logs
grep -i "fp8\|precision\|kernel" /tmp/tensorrt_rebuild.log
```

**Success Criteria:**
- ✅ TensorRT engine rebuilds with FP8 configuration
- ✅ Logs show FP8 kernels selected (not FP16 fallback)
- ✅ Latency improvement observed (40ms → ~30ms)

#### 1.2: Optimization Profiles (3 hours)

**Background:** TensorRT optimization profiles allow specialization for different input shapes (batch sizes). Without profiles, TensorRT uses conservative defaults.

```python
# Create TensorRT profile generation script
cat > /opt/akidb/scripts/generate_trt_profiles.py << 'EOF'
#!/usr/bin/env python3
"""
Generate optimized TensorRT profiles for Qwen3 4B on Jetson Thor.

Profiles specify min/opt/max shapes for dynamic inputs, allowing
TensorRT to optimize for specific batch sizes.
"""

import onnx
import onnxruntime as ort
from pathlib import Path
import json

def create_optimization_profiles():
    """
    Create TensorRT profiles for common batch sizes.

    Profile 1: Low latency (batch 1-2, optimized for batch 1)
    Profile 2: Balanced (batch 2-8, optimized for batch 4)
    Profile 3: High throughput (batch 8-32, optimized for batch 16)
    """

    profiles = [
        {
            "name": "low_latency",
            "min_batch": 1,
            "opt_batch": 1,
            "max_batch": 2,
            "description": "Optimized for single-request latency (<30ms)"
        },
        {
            "name": "balanced",
            "min_batch": 2,
            "opt_batch": 4,
            "max_batch": 8,
            "description": "Balanced latency and throughput"
        },
        {
            "name": "high_throughput",
            "min_batch": 8,
            "opt_batch": 16,
            "max_batch": 32,
            "description": "Optimized for throughput (>50 QPS)"
        }
    ]

    model_path = Path("/opt/akidb/models/qwen3-4b-onnx-fp8/model.onnx")

    for profile in profiles:
        print(f"\n🔧 Creating profile: {profile['name']}")
        print(f"   Min batch: {profile['min_batch']}")
        print(f"   Opt batch: {profile['opt_batch']}")
        print(f"   Max batch: {profile['max_batch']}")

        # TensorRT session options
        sess_options = ort.SessionOptions()
        sess_options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL

        # TensorRT EP options with profile
        trt_options = {
            'device_id': 0,
            'trt_fp8_enable': True,
            'trt_fp16_enable': False,
            'trt_int8_enable': False,
            'trt_max_workspace_size': 2_000_000_000,  # 2GB
            'trt_engine_cache_enable': True,
            'trt_engine_cache_path': f'/opt/akidb/trt_cache/{profile["name"]}',
            'trt_timing_cache_enable': True,
            'trt_builder_optimization_level': 5,  # Max optimization

            # Profile shapes (input_ids: [batch, seq_len])
            'trt_profile_min_shapes': f'input_ids:{profile["min_batch"]}x512',
            'trt_profile_opt_shapes': f'input_ids:{profile["opt_batch"]}x512',
            'trt_profile_max_shapes': f'input_ids:{profile["max_batch"]}x512',
        }

        print(f"   Profile shapes: {profile['min_batch']}x512 → {profile['opt_batch']}x512 → {profile['max_batch']}x512")

        # Create session (will build TensorRT engine with profile)
        print("   Building TensorRT engine (2-5 minutes)...")
        session = ort.InferenceSession(
            str(model_path),
            sess_options,
            providers=[('TensorrtExecutionProvider', trt_options)]
        )

        print(f"   ✅ Profile created: /opt/akidb/trt_cache/{profile['name']}/")

        # Save profile metadata
        profile_file = Path(f"/opt/akidb/trt_cache/{profile['name']}/profile.json")
        profile_file.parent.mkdir(parents=True, exist_ok=True)
        with open(profile_file, 'w') as f:
            json.dump(profile, f, indent=2)

    print("\n✅ All optimization profiles created!")

if __name__ == "__main__":
    create_optimization_profiles()
EOF

chmod +x /opt/akidb/scripts/generate_trt_profiles.py

# Generate profiles (will take ~15 minutes total: 3 profiles × 5 min each)
python3 /opt/akidb/scripts/generate_trt_profiles.py 2>&1 | tee /tmp/profile_generation.log
```

**Success Criteria:**
- ✅ 3 TensorRT profiles created (low_latency, balanced, high_throughput)
- ✅ Profile metadata saved to `/opt/akidb/trt_cache/*/profile.json`
- ✅ Each profile builds successfully (<5 min)

#### 1.3: Benchmark Profiles (2 hours)

```bash
# Create profile benchmarking script
cat > /opt/akidb/scripts/benchmark_profiles.py << 'EOF'
#!/usr/bin/env python3
"""
Benchmark different TensorRT profiles to find optimal configuration.
"""

import onnxruntime as ort
import numpy as np
import time
from pathlib import Path

def benchmark_profile(profile_name, batch_size, num_iterations=100):
    """Benchmark a specific TensorRT profile."""

    model_path = "/opt/akidb/models/qwen3-4b-onnx-fp8/model.onnx"
    cache_path = f"/opt/akidb/trt_cache/{profile_name}"

    # Create session with profile
    sess_options = ort.SessionOptions()
    trt_options = {
        'device_id': 0,
        'trt_fp8_enable': True,
        'trt_engine_cache_enable': True,
        'trt_engine_cache_path': cache_path,
    }

    session = ort.InferenceSession(
        model_path,
        sess_options,
        providers=[('TensorrtExecutionProvider', trt_options)]
    )

    # Prepare input (random tokens)
    input_ids = np.random.randint(0, 151936, size=(batch_size, 512), dtype=np.int64)
    attention_mask = np.ones((batch_size, 512), dtype=np.int64)

    # Warmup
    for _ in range(10):
        session.run(None, {'input_ids': input_ids, 'attention_mask': attention_mask})

    # Benchmark
    latencies = []
    for _ in range(num_iterations):
        start = time.perf_counter()
        session.run(None, {'input_ids': input_ids, 'attention_mask': attention_mask})
        latencies.append((time.perf_counter() - start) * 1000)  # ms

    # Statistics
    latencies = np.array(latencies)
    return {
        'profile': profile_name,
        'batch_size': batch_size,
        'p50': np.percentile(latencies, 50),
        'p95': np.percentile(latencies, 95),
        'p99': np.percentile(latencies, 99),
        'mean': np.mean(latencies),
        'std': np.std(latencies),
    }

def main():
    print("🚀 Benchmarking TensorRT Profiles\n")

    test_cases = [
        ('low_latency', 1),
        ('balanced', 1),
        ('balanced', 4),
        ('high_throughput', 8),
        ('high_throughput', 16),
    ]

    results = []
    for profile, batch in test_cases:
        print(f"📊 Benchmarking {profile} @ batch={batch}...")
        result = benchmark_profile(profile, batch)
        results.append(result)

        print(f"   P50: {result['p50']:.2f}ms")
        print(f"   P95: {result['p95']:.2f}ms")
        print(f"   P99: {result['p99']:.2f}ms")
        print()

    # Find best profile for batch=1 (primary target)
    batch1_results = [r for r in results if r['batch_size'] == 1]
    best = min(batch1_results, key=lambda r: r['p95'])

    print(f"\n✅ Best profile for batch=1: {best['profile']}")
    print(f"   P95 latency: {best['p95']:.2f}ms")

    return 0 if best['p95'] < 30 else 1  # Exit 0 if <30ms target met

if __name__ == "__main__":
    exit(main())
EOF

chmod +x /opt/akidb/scripts/benchmark_profiles.py
python3 /opt/akidb/scripts/benchmark_profiles.py 2>&1 | tee /tmp/profile_benchmarks.txt
```

**Success Criteria:**
- ✅ All profiles benchmarked successfully
- ✅ `low_latency` profile achieves <30ms P95 for batch=1
- ✅ Best profile identified and documented

**Estimated Time (Day 1):** 7 hours (2h FP8 + 3h profiles + 2h benchmarking)

---

### Day 2: TensorRT Tactics & Workspace Tuning (Tuesday)

**Objective:** Fine-tune TensorRT tactics selection and workspace size for optimal performance.

#### 2.1: Tactic Source Selection (2 hours)

**Background:** TensorRT can use different kernel sources (CUBLAS, cuDNN, edge mask convolutions). Testing different combinations finds fastest kernels.

```rust
// Update onnx.rs to allow tactic source configuration
// crates/akidb-embedding/src/onnx.rs

pub struct TensorRTConfig {
    pub device_id: i32,
    pub fp8_enable: bool,
    pub engine_cache_path: Option<PathBuf>,
    pub workspace_size: u64,  // NEW
    pub tactic_sources: String,  // NEW (e.g., "CUBLAS,CUDNN,EDGE_MASK_CONVOLUTIONS")
    pub builder_opt_level: u32,  // NEW (0-5, 5=max)
}

impl Default for TensorRTConfig {
    fn default() -> Self {
        Self {
            device_id: 0,
            fp8_enable: true,
            engine_cache_path: Some(PathBuf::from("/tmp/akidb_trt_cache")),
            workspace_size: 2_000_000_000,  // 2GB (increased from 512MB)
            tactic_sources: "CUBLAS,CUDNN,EDGE_MASK_CONVOLUTIONS".to_string(),
            builder_opt_level: 5,  // Max optimization
        }
    }
}

// Apply in TensorRT EP configuration
let mut trt_options = ort::TensorRTExecutionProviderOptions::default();
trt_options.trt_max_workspace_size = config.workspace_size;
trt_options.trt_tactic_sources = config.tactic_sources.clone();
trt_options.trt_builder_optimization_level = config.builder_opt_level;
```

**Test different tactic combinations:**

```bash
cd ~/akidb2

# Test Tactic 1: CUBLAS only (baseline)
TACTIC_SOURCES="CUBLAS" cargo bench --bench qwen3_bench -- --warm-up-time 1 --measurement-time 30 batch_sizes/1

# Test Tactic 2: CUBLAS + cuDNN
TACTIC_SOURCES="CUBLAS,CUDNN" cargo bench --bench qwen3_bench -- --warm-up-time 1 --measurement-time 30 batch_sizes/1

# Test Tactic 3: All sources (CUBLAS + cuDNN + edge mask)
TACTIC_SOURCES="CUBLAS,CUDNN,EDGE_MASK_CONVOLUTIONS" cargo bench --bench qwen3_bench -- --warm-up-time 1 --measurement-time 30 batch_sizes/1

# Compare results
echo "Tactic comparison:"
grep "time:" target/criterion/qwen3_batch_sizes/1/*/base/estimates.json
```

**Success Criteria:**
- ✅ 3 tactic configurations tested
- ✅ Fastest configuration identified
- ✅ Latency improvement documented (expect 5-10% improvement)

#### 2.2: Workspace Size Tuning (2 hours)

**Background:** Larger workspace gives TensorRT more optimization freedom (layer fusion, kernel selection), but uses more GPU memory.

```python
# Test different workspace sizes
cat > /opt/akidb/scripts/test_workspace_sizes.py << 'EOF'
#!/usr/bin/env python3
"""
Test different TensorRT workspace sizes to find optimal memory/performance trade-off.
"""

import onnxruntime as ort
import numpy as np
import time
from pathlib import Path

def benchmark_workspace_size(workspace_mb, batch_size=1, num_iterations=100):
    """Benchmark with specific workspace size."""

    model_path = "/opt/akidb/models/qwen3-4b-onnx-fp8/model.onnx"
    cache_path = f"/tmp/workspace_test_{workspace_mb}mb"
    Path(cache_path).mkdir(parents=True, exist_ok=True)

    sess_options = ort.SessionOptions()
    trt_options = {
        'device_id': 0,
        'trt_fp8_enable': True,
        'trt_max_workspace_size': workspace_mb * 1024 * 1024,
        'trt_engine_cache_enable': True,
        'trt_engine_cache_path': cache_path,
        'trt_builder_optimization_level': 5,
    }

    print(f"Building TensorRT engine with {workspace_mb}MB workspace...")
    session = ort.InferenceSession(
        model_path,
        sess_options,
        providers=[('TensorrtExecutionProvider', trt_options)]
    )

    # Benchmark
    input_ids = np.random.randint(0, 151936, size=(batch_size, 512), dtype=np.int64)
    attention_mask = np.ones((batch_size, 512), dtype=np.int64)

    # Warmup
    for _ in range(10):
        session.run(None, {'input_ids': input_ids, 'attention_mask': attention_mask})

    # Measure
    latencies = []
    for _ in range(num_iterations):
        start = time.perf_counter()
        session.run(None, {'input_ids': input_ids, 'attention_mask': attention_mask})
        latencies.append((time.perf_counter() - start) * 1000)

    return {
        'workspace_mb': workspace_mb,
        'p50': np.percentile(latencies, 50),
        'p95': np.percentile(latencies, 95),
        'p99': np.percentile(latencies, 99),
    }

def main():
    print("🧪 Testing TensorRT Workspace Sizes\n")

    workspace_sizes = [512, 1024, 2048, 4096]  # MB

    results = []
    for size in workspace_sizes:
        print(f"\n📊 Workspace: {size}MB")
        result = benchmark_workspace_size(size)
        results.append(result)

        print(f"   P50: {result['p50']:.2f}ms")
        print(f"   P95: {result['p95']:.2f}ms")

    # Find sweet spot (diminishing returns)
    print("\n📈 Results:")
    for r in results:
        print(f"   {r['workspace_mb']:4d}MB → P95: {r['p95']:5.2f}ms")

    # Recommend optimal size
    best = min(results, key=lambda r: r['p95'])
    print(f"\n✅ Recommended workspace size: {best['workspace_mb']}MB")

if __name__ == "__main__":
    main()
EOF

chmod +x /opt/akidb/scripts/test_workspace_sizes.py
python3 /opt/akidb/scripts/test_workspace_sizes.py 2>&1 | tee /tmp/workspace_tuning.txt
```

**Success Criteria:**
- ✅ Workspace sizes 512MB, 1GB, 2GB, 4GB tested
- ✅ Optimal size identified (likely 2GB)
- ✅ Memory/performance trade-off documented

#### 2.3: Nsight Systems Profiling (3 hours)

**Background:** NVIDIA Nsight Systems provides deep profiling of GPU kernels, memory transfers, and CPU/GPU synchronization.

```bash
# Profile with Nsight Systems
nsys profile \
  --trace=cuda,nvtx,osrt \
  --output=/tmp/qwen3_profile.nsys-rep \
  --force-overwrite=true \
  cargo test -p akidb-embedding --features onnx --release \
    --test qwen3_integration_test test_single_embedding -- --nocapture

# Analyze profile
nsys-ui /tmp/qwen3_profile.nsys-rep &

# Export statistics
nsys stats /tmp/qwen3_profile.nsys-rep > /tmp/qwen3_profile_stats.txt

# Key metrics to check:
# 1. GPU utilization (target: >80%)
# 2. Kernel execution time (should be mostly FP8 GEMM)
# 3. Memory transfer overhead (target: <5% of total time)
# 4. CPU/GPU synchronization gaps (minimize)

echo "Key findings from Nsight profile:"
grep -E "GPU|Kernel|Memory" /tmp/qwen3_profile_stats.txt | head -20
```

**Success Criteria:**
- ✅ Nsight Systems profile captured
- ✅ GPU utilization >80% during inference
- ✅ FP8 kernels verified (not FP16 fallback)
- ✅ Bottlenecks identified and documented

**Estimated Time (Day 2):** 7 hours (2h tactics + 2h workspace + 3h profiling)

---

### Day 3: Memory & Copy Optimization (Wednesday)

**Objective:** Reduce memory copy overhead from ~5ms to <1ms using pinned memory and CUDA streams.

#### 3.1: Pinned Memory Allocation (2 hours)

**Background:** Pinned (page-locked) memory enables faster DMA transfers between CPU and GPU (~5x faster than pageable memory).

```rust
// Add pinned memory allocator to onnx.rs
use std::alloc::{alloc, dealloc, Layout};

pub struct PinnedBuffer<T> {
    ptr: *mut T,
    layout: Layout,
    len: usize,
}

impl<T> PinnedBuffer<T> {
    pub fn new(capacity: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let layout = Layout::array::<T>(capacity)?;

        // Allocate pinned memory via CUDA API
        let ptr = unsafe {
            let mut ptr: *mut T = std::ptr::null_mut();
            let status = cuda_sys::cudaMallocHost(
                &mut ptr as *mut *mut T as *mut *mut std::ffi::c_void,
                layout.size()
            );

            if status != cuda_sys::cudaError_t::cudaSuccess {
                return Err(format!("cudaMallocHost failed: {:?}", status).into());
            }

            ptr
        };

        Ok(Self {
            ptr,
            layout,
            len: capacity,
        })
    }

    pub fn as_slice(&self) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

impl<T> Drop for PinnedBuffer<T> {
    fn drop(&mut self) {
        unsafe {
            cuda_sys::cudaFreeHost(self.ptr as *mut std::ffi::c_void);
        }
    }
}

// Use pinned buffers in OnnxEmbeddingProvider
pub struct OnnxEmbeddingProvider {
    session: Arc<Session>,
    tokenizer: Arc<Tokenizer>,
    config: OnnxConfig,

    // NEW: Pre-allocated pinned buffers
    input_buffer: Arc<Mutex<PinnedBuffer<i64>>>,  // input_ids + attention_mask
    output_buffer: Arc<Mutex<PinnedBuffer<f32>>>, // embeddings
}
```

**Success Criteria:**
- ✅ Pinned memory allocator implemented
- ✅ Input/output buffers pre-allocated
- ✅ Memory copy latency reduced (5ms → ~1ms)

#### 3.2: CUDA Streams for Overlap (3 hours)

**Background:** CUDA streams allow overlapping CPU and GPU operations (tokenization, memory copy, inference).

```rust
// Add CUDA stream management
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

    pub fn get(&self, idx: usize) -> cudaStream_t {
        self.streams[idx % self.streams.len()]
    }
}

// Pipelined inference with streams
impl OnnxEmbeddingProvider {
    pub async fn embed_batch_pipelined(
        &self,
        texts: Vec<String>,
    ) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let stream_pool = self.stream_pool.lock().await;

        // Stream 0: Tokenization (CPU)
        let stream0 = stream_pool.get(0);
        let tokens = tokio::task::spawn_blocking(move || {
            self.tokenize_batch(texts)
        }).await?;

        // Stream 1: H2D copy (CPU → GPU)
        let stream1 = stream_pool.get(1);
        let input_tensor = self.copy_to_device(tokens, stream1)?;

        // Stream 2: Inference (GPU)
        let stream2 = stream_pool.get(2);
        let output_tensor = self.session.run_with_stream(input_tensor, stream2)?;

        // Stream 3: D2H copy (GPU → CPU)
        let stream3 = stream_pool.get(3);
        let embeddings = self.copy_to_host(output_tensor, stream3)?;

        // Synchronize all streams
        for stream in &stream_pool.streams {
            unsafe { cudaStreamSynchronize(*stream); }
        }

        Ok(embeddings)
    }
}
```

**Success Criteria:**
- ✅ 4 CUDA streams created (tokenization, H2D, inference, D2H)
- ✅ Pipelined inference implemented
- ✅ Latency reduced by overlapping operations (~10-15% improvement)

#### 3.3: Buffer Pooling (2 hours)

**Background:** Reusing pre-allocated buffers avoids allocation overhead and fragmentation.

```rust
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

        if let Some(idx) = available.pop() {
            Some(self.buffers[idx].clone())
        } else {
            None  // Pool exhausted
        }
    }

    pub async fn release(&self, buffer: Arc<Mutex<PinnedBuffer<f32>>>) {
        let mut available = self.available.lock().await;

        // Find buffer index
        for (idx, buf) in self.buffers.iter().enumerate() {
            if Arc::ptr_eq(buf, &buffer) {
                available.push(idx);
                break;
            }
        }
    }
}
```

**Success Criteria:**
- ✅ Buffer pool implemented (8 buffers for concurrent requests)
- ✅ Acquire/release mechanism tested
- ✅ Allocation overhead eliminated

**Estimated Time (Day 3):** 7 hours (2h pinned memory + 3h CUDA streams + 2h pooling)

---

### Day 4: Dynamic Batching (Thursday)

**Objective:** Implement dynamic batching to increase throughput from ~15 QPS to >50 QPS.

#### 4.1: Batching Queue (2 hours)

**Background:** Collect incoming requests into batches for efficient GPU utilization.

```rust
use tokio::sync::mpsc;
use std::time::{Duration, Instant};

pub struct BatchingQueue {
    sender: mpsc::UnboundedSender<BatchRequest>,
    receiver: Arc<Mutex<mpsc::UnboundedReceiver<BatchRequest>>>,
    config: BatchingConfig,
}

pub struct BatchingConfig {
    pub max_batch_size: usize,    // Maximum batch size (e.g., 32)
    pub batch_timeout: Duration,  // Timeout to dispatch batch (e.g., 5ms)
    pub max_queue_size: usize,    // Queue depth limit (backpressure)
}

pub struct BatchRequest {
    pub texts: Vec<String>,
    pub response_tx: oneshot::Sender<Result<Vec<Vec<f32>>, EmbeddingError>>,
}

impl BatchingQueue {
    pub fn new(config: BatchingConfig) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();

        Self {
            sender,
            receiver: Arc::new(Mutex::new(receiver)),
            config,
        }
    }

    pub async fn submit(&self, request: BatchRequest) -> Result<(), EmbeddingError> {
        self.sender.send(request)
            .map_err(|_| EmbeddingError::Internal("Queue closed".to_string()))
    }

    pub async fn collect_batch(&self) -> Vec<BatchRequest> {
        let mut batch = Vec::new();
        let deadline = Instant::now() + self.config.batch_timeout;

        let mut receiver = self.receiver.lock().await;

        // Collect requests until timeout or max_batch_size
        loop {
            let timeout = deadline.saturating_duration_since(Instant::now());

            match tokio::time::timeout(timeout, receiver.recv()).await {
                Ok(Some(request)) => {
                    batch.push(request);

                    if batch.len() >= self.config.max_batch_size {
                        break;  // Batch full
                    }
                }
                Ok(None) => break,  // Channel closed
                Err(_) => break,    // Timeout
            }
        }

        batch
    }
}
```

**Success Criteria:**
- ✅ Batching queue implemented
- ✅ Timeout-based dispatch working (5ms timeout)
- ✅ Max batch size enforcement (32 requests)

#### 4.2: Batch Processor (3 hours)

**Background:** Process collected batches with the ONNX provider.

```rust
pub struct BatchProcessor {
    provider: Arc<OnnxEmbeddingProvider>,
    queue: Arc<BatchingQueue>,
    metrics: Arc<BatchMetrics>,
}

impl BatchProcessor {
    pub fn new(provider: Arc<OnnxEmbeddingProvider>, config: BatchingConfig) -> Self {
        Self {
            provider,
            queue: Arc::new(BatchingQueue::new(config)),
            metrics: Arc::new(BatchMetrics::default()),
        }
    }

    pub async fn start(&self) {
        loop {
            // Collect batch from queue
            let requests = self.queue.collect_batch().await;

            if requests.is_empty() {
                continue;
            }

            // Merge texts from all requests
            let texts: Vec<String> = requests.iter()
                .flat_map(|req| req.texts.clone())
                .collect();

            let batch_size = texts.len();
            let start = Instant::now();

            // Process batch with ONNX provider
            match self.provider.embed_batch_internal(texts).await {
                Ok(embeddings) => {
                    // Split embeddings back to original requests
                    let mut offset = 0;
                    for request in requests {
                        let count = request.texts.len();
                        let batch_embeddings = embeddings[offset..offset + count].to_vec();
                        offset += count;

                        let _ = request.response_tx.send(Ok(batch_embeddings));
                    }

                    // Update metrics
                    let duration = start.elapsed();
                    self.metrics.record_batch(batch_size, duration);
                }
                Err(err) => {
                    // Send error to all requests
                    for request in requests {
                        let _ = request.response_tx.send(Err(err.clone()));
                    }
                }
            }
        }
    }

    pub async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let (tx, rx) = oneshot::channel();

        let request = BatchRequest {
            texts,
            response_tx: tx,
        };

        self.queue.submit(request).await?;

        rx.await
            .map_err(|_| EmbeddingError::Internal("Response channel closed".to_string()))?
    }
}
```

**Success Criteria:**
- ✅ Batch processor implemented
- ✅ Request merging and splitting working
- ✅ Error handling for batch failures

#### 4.3: Metrics & Monitoring (2 hours)

**Background:** Track batching metrics for observability.

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

    pub fn report(&self) -> String {
        format!(
            "Batches: {}, Requests: {}, Avg batch size: {}, Avg latency: {}μs",
            self.total_batches.load(Ordering::Relaxed),
            self.total_requests.load(Ordering::Relaxed),
            self.avg_batch_size.load(Ordering::Relaxed),
            self.avg_latency_us.load(Ordering::Relaxed),
        )
    }
}
```

**Success Criteria:**
- ✅ Metrics collection implemented
- ✅ Batch size, latency, throughput tracked
- ✅ Metrics exported (Prometheus format)

**Estimated Time (Day 4):** 7 hours (2h queue + 3h processor + 2h metrics)

---

### Day 5: Integration Testing & Documentation (Friday)

**Objective:** Validate all optimizations and document Week 3 results.

#### 5.1: End-to-End Performance Testing (3 hours)

```bash
# Create comprehensive performance test
cat > ~/akidb2/scripts/week3_performance_test.sh << 'EOF'
#!/bin/bash
set -e

echo "🚀 Week 3 Performance Testing"
echo "=============================="
echo

# Test 1: Single embedding latency
echo "📊 Test 1: Single Embedding Latency (P95 <30ms target)"
cargo test -p akidb-embedding --features onnx --release \
  --test qwen3_integration_test test_single_embedding -- --nocapture 2>&1 | \
  grep -E "(Duration:|Latency:)" | tee -a /tmp/week3_results.txt

# Test 2: Batch embedding throughput
echo
echo "📊 Test 2: Batch Embedding Throughput (>50 QPS target)"
cargo test -p akidb-embedding --features onnx --release \
  --test qwen3_integration_test test_large_batch -- --nocapture 2>&1 | \
  grep -E "(Throughput:|QPS:)" | tee -a /tmp/week3_results.txt

# Test 3: Concurrent requests (4 threads)
echo
echo "📊 Test 3: Concurrent Requests (>150 QPS target)"
cargo test -p akidb-embedding --features onnx --release \
  --test qwen3_integration_test test_concurrent_requests -- --nocapture 2>&1 | \
  grep -E "(Total QPS:|Throughput:)" | tee -a /tmp/week3_results.txt

# Test 4: Dynamic batching effectiveness
echo
echo "📊 Test 4: Dynamic Batching (batch size distribution)"
cargo test -p akidb-embedding --features onnx --release \
  --test qwen3_integration_test test_dynamic_batching -- --nocapture 2>&1 | \
  grep -E "(Avg batch:|Batches:)" | tee -a /tmp/week3_results.txt

# Test 5: Memory usage (GPU)
echo
echo "📊 Test 5: GPU Memory Usage (<4GB target)"
nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits | \
  tee -a /tmp/week3_results.txt

echo
echo "✅ Week 3 performance testing complete!"
echo "Results saved to /tmp/week3_results.txt"
EOF

chmod +x ~/akidb2/scripts/week3_performance_test.sh
./week3_performance_test.sh
```

**Success Criteria:**
- ✅ All 5 performance tests pass
- ✅ P95 latency <30ms achieved
- ✅ Throughput >50 QPS achieved
- ✅ GPU memory <4GB confirmed

#### 5.2: Create Week 3 Completion Report (2 hours)

```bash
cat > ~/akidb2/automatosx/tmp/JETSON-THOR-WEEK3-COMPLETION-REPORT.md << 'EOF'
# Jetson Thor Week 3: Performance Optimization - Completion Report

**Date:** $(date +%Y-%m-%d)
**Status:** ✅ COMPLETE / 🚧 IN PROGRESS / ❌ FAILED
**Duration:** 5 days

---

## Executive Summary

Week 3 focused on performance optimization to achieve production-grade latency and throughput on Jetson Thor. Starting from Week 2 baseline of ~80ms P95 latency and ~15 QPS throughput, we implemented TensorRT optimization, memory optimizations, and dynamic batching.

**Final Results:**
- Latency: [INSERT]ms P95 (target: <30ms, baseline: ~80ms)
- Throughput: [INSERT] QPS (target: >50 QPS, baseline: ~15 QPS)
- GPU Memory: [INSERT]GB (target: <4GB)

**Status:** [✅ PASS / ❌ FAIL]

---

## Achievements

### Day 1: TensorRT Profile Optimization
- ✅ FP8 Tensor Cores enabled and verified
- ✅ 3 optimization profiles created (low_latency, balanced, high_throughput)
- ✅ Best profile identified: [INSERT]
- ✅ Latency improvement: 80ms → [INSERT]ms ([INSERT]x faster)

### Day 2: TensorRT Tactics & Workspace Tuning
- ✅ Tactic sources tested: CUBLAS, CUDNN, EDGE_MASK_CONVOLUTIONS
- ✅ Optimal tactic configuration: [INSERT]
- ✅ Workspace size tuned: [INSERT]MB (from 512MB)
- ✅ Nsight Systems profiling completed

### Day 3: Memory & Copy Optimization
- ✅ Pinned memory allocator implemented
- ✅ CUDA streams for operation overlap (4 streams)
- ✅ Buffer pooling implemented (8 buffers)
- ✅ Memory copy overhead reduced: 5ms → [INSERT]ms

### Day 4: Dynamic Batching
- ✅ Batching queue with timeout-based dispatch
- ✅ Batch processor with request merging
- ✅ Metrics collection (batch size, latency, throughput)
- ✅ Throughput improvement: 15 QPS → [INSERT] QPS

### Day 5: Integration Testing
- ✅ End-to-end performance tests passing
- ✅ All optimization targets validated
- ✅ Documentation complete

---

## Performance Results

### Latency Comparison

| Metric | Week 2 Baseline | Week 3 Optimized | Improvement | Target | Status |
|--------|-----------------|------------------|-------------|--------|--------|
| Single (P50) | ~60ms | [INSERT]ms | [INSERT]x | - | - |
| Single (P95) | ~80ms | [INSERT]ms | [INSERT]x | <30ms | [INSERT] |
| Single (P99) | ~95ms | [INSERT]ms | [INSERT]x | <40ms | [INSERT] |
| Batch 8 (P95) | ~200ms | [INSERT]ms | [INSERT]x | <50ms | [INSERT] |

### Throughput Comparison

| Metric | Week 2 Baseline | Week 3 Optimized | Improvement | Target | Status |
|--------|-----------------|------------------|-------------|--------|--------|
| Single-threaded | ~15 QPS | [INSERT] QPS | [INSERT]x | >50 QPS | [INSERT] |
| Concurrent (4 threads) | ~40 QPS | [INSERT] QPS | [INSERT]x | >150 QPS | [INSERT] |

### Resource Usage

| Metric | Week 2 Baseline | Week 3 Optimized | Change | Target | Status |
|--------|-----------------|------------------|--------|--------|--------|
| GPU Memory | ~3.5GB | [INSERT]GB | [INSERT] | <4GB | [INSERT] |
| GPU Utilization | ~40% | [INSERT]% | +[INSERT]% | >80% | [INSERT] |

---

## Optimization Breakdown

### TensorRT Optimization (40ms → [INSERT]ms)
- FP8 Tensor Core usage: [✅ / ❌]
- Optimization profile: [INSERT]
- Tactic sources: [INSERT]
- Workspace size: [INSERT]MB

### Memory Optimization (5ms → [INSERT]ms)
- Pinned memory: [✅ / ❌]
- CUDA streams: [INSERT] streams
- Buffer pooling: [INSERT] buffers

### Dynamic Batching (+[INSERT]% throughput)
- Avg batch size: [INSERT]
- Batch timeout: 5ms
- Max batch size: 32

---

## Quality Validation

### Embedding Quality (vs HuggingFace baseline)
- Cosine similarity: [INSERT] (target: >0.99)
- Semantic similarity tests: [PASS / FAIL]
- L2 normalization: [PASS / FAIL]

### Stability Tests
- 1-hour soak test: [PASS / FAIL]
- Concurrent stress test: [PASS / FAIL]
- Memory leak detection: [PASS / FAIL]

---

## Next Steps (Week 4)

1. **Multi-Model Support** (5 days)
   - Support E5, BGE, Instructor models
   - Runtime model selection API
   - Model registry with LRU cache

2. **API Server Integration** (Week 5)
   - REST API endpoints
   - gRPC service integration
   - Load testing at scale

3. **Production Deployment** (Week 6)
   - Kubernetes Helm charts
   - Docker images for Jetson Thor
   - Production monitoring

---

## Files & Artifacts

**Performance Reports:**
- Criterion benchmarks: `target/criterion/report/index.html`
- Week 3 test results: `/tmp/week3_results.txt`
- Nsight Systems profile: `/tmp/qwen3_profile.nsys-rep`

**Code Changes:**
- TensorRT configuration: `crates/akidb-embedding/src/onnx.rs`
- Pinned memory: `crates/akidb-embedding/src/memory.rs`
- Dynamic batching: `crates/akidb-embedding/src/batching.rs`

**Documentation:**
- Optimization techniques: `docs/TENSORRT-OPTIMIZATION.md`
- Performance tuning guide: `docs/PERFORMANCE-TUNING-GUIDE.md`

---

**Report Prepared By:** [YOUR_NAME]
**Report Date:** $(date +%Y-%m-%d)
**Project:** AkiDB 2.0 - Jetson Thor Performance Optimization
EOF
```

#### 5.3: Create Performance Tuning Guide (2 hours)

```bash
cat > ~/akidb2/docs/JETSON-THOR-PERFORMANCE-TUNING-GUIDE.md << 'EOF'
# Jetson Thor Performance Tuning Guide

**Target Audience:** DevOps engineers, SREs, performance engineers
**Prerequisites:** Jetson Thor with CUDA 12.2+, TensorRT 9.0+
**AkiDB Version:** 2.0.0+

---

## Quick Reference

**TL;DR - Optimal Configuration:**

```rust
let config = OnnxConfig {
    model_path: PathBuf::from("/opt/akidb/models/qwen3-4b-onnx-fp8/model.onnx"),
    tokenizer_path: PathBuf::from("/opt/akidb/models/qwen3-4b-onnx-fp8/tokenizer.json"),
    model_name: "Qwen/Qwen2.5-4B".to_string(),
    dimension: 4096,
    max_length: 512,
    execution_provider: ExecutionProviderConfig::TensorRT {
        device_id: 0,
        fp8_enable: true,
        engine_cache_path: Some(PathBuf::from("/var/cache/akidb/trt")),
        workspace_size: 2_000_000_000,  // 2GB
        tactic_sources: "CUBLAS,CUDNN,EDGE_MASK_CONVOLUTIONS".to_string(),
        builder_opt_level: 5,  // Max optimization
        profile: "low_latency".to_string(),  // For <30ms P95
    },
};
```

**Expected Performance:**
- Latency: <30ms P95 (single embedding)
- Throughput: >50 QPS (single-threaded), >150 QPS (concurrent)
- GPU Memory: <4GB

---

## Configuration Parameters

### TensorRT Execution Provider

| Parameter | Values | Default | Recommendation |
|-----------|--------|---------|----------------|
| `fp8_enable` | true/false | true | ✅ true (use FP8 Tensor Cores) |
| `workspace_size` | bytes | 2GB | ✅ 2GB (optimal for Qwen3 4B) |
| `builder_opt_level` | 0-5 | 3 | ✅ 5 (max optimization) |
| `profile` | string | "default" | ✅ "low_latency" for <30ms |
| `tactic_sources` | string | "CUBLAS,CUDNN" | ✅ "CUBLAS,CUDNN,EDGE_MASK_CONVOLUTIONS" |
| `engine_cache_path` | path | /tmp | ✅ /var/cache/akidb/trt (persistent) |

### Dynamic Batching

| Parameter | Values | Default | Recommendation |
|-----------|--------|---------|----------------|
| `max_batch_size` | 1-128 | 32 | ✅ 32 (balance latency/throughput) |
| `batch_timeout` | ms | 5ms | ✅ 5ms (low latency priority) |
| `max_queue_size` | int | 1000 | ✅ 500 (prevent OOM) |

---

## Performance Optimization Checklist

### Before Deployment
- [ ] TensorRT engine built with FP8 enabled
- [ ] Optimization profile selected (low_latency for <30ms)
- [ ] Workspace size set to 2GB
- [ ] Engine cache path configured (persistent directory)
- [ ] Pinned memory enabled
- [ ] CUDA streams configured (4 streams)
- [ ] Dynamic batching enabled with 5ms timeout

### Monitoring
- [ ] GPU utilization >80% during load
- [ ] P95 latency <30ms for single embeddings
- [ ] Throughput >50 QPS (single-threaded)
- [ ] GPU memory usage <4GB
- [ ] No memory leaks after 1-hour soak test

---

## Troubleshooting

### Issue 1: P95 Latency >30ms

**Symptoms:** Latency exceeds target despite optimization.

**Debug Steps:**
1. Check TensorRT profile: `ls /var/cache/akidb/trt/`
2. Verify FP8 usage: `nsys profile --trace=cuda ...` (look for fp8 kernels)
3. Check GPU utilization: `nvidia-smi` (should be >80%)
4. Profile with Nsight Systems (identify bottlenecks)

**Solutions:**
- Delete TensorRT cache and rebuild with optimal settings
- Use `low_latency` profile (not `balanced` or `high_throughput`)
- Increase workspace size to 4GB (if memory allows)
- Verify Jetson Thor has latest JetPack (6.0+)

### Issue 2: Low Throughput (<50 QPS)

**Symptoms:** Single-threaded QPS below target.

**Debug Steps:**
1. Check dynamic batching metrics: `curl http://localhost:8080/metrics | grep batch`
2. Verify avg batch size >4
3. Check queue wait time (should be <5ms)

**Solutions:**
- Increase batch timeout from 5ms to 10ms (trade latency for throughput)
- Increase max_batch_size to 64
- Run multiple instances (load balancing)

### Issue 3: GPU Memory Exhaustion

**Symptoms:** `cudaMalloc failed: out of memory`

**Debug Steps:**
1. Check GPU memory: `nvidia-smi`
2. Check TensorRT workspace size
3. Check batch size (larger batches = more memory)

**Solutions:**
- Reduce workspace size to 1GB
- Reduce max_batch_size to 16
- Enable gradient checkpointing (for larger models)
- Use model with fewer parameters (Qwen3 1.5B instead of 4B)

---

**Document Version:** 1.0
**Last Updated:** $(date +%Y-%m-%d)
**Maintainer:** Backend Team
EOF
```

**Success Criteria (Day 5):**
- ✅ End-to-end performance tests pass
- ✅ Week 3 completion report created
- ✅ Performance tuning guide documented

**Estimated Time (Day 5):** 7 hours (3h testing + 2h report + 2h guide)

---

## Success Criteria

### Week 3 Completion Checklist

**Performance Targets:**
- [ ] **P95 latency <30ms** for single embedding (batch size 1)
- [ ] **P95 latency <50ms** for batch size 8
- [ ] **Throughput >50 QPS** (single-threaded)
- [ ] **Throughput >150 QPS** (concurrent, 4 threads)
- [ ] **GPU memory <4GB** (model + workspace)

**Optimization Implementation:**
- [ ] FP8 Tensor Cores enabled and verified (Nsight profiling)
- [ ] 3 TensorRT optimization profiles created (low_latency, balanced, high_throughput)
- [ ] Optimal profile identified and benchmarked
- [ ] Pinned memory allocator implemented
- [ ] 4 CUDA streams for operation overlap
- [ ] Buffer pooling (8 buffers)
- [ ] Dynamic batching with timeout-based dispatch

**Quality Validation:**
- [ ] Embedding quality maintained (cosine similarity >0.99 vs HuggingFace)
- [ ] No memory leaks (1-hour soak test)
- [ ] Stable under concurrent load (4-8 threads)

**Documentation:**
- [ ] Week 3 completion report created
- [ ] Performance tuning guide documented
- [ ] Optimization techniques explained
- [ ] Troubleshooting guide created

### Success Metrics

| Metric | Week 2 Baseline | Week 3 Target | Status |
|--------|-----------------|---------------|--------|
| **Latency (P95, batch=1)** | ~80ms | <30ms | TBD |
| **Latency (P95, batch=8)** | ~200ms | <50ms | TBD |
| **Throughput (single)** | ~15 QPS | >50 QPS | TBD |
| **Throughput (concurrent)** | ~40 QPS | >150 QPS | TBD |
| **GPU Memory** | ~3.5GB | <4GB | TBD |
| **GPU Utilization** | ~40% | >80% | TBD |
| **Quality (cosine sim)** | >0.99 | >0.99 | TBD |

---

## Risk Management

### High Risks

**Risk 1: FP8 Tensor Cores Not Used** 🔴 **HIGH**
- **Impact:** Latency remains ~40ms (can't reach <30ms target)
- **Probability:** Medium
- **Mitigation:**
  - Verify FP8 kernels in Nsight Systems profile
  - Check TensorRT verbose logs for precision fallback warnings
  - Test with explicit FP8 ONNX model export
  - Contact NVIDIA support if issues persist

**Risk 2: Dynamic Batching Increases Latency** 🟡 **MEDIUM**
- **Impact:** P95 latency increases due to batch wait time
- **Probability:** Medium
- **Mitigation:**
  - Use short timeout (5ms) for low-latency priority
  - Adaptive batching based on load
  - Priority queue for urgent requests (bypass batching)

### Medium Risks

**Risk 3: Memory Optimization Complexity** 🟡 **MEDIUM**
- **Impact:** Pinned memory/CUDA streams introduce bugs
- **Probability:** Medium
- **Mitigation:**
  - Incremental implementation (test each optimization separately)
  - Extensive testing with memory sanitizers
  - Fallback to baseline if issues occur

**Risk 4: Performance Regression** 🟢 **LOW**
- **Impact:** Some optimizations may degrade performance
- **Probability:** Low
- **Mitigation:**
  - Benchmark after each optimization
  - Keep Week 2 baseline for comparison
  - Rollback individual optimizations if needed

---

## Appendix: Code Examples

### Example 1: Optimal TensorRT Configuration

```rust
use akidb_embedding::{OnnxConfig, ExecutionProviderConfig};
use std::path::PathBuf;

let config = OnnxConfig {
    model_path: PathBuf::from("/opt/akidb/models/qwen3-4b-onnx-fp8/model.onnx"),
    tokenizer_path: PathBuf::from("/opt/akidb/models/qwen3-4b-onnx-fp8/tokenizer.json"),
    model_name: "Qwen/Qwen2.5-4B".to_string(),
    dimension: 4096,
    max_length: 512,
    execution_provider: ExecutionProviderConfig::TensorRT {
        device_id: 0,
        fp8_enable: true,
        engine_cache_path: Some(PathBuf::from("/var/cache/akidb/trt")),
        workspace_size: 2_000_000_000,  // 2GB
        tactic_sources: "CUBLAS,CUDNN,EDGE_MASK_CONVOLUTIONS".to_string(),
        builder_opt_level: 5,
        profile: "low_latency".to_string(),
    },
};

let provider = OnnxEmbeddingProvider::with_config(config).await?;
```

### Example 2: Dynamic Batching Usage

```rust
use akidb_embedding::{BatchProcessor, BatchingConfig};
use std::time::Duration;

let batching_config = BatchingConfig {
    max_batch_size: 32,
    batch_timeout: Duration::from_millis(5),  // 5ms timeout
    max_queue_size: 500,
};

let batch_processor = BatchProcessor::new(
    Arc::new(provider),
    batching_config
);

// Start batch processing loop (background task)
tokio::spawn(async move {
    batch_processor.start().await;
});

// Submit embedding request (will be batched automatically)
let embeddings = batch_processor.embed(vec!["Test text".to_string()]).await?;
```

### Example 3: Nsight Systems Profiling

```bash
#!/bin/bash
# Profile ONNX inference with Nsight Systems

nsys profile \
  --trace=cuda,nvtx,osrt \
  --cuda-memory-usage=true \
  --output=/tmp/qwen3_optimized.nsys-rep \
  --force-overwrite=true \
  --stats=true \
  cargo test -p akidb-embedding --features onnx --release \
    --test qwen3_integration_test test_single_embedding -- --nocapture

# Analyze profile
nsys-ui /tmp/qwen3_optimized.nsys-rep &

# Export statistics
nsys stats /tmp/qwen3_optimized.nsys-rep --report cuda_gpu_kern_sum
nsys stats /tmp/qwen3_optimized.nsys-rep --report cuda_api_sum
```

---

**PRD Version:** 1.0
**Last Updated:** $(date +%Y-%m-%d)
**Next Review:** End of Week 3

---

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>
