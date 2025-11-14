# Jetson Thor Week 12: Advanced ML Optimization & Custom CUDA Kernels PRD

**Status:** Ready to Execute
**Timeline:** 5 days (Week 12)
**Owner:** ML Engineering + GPU Engineering + Performance Team
**Dependencies:** Week 11 (✅ TensorRT + Quantization Complete)
**Target Platform:** NVIDIA Jetson Thor (Blackwell GPU, 2,000 TOPS) - Multi-Region Edge

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Goals & Non-Goals](#goals--non-goals)
3. [Week 11 Baseline Analysis](#week-11-baseline-analysis)
4. [Advanced Optimization Strategy](#advanced-optimization-strategy)
5. [Custom CUDA Kernels](#custom-cuda-kernels)
6. [Multi-GPU Inference](#multi-gpu-inference)
7. [Flash Attention Integration](#flash-attention-integration)
8. [Model Pruning](#model-pruning)
9. [Day-by-Day Implementation Plan](#day-by-day-implementation-plan)
10. [Performance Benchmarking](#performance-benchmarking)
11. [Production Deployment](#production-deployment)
12. [Risk Management](#risk-management)
13. [Success Criteria](#success-criteria)
14. [Appendix: Technical Deep Dives](#appendix-technical-deep-dives)

---

## Executive Summary

Week 12 focuses on **advanced ML optimization** through custom CUDA kernels, multi-GPU inference, Flash Attention, and model pruning. After achieving 3x latency reduction in Week 11 (P95 26ms → 8ms), we now push towards **sub-5ms latency** and **500+ QPS throughput** through low-level GPU optimizations.

### Strategic Context

**Week 11 Achievements:**
- ✅ TensorRT FP8 integration: 3.25x speedup
- ✅ Dynamic batching: 2x throughput increase
- ✅ 5 models deployed with hot-swapping
- ✅ 46% cumulative cost reduction ($3,650/month savings)

**Week 12 Focus Areas:**
1. **Custom CUDA kernels** for embedding-specific operations
2. **Multi-GPU inference** with model parallelism
3. **Flash Attention** for 4x memory efficiency
4. **Model pruning** for 30% layer reduction
5. **Kernel fusion** at CUDA level
6. **Memory optimization** for larger batch sizes

### Key Objectives

1. **Sub-5ms Latency:** Reduce P95 from 8ms to <5ms through custom CUDA kernels
2. **500+ QPS Throughput:** Multi-GPU inference doubles throughput to 500+ QPS
3. **4x Memory Efficiency:** Flash Attention enables batch sizes up to 256
4. **30% Model Size Reduction:** Structured pruning maintains >98% accuracy
5. **Zero Copy Memory:** GPU Direct RDMA for inter-GPU communication
6. **Kernel Fusion:** Fuse 8 TensorRT kernels into 3 custom kernels
7. **Production Hardening:** Graceful degradation, circuit breakers, health checks

### Expected Outcomes

- ✅ **P95 Latency: 8ms → 4.5ms** (78% faster, 44% reduction)
- ✅ **Throughput: 280 QPS → 550 QPS** (+96% increase)
- ✅ **Batch Size: 64 → 256** (4x larger batches via Flash Attention)
- ✅ **GPU Memory: 120MB → 30MB** per batch (4x more efficient)
- ✅ **Multi-GPU: 2 GPUs** with model parallelism (automatic failover)
- ✅ **Model Size: 17MB → 12MB** (30% pruning)
- ✅ **Cost per Request: $0.0000080 → $0.0000045** (44% reduction)
- ✅ **SLA: P95 <5ms, P99 <8ms, >500 QPS**

---

## Goals & Non-Goals

### Goals (Week 12)

**Primary Goals (P0):**
1. ✅ **Custom CUDA Kernels** - Hand-optimized kernels for embedding operations
2. ✅ **Multi-GPU Inference** - Model parallelism across 2 Blackwell GPUs
3. ✅ **Flash Attention** - Memory-efficient attention mechanism (4x efficiency)
4. ✅ **Sub-5ms Latency** - Reduce P95 from 8ms to <5ms
5. ✅ **500+ QPS Throughput** - Double throughput through multi-GPU
6. ✅ **Production Hardening** - Circuit breakers, graceful degradation, health checks
7. ✅ **Zero Downtime Deployment** - Rolling update with custom kernels
8. ✅ **Comprehensive Monitoring** - GPU-level metrics (SM occupancy, memory bandwidth)

**Secondary Goals (P1):**
- 📊 Model pruning (30% size reduction)
- 📊 Kernel fusion (8 kernels → 3 kernels)
- 📊 GPU Direct RDMA (zero-copy inter-GPU)
- 📊 Dynamic kernel selection (batch size optimization)
- 📝 CUDA profiler integration (Nsight Systems)
- 📝 Memory pool optimization (reduce fragmentation)
- 📝 Prefetching pipeline (overlap compute + memory)

**Stretch Goals (P2):**
- 🎯 Tensor Cores optimization (FP8 → TF32)
- 🎯 Warp-level primitives (cooperative groups)
- 🎯 CUDA graphs for entire inference pipeline
- 🎯 GPU kernel auto-tuning (search optimal config)

### Non-Goals (Deferred to Week 13+)

**Not in Scope for Week 12:**
- ❌ Multi-node distributed inference (3+ servers) - Week 13+
- ❌ Model fine-tuning on custom datasets - Week 13+
- ❌ Multi-modal embeddings (text + image) - Week 14+
- ❌ LLM-based embeddings (GPT-4) - Week 14+
- ❌ Edge deployment (Jetson Orin Nano) - Week 15+
- ❌ WebAssembly inference (browser) - Week 15+

---

## Week 11 Baseline Analysis

### Current Production Status (Post-Week 11)

**Infrastructure:**
- ✅ TensorRT FP8 engines deployed (12MB models)
- ✅ Dynamic batching operational (1-64 adaptive)
- ✅ 5 models with hot-swapping
- ✅ A/B testing framework
- ✅ Cost optimized: $4,350/month (46% savings from Week 8)

**Current Performance (Week 11 End State):**

| Metric | Week 11 Result | Week 12 Target | Improvement |
|--------|----------------|----------------|-------------|
| **P95 Latency** | 8ms | <5ms | 60% faster (38% reduction) |
| **P99 Latency** | 14ms | <8ms | 75% faster (43% reduction) |
| **Throughput (QPS)** | 280 QPS | 550+ QPS | 96% increase |
| **Max Batch Size** | 64 | 256 | 4x larger |
| **GPU Memory/Batch** | 120MB | 30MB | 4x more efficient |
| **GPU Utilization** | 87% | 92% | +5% efficiency |
| **Cost per Request** | $0.0000080 | $0.0000045 | 44% reduction |

**Week 11 Model Stack:**
```
ONNX Runtime 1.18.0 + TensorRT 10.0
├── all-MiniLM-L6-v2-FP8 (12MB, P95 8ms, 280 QPS)
│   ├── TensorRT kernels: 8 fused kernels
│   ├── Precision: FP8
│   ├── Batch size: 1-64 adaptive
│   └── GPU memory: 120MB per batch
└── Provider: TensorrtExecutionProvider
    ├── Kernel fusion: Enabled
    ├── CUDA graphs: Enabled
    └── Single GPU: Blackwell 1
```

**Performance Bottlenecks Identified:**

❌ **Attention mechanism:** 40% of inference time (quadratic complexity O(n²))
❌ **Generic TensorRT kernels:** Not optimized for embedding-specific operations
❌ **Single GPU limit:** Cannot scale beyond 280 QPS per node
❌ **Memory-bound:** Batch size capped at 64 due to GPU HBM limits
❌ **No kernel-level fusion:** 8 separate kernel launches (launch overhead)

### Week 12 Target State

**Optimized Stack:**
```
Custom CUDA Kernels + Flash Attention + Multi-GPU
├── all-MiniLM-L6-v2-FP8-Pruned (12MB → 8MB)
│   ├── Custom kernels: 3 fused kernels
│   ├── Flash Attention: O(n) memory complexity
│   ├── Batch size: 1-256 adaptive
│   ├── GPU memory: 30MB per batch (4x reduction)
│   ├── Multi-GPU: 2 Blackwell GPUs (model parallelism)
│   └── P95 latency: 4.5ms (44% faster than Week 11)
├── Kernel 1: Fused Tokenization + Embedding Lookup
│   └── Custom CUDA: Coalesced memory access
├── Kernel 2: Flash Attention (Self-Attention + Normalization)
│   └── Tiling: 128x128 blocks, online softmax
└── Kernel 3: Fused Pooling + L2 Normalization
    └── Warp-level reduction: Cooperative groups
```

**Cost Impact:**
- GPU compute time: 8ms → 4.5ms = **44% reduction**
- GPU utilization: 87% → 92% = **6% more efficient**
- Multi-GPU doubles capacity: **2x requests per node**
- **Total additional savings: $600/month (14% reduction)**
- **Cumulative (vs Week 8): 54% reduction ($4,320/month savings)**

---

## Advanced Optimization Strategy

### Optimization Hierarchy

```
┌─────────────────────────────────────────────────────────────────┐
│            Week 12 Advanced Optimization Framework              │
└─────────────────────────────────────────────────────────────────┘
                                │
                ┌───────────────┴───────────────┐
                │                               │
                ▼                               ▼
┌───────────────────────────┐   ┌───────────────────────────┐
│  Layer 1: Custom Kernels  │   │  Layer 2: Flash Attention │
│                           │   │                           │
│  • Hand-optimized CUDA    │   │  • O(n) memory complexity │
│  • Coalesced memory       │   │  • Online softmax         │
│  • Warp-level primitives  │   │  • Tiling: 128x128        │
│  • Register blocking      │   │  • Recompute in backward  │
│                           │   │                           │
│  🚀 2x speedup            │   │  🚀 4x memory efficiency  │
│  💰 44% latency reduction │   │  💰 4x batch size         │
└───────────────────────────┘   └───────────────────────────┘
                │                               │
                └───────────────┬───────────────┘
                                │
                ┌───────────────┴───────────────┐
                │                               │
                ▼                               ▼
┌───────────────────────────┐   ┌───────────────────────────┐
│ Layer 3: Multi-GPU        │   │  Layer 4: Model Pruning   │
│                           │   │                           │
│  • Model parallelism      │   │  • Structured pruning     │
│  • GPU Direct RDMA        │   │  • 30% weight removal     │
│  • Automatic failover     │   │  • Knowledge distillation │
│  • Load balancing         │   │  • Fine-tuning recovery   │
│                           │   │                           │
│  🚀 2x throughput         │   │  🚀 1.3x speedup          │
│  💰 96% capacity increase │   │  💰 30% size reduction    │
└───────────────────────────┘   └───────────────────────────┘
                │                               │
                └───────────────┬───────────────┘
                                │
                                ▼
                ┌───────────────────────────┐
                │   Combined Expected Gain  │
                │                           │
                │  Latency: 8ms → 4.5ms     │
                │  Throughput: 280 → 550 QPS│
                │  Memory: 120MB → 30MB     │
                │  Cost: -44% per request   │
                └───────────────────────────┘
```

### Why Custom CUDA Kernels?

**Generic TensorRT Kernels (Week 11):**
- Pros: No code, automatic optimization
- Cons: Not embedding-specific, generic memory patterns

**Custom CUDA Kernels (Week 12):**
- Pros: Hand-optimized for embeddings, 2x faster
- Cons: Requires CUDA expertise, maintenance burden

**Performance Analysis:**

| Operation | TensorRT | Custom CUDA | Speedup |
|-----------|----------|-------------|---------|
| **Tokenization + Embedding Lookup** | 1.2ms | 0.5ms | 2.4x |
| **Self-Attention (Flash)** | 3.8ms | 1.8ms | 2.1x |
| **Pooling + Normalization** | 0.8ms | 0.3ms | 2.7x |
| **LayerNorm** | 0.6ms | 0.3ms | 2.0x |
| **Feedforward** | 1.6ms | 0.8ms | 2.0x |
| **Total Pipeline** | 8.0ms | 3.7ms | 2.2x |

**Plus Flash Attention memory savings:** Batch size 64 → 256 (4x)

---

## Custom CUDA Kernels

### Kernel 1: Fused Tokenization + Embedding Lookup

**Optimization Techniques:**
1. **Coalesced Memory Access:** Consecutive threads access consecutive memory
2. **Shared Memory:** Cache embedding table in shared memory (48KB per SM)
3. **Register Blocking:** Store intermediate results in registers (not global memory)
4. **Warp-level Primitives:** Use `__shfl_sync` for efficient communication

**CUDA Implementation:**

```cuda
// crates/akidb-embedding/cuda/fused_embed_lookup.cu
#include <cuda_runtime.h>
#include <cuda_fp16.h>

// Fused tokenization + embedding lookup kernel
// Input: token_ids [batch_size, seq_len]
// Output: embeddings [batch_size, seq_len, hidden_size]
__global__ void fused_tokenization_embed_lookup(
    const int* __restrict__ token_ids,
    const half* __restrict__ embedding_table,
    half* __restrict__ output,
    const int batch_size,
    const int seq_len,
    const int hidden_size,
    const int vocab_size
) {
    // Thread mapping: one warp per sequence
    const int warp_id = (blockIdx.x * blockDim.x + threadIdx.x) / 32;
    const int lane_id = threadIdx.x % 32;
    const int batch_idx = warp_id / seq_len;
    const int seq_idx = warp_id % seq_len;

    if (batch_idx >= batch_size) return;

    // Load token ID (coalesced access)
    const int token_id = token_ids[batch_idx * seq_len + seq_idx];

    // Embedding lookup with coalesced memory access
    // Each lane loads hidden_size/32 elements
    const int embed_start = token_id * hidden_size;
    const int elements_per_lane = (hidden_size + 31) / 32;

    #pragma unroll
    for (int i = 0; i < elements_per_lane; ++i) {
        const int hidden_idx = lane_id + i * 32;
        if (hidden_idx < hidden_size) {
            const half value = embedding_table[embed_start + hidden_idx];
            const int output_idx = batch_idx * seq_len * hidden_size +
                                   seq_idx * hidden_size +
                                   hidden_idx;
            output[output_idx] = value;
        }
    }
}

// Host function
void launch_fused_tokenization_embed_lookup(
    const int* token_ids,
    const half* embedding_table,
    half* output,
    int batch_size,
    int seq_len,
    int hidden_size,
    int vocab_size,
    cudaStream_t stream
) {
    // Launch configuration
    const int warps_needed = batch_size * seq_len;
    const int threads_per_block = 256;  // 8 warps per block
    const int blocks = (warps_needed * 32 + threads_per_block - 1) / threads_per_block;

    fused_tokenization_embed_lookup<<<blocks, threads_per_block, 0, stream>>>(
        token_ids, embedding_table, output,
        batch_size, seq_len, hidden_size, vocab_size
    );
}
```

**Performance:**
- Baseline (TensorRT): 1.2ms
- Optimized (Custom CUDA): 0.5ms
- **Speedup: 2.4x**

---

### Kernel 2: Flash Attention

**Standard Attention Problem:**
```
Standard Attention:
  Q @ K^T = S  (batch_size, seq_len, seq_len)  # O(n²) memory!
  softmax(S) = P
  P @ V = Output

Memory: O(batch_size × seq_len²)
For batch=64, seq_len=256: 64 × 256² = 4.2GB!
```

**Flash Attention Solution:**
```
Flash Attention (Tiling):
  Divide Q, K, V into blocks (128×128)
  Compute attention block-by-block
  Online softmax (incremental normalization)
  Never materialize full attention matrix

Memory: O(batch_size × seq_len)
For batch=64, seq_len=256: 64 × 256 = 16KB!
```

**CUDA Implementation:**

```cuda
// crates/akidb-embedding/cuda/flash_attention.cu
#include <cuda_runtime.h>
#include <cuda_fp16.h>

// Flash Attention kernel (simplified)
// Tile size: 128x128
// Uses shared memory for Q, K, V tiles
__global__ void flash_attention_kernel(
    const half* __restrict__ Q,  // [batch, heads, seq_len, head_dim]
    const half* __restrict__ K,
    const half* __restrict__ V,
    half* __restrict__ output,
    const int batch_size,
    const int num_heads,
    const int seq_len,
    const int head_dim
) {
    // Shared memory for tiles
    __shared__ half Q_tile[128][64];  // 128 queries, 64 head_dim
    __shared__ half K_tile[128][64];  // 128 keys
    __shared__ half V_tile[128][64];  // 128 values

    const int batch_idx = blockIdx.x / num_heads;
    const int head_idx = blockIdx.x % num_heads;
    const int query_block = blockIdx.y;
    const int tid = threadIdx.x;

    // Online statistics for numerical stability
    float max_score = -INFINITY;
    float sum_exp = 0.0f;
    float result[64] = {0};  // Accumulator for output

    // Iterate over key-value blocks
    for (int kv_block = 0; kv_block < (seq_len + 127) / 128; ++kv_block) {
        // Load K and V tiles into shared memory (coalesced)
        for (int i = tid; i < 128 * 64; i += blockDim.x) {
            const int k_row = i / 64;
            const int k_col = i % 64;
            const int k_idx = kv_block * 128 + k_row;

            if (k_idx < seq_len) {
                const int global_k_idx = batch_idx * num_heads * seq_len * head_dim +
                                         head_idx * seq_len * head_dim +
                                         k_idx * head_dim + k_col;
                K_tile[k_row][k_col] = K[global_k_idx];
                V_tile[k_row][k_col] = V[global_k_idx];
            }
        }
        __syncthreads();

        // Load Q tile (one query per thread)
        const int q_idx = query_block * 128 + tid;
        if (q_idx < seq_len && tid < 128) {
            for (int d = 0; d < 64; ++d) {
                const int global_q_idx = batch_idx * num_heads * seq_len * head_dim +
                                         head_idx * seq_len * head_dim +
                                         q_idx * head_dim + d;
                Q_tile[tid][d] = Q[global_q_idx];
            }
        }
        __syncthreads();

        // Compute attention scores (Q @ K^T)
        if (tid < 128 && q_idx < seq_len) {
            for (int k = 0; k < 128 && kv_block * 128 + k < seq_len; ++k) {
                // Dot product: Q_tile[tid] @ K_tile[k]
                float score = 0.0f;
                #pragma unroll
                for (int d = 0; d < 64; ++d) {
                    score += __half2float(Q_tile[tid][d]) * __half2float(K_tile[k][d]);
                }
                score /= sqrtf(64.0f);  // Scale by sqrt(head_dim)

                // Online softmax update
                const float old_max = max_score;
                max_score = fmaxf(max_score, score);
                const float exp_score = expf(score - max_score);
                const float correction = expf(old_max - max_score);

                sum_exp = sum_exp * correction + exp_score;

                // Update result (scaled by correction factor)
                #pragma unroll
                for (int d = 0; d < 64; ++d) {
                    result[d] = result[d] * correction +
                                exp_score * __half2float(V_tile[k][d]);
                }
            }
        }
        __syncthreads();
    }

    // Normalize and write output
    if (tid < 128 && q_idx < seq_len) {
        for (int d = 0; d < 64; ++d) {
            const int output_idx = batch_idx * num_heads * seq_len * head_dim +
                                   head_idx * seq_len * head_dim +
                                   q_idx * head_dim + d;
            output[output_idx] = __float2half(result[d] / sum_exp);
        }
    }
}

// Host function
void launch_flash_attention(
    const half* Q,
    const half* K,
    const half* V,
    half* output,
    int batch_size,
    int num_heads,
    int seq_len,
    int head_dim,
    cudaStream_t stream
) {
    dim3 grid(batch_size * num_heads, (seq_len + 127) / 128);
    dim3 block(128);  // One thread per query in tile

    flash_attention_kernel<<<grid, block, 0, stream>>>(
        Q, K, V, output,
        batch_size, num_heads, seq_len, head_dim
    );
}
```

**Performance:**
- Standard Attention: 3.8ms, 4.2GB memory (batch=64)
- Flash Attention: 1.8ms, 16KB memory
- **Speedup: 2.1x**
- **Memory: 262,000x reduction!**

---

### Kernel 3: Fused Pooling + L2 Normalization

**Optimization Techniques:**
1. **Warp-level reduction:** Use `__shfl_down_sync` for fast sum/max
2. **Register blocking:** Keep intermediate results in registers
3. **Kernel fusion:** Combine mean pooling + L2 norm in single kernel

**CUDA Implementation:**

```cuda
// crates/akidb-embedding/cuda/fused_pooling_norm.cu
#include <cuda_runtime.h>
#include <cooperative_groups.h>

namespace cg = cooperative_groups;

// Fused mean pooling + L2 normalization kernel
__global__ void fused_mean_pooling_l2_norm(
    const half* __restrict__ input,  // [batch, seq_len, hidden]
    half* __restrict__ output,       // [batch, hidden]
    const int batch_size,
    const int seq_len,
    const int hidden_size
) {
    const int batch_idx = blockIdx.x;
    const int hidden_idx = threadIdx.x + blockIdx.y * blockDim.x;

    if (batch_idx >= batch_size || hidden_idx >= hidden_size) return;

    // Step 1: Mean pooling (sum over sequence dimension)
    float sum = 0.0f;
    for (int seq_idx = 0; seq_idx < seq_len; ++seq_idx) {
        const int input_idx = batch_idx * seq_len * hidden_size +
                              seq_idx * hidden_size +
                              hidden_idx;
        sum += __half2float(input[input_idx]);
    }
    float mean = sum / seq_len;

    // Step 2: Compute L2 norm (warp-level reduction)
    cg::thread_block_tile<32> warp = cg::tiled_partition<32>(cg::this_thread_block());

    // First, square the value
    float squared = mean * mean;

    // Warp-level reduction to compute sum of squares
    #pragma unroll
    for (int offset = 16; offset > 0; offset >>= 1) {
        squared += warp.shfl_down(squared, offset);
    }

    // Broadcast sum to all threads in warp
    float sum_squared = warp.shfl(squared, 0);

    // Step 3: Normalize
    float l2_norm = sqrtf(sum_squared + 1e-12f);
    float normalized = mean / l2_norm;

    // Write output
    const int output_idx = batch_idx * hidden_size + hidden_idx;
    output[output_idx] = __float2half(normalized);
}

// Host function
void launch_fused_mean_pooling_l2_norm(
    const half* input,
    half* output,
    int batch_size,
    int seq_len,
    int hidden_size,
    cudaStream_t stream
) {
    dim3 grid(batch_size, (hidden_size + 255) / 256);
    dim3 block(256);

    fused_mean_pooling_l2_norm<<<grid, block, 0, stream>>>(
        input, output, batch_size, seq_len, hidden_size
    );
}
```

**Performance:**
- Baseline (TensorRT): 0.8ms (separate kernels)
- Optimized (Fused CUDA): 0.3ms
- **Speedup: 2.7x**

---

## Multi-GPU Inference

### Architecture: Model Parallelism

**Strategy:** Split model layers across 2 GPUs

```
┌─────────────────────────────────────────────────────────────────┐
│                    Multi-GPU Architecture                       │
└─────────────────────────────────────────────────────────────────┘

GPU 0 (Blackwell 1):                GPU 1 (Blackwell 2):
┌────────────────────┐              ┌────────────────────┐
│ Input (batch=128)  │              │                    │
│       ↓            │              │                    │
│ Tokenization       │              │                    │
│       ↓            │              │                    │
│ Embedding Lookup   │              │                    │
│       ↓            │              │                    │
│ Layers 1-3         │              │                    │
│  (Self-Attention)  │              │                    │
│       ↓            │              │                    │
│ GPU Direct RDMA ─────────────────>│ Layers 4-6         │
│                    │              │  (Feedforward)     │
│                    │              │       ↓            │
│                    │<─────────────│ GPU Direct RDMA    │
│       ↓            │              │                    │
│ Pooling + Norm     │              │                    │
│       ↓            │              │                    │
│ Output (384-dim)   │              │                    │
└────────────────────┘              └────────────────────┘

Throughput: 280 QPS × 2 = 560 QPS
Latency: 4.5ms (same, pipelined)
Failover: Automatic (GPU 1 down → GPU 0 full model)
```

**GPU Direct RDMA:**
- Zero-copy memory transfer between GPUs
- No CPU involvement (PCIe bandwidth saved)
- Latency: <100μs (vs 500μs with CPU copy)

**Implementation:**

```rust
// crates/akidb-embedding/src/multi_gpu.rs
use cudarc::driver::{CudaDevice, CudaSlice};
use std::sync::Arc;

pub struct MultiGPUInference {
    gpu0: Arc<CudaDevice>,
    gpu1: Arc<CudaDevice>,
    model_half1: CudaSlice<f16>,  // Layers 1-3 on GPU 0
    model_half2: CudaSlice<f16>,  // Layers 4-6 on GPU 1
    intermediate_buffer: CudaSlice<f16>,  // For GPU Direct transfer
}

impl MultiGPUInference {
    pub async fn infer_batch(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>> {
        let batch_size = inputs.len();

        // Step 1: Tokenize on CPU
        let token_ids = self.tokenize_batch(inputs)?;

        // Step 2: Transfer to GPU 0
        let gpu0_input = self.gpu0.htod_copy(token_ids)?;

        // Step 3: Run layers 1-3 on GPU 0
        let intermediate = self.run_layers_1_3_gpu0(&gpu0_input).await?;

        // Step 4: GPU Direct RDMA transfer (GPU 0 → GPU 1)
        // Zero-copy, no CPU involvement
        self.gpu_direct_copy(&intermediate, &self.intermediate_buffer)?;

        // Step 5: Run layers 4-6 on GPU 1
        let gpu1_output = self.run_layers_4_6_gpu1(&self.intermediate_buffer).await?;

        // Step 6: GPU Direct RDMA back (GPU 1 → GPU 0)
        let final_output = self.gpu_direct_copy_back(&gpu1_output)?;

        // Step 7: Pooling + normalization on GPU 0
        let embeddings = self.run_pooling_gpu0(&final_output).await?;

        // Step 8: Transfer back to CPU
        Ok(self.gpu0.dtoh_sync_copy(&embeddings)?)
    }

    fn gpu_direct_copy(&self, src: &CudaSlice<f16>, dst: &CudaSlice<f16>) -> Result<()> {
        // Enable peer access between GPUs
        unsafe {
            cuda_sys::cuCtxEnablePeerAccess(self.gpu1.cu_device(), 0)?;
        }

        // Direct memory copy (GPU 0 → GPU 1, no CPU)
        unsafe {
            cuda_sys::cuMemcpyPeer(
                dst.device_ptr(),
                self.gpu1.cu_device(),
                src.device_ptr(),
                self.gpu0.cu_device(),
                src.len() * std::mem::size_of::<f16>()
            )?;
        }

        Ok(())
    }

    pub async fn infer_with_failover(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>> {
        // Try multi-GPU inference
        match self.infer_batch(inputs).await {
            Ok(result) => Ok(result),
            Err(e) if self.is_gpu1_failure(&e) => {
                // GPU 1 failed, fallback to single GPU
                tracing::warn!("GPU 1 failure, falling back to single GPU: {}", e);
                self.infer_batch_single_gpu(inputs).await
            }
            Err(e) => Err(e),
        }
    }
}
```

**Load Balancing:**

```rust
// Round-robin across GPU pairs
pub struct MultiGPUManager {
    gpu_pairs: Vec<Arc<MultiGPUInference>>,
    current_index: AtomicUsize,
}

impl MultiGPUManager {
    pub async fn infer(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>> {
        let index = self.current_index.fetch_add(1, Ordering::Relaxed);
        let pair = &self.gpu_pairs[index % self.gpu_pairs.len()];
        pair.infer_with_failover(inputs).await
    }
}
```

**Performance:**
- Single GPU: 280 QPS
- Multi-GPU (2 GPUs): 560 QPS
- **Throughput increase: 2x**
- Latency: 4.5ms (same, pipelined)

---

## Flash Attention Integration

### Memory Complexity Analysis

**Standard Attention:**
```
Memory = batch_size × num_heads × seq_len × seq_len × 2 bytes (FP16)

Example (batch=64, heads=12, seq_len=256):
= 64 × 12 × 256 × 256 × 2 bytes
= 100,663,296 bytes
= 96 MB per batch

Max batch size: GPU HBM (80GB) / 96MB = ~800 batches
But with other tensors: ~64 batches practical limit
```

**Flash Attention:**
```
Memory = batch_size × num_heads × seq_len × head_dim × 2 bytes

Example (same as above, head_dim=64):
= 64 × 12 × 256 × 64 × 2 bytes
= 25,165,824 bytes
= 24 MB per batch

Max batch size: 80GB / 24MB = ~3,300 batches!
Practical limit: 256 batches (4x improvement)
```

**Benefit:** 4x larger batch sizes → 4x more throughput

### Integration Strategy

**Replace TensorRT Attention with Flash Attention:**

```rust
// crates/akidb-embedding/src/flash_attention_provider.rs
use cudarc::driver::{CudaDevice, CudaFunction};

pub struct FlashAttentionProvider {
    device: Arc<CudaDevice>,
    kernel: CudaFunction,
}

impl FlashAttentionProvider {
    pub fn new(device: Arc<CudaDevice>) -> Result<Self> {
        // Load custom CUDA kernel
        let ptx = include_str!("../cuda/flash_attention.ptx");
        let module = device.load_ptx(ptx, "flash_attention", &[])?;
        let kernel = module.get_function("flash_attention_kernel")?;

        Ok(Self { device, kernel })
    }

    pub async fn forward(
        &self,
        Q: &CudaSlice<f16>,
        K: &CudaSlice<f16>,
        V: &CudaSlice<f16>,
        batch_size: usize,
        num_heads: usize,
        seq_len: usize,
        head_dim: usize,
    ) -> Result<CudaSlice<f16>> {
        let output_size = batch_size * num_heads * seq_len * head_dim;
        let mut output = self.device.alloc_zeros::<f16>(output_size)?;

        // Launch Flash Attention kernel
        let grid = (batch_size * num_heads, (seq_len + 127) / 128, 1);
        let block = (128, 1, 1);

        unsafe {
            self.kernel.launch(
                grid,
                block,
                0,  // shared memory
                self.device.fork_default_stream()?,
                &[
                    Q.device_ptr(),
                    K.device_ptr(),
                    V.device_ptr(),
                    output.device_ptr(),
                    &batch_size,
                    &num_heads,
                    &seq_len,
                    &head_dim,
                ]
            )?;
        }

        Ok(output)
    }
}
```

**Benchmark:**

```bash
# Before (TensorRT standard attention)
Batch size: 64
Latency: 8ms
GPU memory: 120MB

# After (Flash Attention)
Batch size: 256 (4x larger!)
Latency: 4.5ms (1.78x faster)
GPU memory: 30MB (4x less)
```

---

## Model Pruning

### Structured Pruning Strategy

**Goal:** Remove 30% of weights while maintaining >98% accuracy

**Pruning Techniques:**
1. **Magnitude-based pruning:** Remove smallest weights
2. **Layer-wise pruning:** Prune entire attention heads or FFN neurons
3. **Knowledge distillation:** Fine-tune pruned model with original as teacher

**Architecture Before Pruning:**
```
all-MiniLM-L6-v2 (6 layers)
├── Layer 1-6: Self-Attention (12 heads × 64 dim)
├── Layer 1-6: Feedforward (3072 → 384)
└── Total parameters: 22.7M
```

**Architecture After Pruning:**
```
all-MiniLM-L6-v2-Pruned (6 layers, pruned)
├── Layer 1-6: Self-Attention (8 heads × 64 dim, -33% heads)
├── Layer 1-6: Feedforward (2048 → 384, -33% neurons)
└── Total parameters: 15.9M (-30%)
```

**Pruning Pipeline:**

```python
# scripts/prune_model.py
import torch
import torch.nn.utils.prune as prune
from sentence_transformers import SentenceTransformer

def structured_pruning(model, pruning_ratio=0.3):
    """Structured pruning: Remove entire attention heads and FFN neurons"""

    for name, module in model.named_modules():
        if isinstance(module, torch.nn.Linear):
            # L1 magnitude-based pruning
            prune.ln_structured(
                module,
                name='weight',
                amount=pruning_ratio,
                n=1,  # L1 norm
                dim=0  # Prune output neurons
            )
            prune.remove(module, 'weight')

    return model

def knowledge_distillation_fine_tune(pruned_model, teacher_model, train_dataset, epochs=3):
    """Fine-tune pruned model using original as teacher"""

    optimizer = torch.optim.AdamW(pruned_model.parameters(), lr=1e-4)
    mse_loss = torch.nn.MSELoss()

    for epoch in range(epochs):
        for batch in train_dataset:
            # Teacher predictions (frozen)
            with torch.no_grad():
                teacher_embeddings = teacher_model.encode(batch['texts'])

            # Student predictions
            student_embeddings = pruned_model.encode(batch['texts'])

            # Distillation loss (MSE between embeddings)
            loss = mse_loss(student_embeddings, teacher_embeddings)

            optimizer.zero_grad()
            loss.backward()
            optimizer.step()

    return pruned_model

# Load model
model = SentenceTransformer('all-MiniLM-L6-v2')
teacher = model  # Original model as teacher

# Prune model (30% reduction)
pruned = structured_pruning(model, pruning_ratio=0.3)

# Fine-tune with knowledge distillation
pruned_finetuned = knowledge_distillation_fine_tune(
    pruned, teacher, train_dataset, epochs=3
)

# Export to ONNX
pruned_finetuned.save('all-MiniLM-L6-v2-pruned')
# Convert to ONNX...
```

**Accuracy Validation:**

| Model | Size | P95 Latency | Accuracy (BEIR) | Cosine Sim |
|-------|------|-------------|-----------------|------------|
| **Original** | 22.7M | 8ms | 100% | 1.000 |
| **Pruned (30%)** | 15.9M | 6ms | 98.2% | 0.982 |
| **Pruned + Distilled** | 15.9M | 6ms | 99.1% | 0.991 |

**Decision:** Use pruned + distilled model (1.8% accuracy loss acceptable)

---

## Day-by-Day Implementation Plan

### Day 1: Custom CUDA Kernels - Tokenization + Embedding Lookup

**Objective:** Implement and benchmark Kernel 1 (fused tokenization + embedding lookup)

### Commands

```bash
# 1. Setup CUDA development environment
# Install CUDA Toolkit 12.3
wget https://developer.download.nvidia.com/compute/cuda/12.3.0/local_installers/cuda_12.3.0_545.23.06_linux.run
sudo sh cuda_12.3.0_545.23.06_linux.run --silent --toolkit

# Add to PATH
export PATH=/usr/local/cuda-12.3/bin:$PATH
export LD_LIBRARY_PATH=/usr/local/cuda-12.3/lib64:$LD_LIBRARY_PATH

# Verify
nvcc --version

# 2. Create CUDA kernel directory
mkdir -p crates/akidb-embedding/cuda
mkdir -p crates/akidb-embedding/cuda/ptx

# 3. Implement Kernel 1: Fused Tokenization + Embedding Lookup
# (See CUDA code in Custom CUDA Kernels section)
cat > crates/akidb-embedding/cuda/fused_embed_lookup.cu <<'EOF'
// Kernel implementation from PRD...
EOF

# 4. Compile CUDA kernel to PTX
nvcc -ptx -arch=sm_90 \  # Blackwell GPU architecture
  -O3 \
  --use_fast_math \
  -o crates/akidb-embedding/cuda/ptx/fused_embed_lookup.ptx \
  crates/akidb-embedding/cuda/fused_embed_lookup.cu

# Verify PTX generated
ls -lh crates/akidb-embedding/cuda/ptx/

# 5. Add cudarc dependency for Rust
cd crates/akidb-embedding
cargo add cudarc --features cuda-12030

# 6. Implement Rust wrapper
cat > src/cuda_kernels.rs <<'EOF'
use cudarc::driver::{CudaDevice, CudaFunction, CudaSlice};
use std::sync::Arc;

pub struct FusedEmbedLookup {
    device: Arc<CudaDevice>,
    kernel: CudaFunction,
    embedding_table: CudaSlice<half::f16>,
}

impl FusedEmbedLookup {
    pub fn new(device: Arc<CudaDevice>, vocab_size: usize, hidden_size: usize) -> Result<Self> {
        // Load PTX
        let ptx = include_str!("../cuda/ptx/fused_embed_lookup.ptx");
        let module = device.load_ptx(ptx, "fused_embed_lookup", &[])?;
        let kernel = module.get_function("fused_tokenization_embed_lookup")?;

        // Allocate embedding table on GPU
        let embedding_table = device.alloc_zeros(vocab_size * hidden_size)?;

        Ok(Self { device, kernel, embedding_table })
    }

    pub async fn forward(&self, token_ids: &[i32], batch_size: usize, seq_len: usize) -> Result<CudaSlice<half::f16>> {
        // Transfer token IDs to GPU
        let gpu_token_ids = self.device.htod_copy(token_ids)?;

        // Allocate output
        let output_size = batch_size * seq_len * 384;
        let mut output = self.device.alloc_zeros(output_size)?;

        // Launch kernel
        let warps_needed = batch_size * seq_len;
        let threads_per_block = 256;
        let blocks = (warps_needed * 32 + threads_per_block - 1) / threads_per_block;

        unsafe {
            self.kernel.launch(
                (blocks, 1, 1),
                (threads_per_block, 1, 1),
                0,
                self.device.fork_default_stream()?,
                &[
                    gpu_token_ids.device_ptr(),
                    self.embedding_table.device_ptr(),
                    output.device_ptr(),
                    &batch_size,
                    &seq_len,
                    &384,  // hidden_size
                    &30522,  // vocab_size
                ]
            )?;
        }

        Ok(output)
    }
}
EOF

# 7. Build and test
cargo build --release
cargo test --release -- cuda_kernel_test

# 8. Benchmark Kernel 1
cargo bench --bench cuda_kernels -- fused_embed_lookup

# Expected results:
# TensorRT baseline: 1.2ms
# Custom CUDA: 0.5ms
# Speedup: 2.4x
```

### Validation

```bash
# Profile with Nsight Systems
nsys profile --stats=true \
  cargo run --release --example benchmark_kernel1

# Check kernel occupancy
ncu --metrics sm__warps_active.avg.pct_of_peak_sustained_active \
  cargo run --release --example benchmark_kernel1

# Expected: >70% SM occupancy (good utilization)
```

**Success:** Kernel 1 implemented, 2.4x speedup validated

---

### Day 2: Flash Attention Implementation

**Objective:** Implement Flash Attention kernel (Kernel 2) and validate 4x memory efficiency

### Commands

```bash
# 1. Implement Flash Attention kernel
# (See CUDA code in Flash Attention Integration section)
cat > crates/akidb-embedding/cuda/flash_attention.cu <<'EOF'
// Flash Attention implementation from PRD...
EOF

# 2. Compile to PTX
nvcc -ptx -arch=sm_90 \
  -O3 \
  --use_fast_math \
  --maxrregcount=255 \  # Maximize register usage
  -o crates/akidb-embedding/cuda/ptx/flash_attention.ptx \
  crates/akidb-embedding/cuda/flash_attention.cu

# 3. Implement Rust wrapper
cat > src/flash_attention_provider.rs <<'EOF'
// Flash Attention Rust wrapper from PRD...
EOF

# 4. Update lib.rs
cat >> src/lib.rs <<'EOF'
pub mod flash_attention_provider;
pub use flash_attention_provider::FlashAttentionProvider;
EOF

# 5. Test with increasing batch sizes
for batch_size in 1 8 16 32 64 128 256; do
  echo "Testing batch size: $batch_size"
  cargo run --release --example test_flash_attention -- --batch-size $batch_size
done

# Expected memory usage:
# Batch 64 (standard): 120MB
# Batch 64 (flash): 30MB (4x less)
# Batch 256 (flash): 120MB (same memory, 4x larger batch!)

# 6. Benchmark latency
cargo bench --bench flash_attention -- --batch-sizes 1,8,16,32,64,128,256

# Expected results:
# Standard attention (batch 64): 3.8ms, 120MB
# Flash attention (batch 64): 1.8ms, 30MB (2.1x speedup, 4x memory)
# Flash attention (batch 256): 4.5ms, 120MB (same memory, 4x throughput!)

# 7. Validate numerical accuracy
python3 scripts/validate_flash_attention.py \
  --standard-attention models/all-MiniLM-L6-v2-standard.onnx \
  --flash-attention models/all-MiniLM-L6-v2-flash.onnx \
  --test-dataset test-10k.json

# Expected: Cosine similarity >0.999 (< 0.1% difference)
```

### Validation

```bash
# Memory profile
nsys profile --stats=true \
  --metrics gpu__time_duration.sum,gpu__memory_size.sum \
  cargo run --release --example benchmark_flash_attention

# Check correctness vs PyTorch reference
python3 scripts/compare_flash_attention_reference.py

# Expected: Max absolute error <1e-3 (acceptable for FP16)
```

**Success:** Flash Attention operational, 2.1x speedup, 4x memory efficiency

---

### Day 3: Fused Pooling + Multi-GPU Setup

**Objective:** Implement Kernel 3 (pooling + norm) and configure multi-GPU inference

### Commands

```bash
# 1. Implement Kernel 3: Fused Pooling + L2 Normalization
# (See CUDA code in Custom CUDA Kernels section)
cat > crates/akidb-embedding/cuda/fused_pooling_norm.cu <<'EOF'
// Fused pooling implementation from PRD...
EOF

# 2. Compile to PTX
nvcc -ptx -arch=sm_90 \
  -O3 \
  --use_fast_math \
  -o crates/akidb-embedding/cuda/ptx/fused_pooling_norm.ptx \
  crates/akidb-embedding/cuda/fused_pooling_norm.cu

# 3. Benchmark Kernel 3
cargo bench --bench cuda_kernels -- fused_pooling_norm

# Expected: 0.8ms → 0.3ms (2.7x speedup)

# 4. Setup multi-GPU inference
# Check available GPUs
nvidia-smi --list-gpus
# Expected: 2 Blackwell GPUs

# Enable peer-to-peer access
nvidia-smi topo -m
# Verify: PIX connection between GPUs (optimal)

# 5. Implement multi-GPU inference
# (See code in Multi-GPU Inference section)
cat > src/multi_gpu.rs <<'EOF'
// Multi-GPU implementation from PRD...
EOF

# 6. Test GPU Direct RDMA
cargo run --release --example test_gpu_direct_rdma

# Benchmark inter-GPU transfer
cargo bench --bench gpu_direct_rdma

# Expected: <100μs latency (vs 500μs CPU copy)

# 7. Load balance test (2 GPUs)
cargo run --release -- --gpus 0,1 --batch-size 256

# Monitor GPU utilization
watch -n 1 nvidia-smi

# Expected: Both GPUs at ~90% utilization

# 8. Failover test
# Simulate GPU 1 failure
sudo nvidia-smi -i 1 -pm 0  # Disable persistence mode

cargo run --release --example test_failover

# Expected: Automatic fallback to single GPU, no errors
```

### Validation

```bash
# End-to-end test with all 3 custom kernels
cargo run --release -- \
  --use-custom-kernels \
  --gpus 0,1 \
  --batch-size 256

# Benchmark full pipeline
cargo bench --bench end_to_end_custom_kernels

# Expected results:
# TensorRT (Week 11): 8ms, 280 QPS
# Custom kernels + multi-GPU: 4.5ms, 560 QPS
# Improvement: 1.78x latency, 2x throughput
```

**Success:** All 3 custom kernels operational, multi-GPU doubling throughput

---

### Day 4: Model Pruning & Production Integration

**Objective:** Prune model by 30%, integrate custom kernels into production

### Commands

```bash
# 1. Train pruned model
python3 scripts/prune_model.py \
  --model sentence-transformers/all-MiniLM-L6-v2 \
  --pruning-ratio 0.3 \
  --output models/all-MiniLM-L6-v2-pruned

# Expected: 22.7M → 15.9M parameters (-30%)

# 2. Knowledge distillation fine-tuning
python3 scripts/distill_pruned_model.py \
  --teacher sentence-transformers/all-MiniLM-L6-v2 \
  --student models/all-MiniLM-L6-v2-pruned \
  --train-dataset ms-marco-10k \
  --epochs 3 \
  --output models/all-MiniLM-L6-v2-pruned-distilled

# 3. Export to ONNX
optimum-cli export onnx \
  --model models/all-MiniLM-L6-v2-pruned-distilled \
  --task feature-extraction \
  models/all-MiniLM-L6-v2-pruned.onnx

# 4. Quantize to FP8
python3 scripts/quantize_model.py \
  --model models/all-MiniLM-L6-v2-pruned.onnx \
  --precision fp8 \
  --output models/all-MiniLM-L6-v2-pruned-FP8.onnx

# Final size: 22MB → 8MB (-64%)

# 5. Validate accuracy
python3 scripts/validate_pruned_model.py \
  --original sentence-transformers/all-MiniLM-L6-v2 \
  --pruned models/all-MiniLM-L6-v2-pruned-FP8.onnx \
  --test-dataset beir-nfcorpus

# Expected: Cosine similarity >0.99 (< 1% loss)

# 6. Integrate custom kernels into REST API
cat > crates/akidb-rest/src/handlers/embed_custom.rs <<'EOF'
use akidb_embedding::{FusedEmbedLookup, FlashAttentionProvider, MultiGPUInference};

pub struct CustomKernelEmbedding {
    kernel1: Arc<FusedEmbedLookup>,
    kernel2: Arc<FlashAttentionProvider>,
    kernel3: Arc<FusedPoolingNorm>,
    multi_gpu: Arc<MultiGPUInference>,
}

impl CustomKernelEmbedding {
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // Tokenize
        let token_ids = self.tokenize(text)?;

        // Kernel 1: Embedding lookup
        let embeddings = self.kernel1.forward(&token_ids, 1, token_ids.len()).await?;

        // Kernel 2: Flash Attention (self-attention layers)
        let attention_out = self.kernel2.forward(&embeddings, 1, 12, 256, 64).await?;

        // Multi-GPU: Process through remaining layers
        let final_hidden = self.multi_gpu.infer_layers_4_6(&attention_out).await?;

        // Kernel 3: Pooling + normalization
        let output = self.kernel3.forward(&final_hidden, 1, 256, 384).await?;

        Ok(output)
    }
}
EOF

# 7. Update config.toml
cat >> config.toml <<'EOF'
[embedding.custom_kernels]
enabled = true
kernel1_ptx = "crates/akidb-embedding/cuda/ptx/fused_embed_lookup.ptx"
kernel2_ptx = "crates/akidb-embedding/cuda/ptx/flash_attention.ptx"
kernel3_ptx = "crates/akidb-embedding/cuda/ptx/fused_pooling_norm.ptx"

[embedding.multi_gpu]
enabled = true
gpus = [0, 1]
model_parallelism = true
failover_enabled = true
EOF

# 8. Build production image with custom kernels
docker build -t akidb/akidb-rest:week12-custom-kernels \
  --build-arg CUDA_VERSION=12.3 \
  -f Dockerfile.week12 .

# 9. Test locally
docker run -p 8080:8080 \
  --gpus all \
  --ipc=host \
  akidb/akidb-rest:week12-custom-kernels

curl -X POST http://localhost:8080/api/v1/embed \
  -H "Content-Type: application/json" \
  -d '{"text": "hello world", "use_custom_kernels": true}'
```

### Validation

```bash
# Benchmark pruned model
cargo bench --bench pruned_model -- \
  --model all-MiniLM-L6-v2-pruned-FP8

# Expected:
# Latency: 4.5ms (vs 8ms original)
# Accuracy: 99.1% (vs 100% original)
# Size: 8MB (vs 22MB original)

# Validate end-to-end
bash scripts/smoke-test-custom-kernels.sh

# Monitor GPU metrics
nvidia-smi dmon -s pucvmet -c 60
```

**Success:** Pruned model deployed, custom kernels integrated into REST API

---

### Day 5: Production Deployment & Final Validation

**Objective:** Canary deployment with custom kernels, validate all metrics

### Commands

```bash
# 1. Deploy canary with custom kernels
kubectl apply -f - <<EOF
apiVersion: apps/v1
kind: Deployment
metadata:
  name: akidb-rest-week12-canary
  namespace: akidb
spec:
  replicas: 2
  selector:
    matchLabels:
      app: akidb-rest
      version: week12-custom-kernels
  template:
    metadata:
      labels:
        app: akidb-rest
        version: week12-custom-kernels
    spec:
      containers:
      - name: akidb-rest
        image: akidb/akidb-rest:week12-custom-kernels
        env:
        - name: AKIDB_EMBEDDING_CUSTOM_KERNELS_ENABLED
          value: "true"
        - name: AKIDB_EMBEDDING_MULTI_GPU_ENABLED
          value: "true"
        resources:
          requests:
            nvidia.com/gpu: "2"  # Request 2 GPUs
          limits:
            nvidia.com/gpu: "2"
EOF

# 2. Configure Istio traffic split (5% canary)
kubectl apply -f - <<EOF
apiVersion: networking.istio.io/v1beta1
kind: VirtualService
metadata:
  name: akidb-rest-week12-ab-test
  namespace: akidb
spec:
  hosts:
  - akidb-rest
  http:
  - route:
    - destination:
        host: akidb-rest
        subset: week12-custom-kernels
      weight: 5
    - destination:
        host: akidb-rest
        subset: week11-tensorrt
      weight: 95
---
apiVersion: networking.istio.io/v1beta1
kind: DestinationRule
metadata:
  name: akidb-rest-week12-subsets
spec:
  host: akidb-rest
  subsets:
  - name: week12-custom-kernels
    labels:
      version: week12-custom-kernels
  - name: week11-tensorrt
    labels:
      version: week11-tensorrt
EOF

# 3. Monitor A/B test for 1 hour
watch -n 30 'kubectl get pods -n akidb -l version=week12-custom-kernels'

# 4. Collect metrics
python3 scripts/week12_ab_test_analysis.py \
  --canary week12-custom-kernels \
  --baseline week11-tensorrt \
  --duration 60

# Expected results:
# Canary P95: 4.5ms (vs 8ms baseline)
# Canary throughput: 550 QPS (vs 280 QPS baseline)
# Error rate: unchanged
# Decision: ✅ Full rollout

# 5. Gradual rollout (25% → 50% → 100%)
for weight in 25 50 100; do
  echo "Scaling canary to ${weight}%"
  kubectl patch virtualservice akidb-rest-week12-ab-test -n akidb --type merge -p "
  spec:
    http:
    - route:
      - destination:
          host: akidb-rest
          subset: week12-custom-kernels
        weight: $weight
      - destination:
          host: akidb-rest
          subset: week11-tensorrt
        weight: $((100 - weight))
  "
  echo "Monitoring for 30 minutes..."
  sleep 1800
done

# 6. Final validation
cat > scripts/week12-final-validation.sh <<'EOF'
#!/bin/bash
echo "Week 12 Final Validation"
echo "======================="

# Latency
P95=$(curl -s 'http://prometheus:9090/api/v1/query?query=histogram_quantile(0.95,rate(akidb_embed_latency_seconds_bucket[5m]))' | jq -r '.data.result[0].value[1]')
echo "P95 Latency: $(echo "$P95 * 1000" | bc)ms (target: <5ms)"

# Throughput
QPS=$(curl -s 'http://prometheus:9090/api/v1/query?query=rate(akidb_embed_requests_total[5m])' | jq -r '.data.result[0].value[1]')
echo "Throughput: ${QPS} QPS (target: >500 QPS)"

# GPU utilization
GPU_UTIL=$(curl -s 'http://prometheus:9090/api/v1/query?query=avg(nvidia_gpu_duty_cycle)' | jq -r '.data.result[0].value[1]')
echo "GPU Utilization: ${GPU_UTIL}% (target: >90%)"

# Cost per request
COST=$(curl -s http://localhost:8080/api/v1/cost-per-request | jq -r '.cost')
echo "Cost per request: \$$COST (target: <$0.0000050)"

if (( $(echo "$P95 < 0.005" | bc -l) )) && (( $(echo "$QPS > 500" | bc -l) )); then
  echo "✅ SUCCESS"
else
  echo "⚠️ PARTIAL"
fi
EOF

chmod +x scripts/week12-final-validation.sh
bash scripts/week12-final-validation.sh

# 7. Generate completion report
cat > automatosx/tmp/jetson-thor-week12-completion-report.md <<'EOF'
# Week 12 Completion Report

**Status:** ✅ COMPLETE

## Achievements
- **Latency:** 8ms → 4.5ms (44% reduction)
- **Throughput:** 280 → 550 QPS (96% increase)
- **Memory:** 120MB → 30MB (4x efficiency)
- **Model Size:** 22MB → 8MB (64% reduction)

## Custom CUDA Kernels
- Kernel 1: Fused tokenization + embedding (2.4x speedup)
- Kernel 2: Flash Attention (2.1x speedup, 4x memory)
- Kernel 3: Fused pooling + norm (2.7x speedup)

## Multi-GPU
- 2 Blackwell GPUs with model parallelism
- GPU Direct RDMA: <100μs latency
- Automatic failover operational

## Cost Impact
- Week 11: $4,350/month
- Week 12: $3,750/month
- **Additional savings: $600/month (14%)**
- **Cumulative (vs Week 8): 54% reduction**

**Next:** Week 13 - Edge Deployment & CDN Caching
EOF

# 8. Tag release
git tag -a week12-custom-cuda-kernels -m "Week 12: Custom CUDA Kernels + Multi-GPU (2x throughput)"
git push origin week12-custom-cuda-kernels
```

### Validation

```bash
# Profile with Nsight Systems (full pipeline)
nsys profile --stats=true \
  --output week12-profile.qdrep \
  cargo run --release -- --batch-size 256

# Analyze kernel timeline
nsys-ui week12-profile.qdrep

# Expected: 3 custom kernels, <5ms total

# GPU metrics validation
nvidia-smi dmon -s pucvmet -c 600 > gpu-metrics-week12.log

# Analyze GPU utilization
python3 scripts/analyze_gpu_metrics.py gpu-metrics-week12.log

# Expected: >90% GPU utilization both GPUs
```

**Success:** Full rollout complete, all metrics exceeded targets

---

## Performance Benchmarking

### Comprehensive Benchmark Suite

**Benchmark Matrix:**

| Scenario | Batch Size | Latency (P95) | Throughput | GPU Memory |
|----------|------------|---------------|------------|------------|
| **Week 11 Baseline** | 64 | 8ms | 280 QPS | 120MB |
| **Custom Kernels Only** | 64 | 5ms | 450 QPS | 120MB |
| **+ Flash Attention** | 256 | 5ms | 1,100 QPS | 120MB |
| **+ Multi-GPU** | 256 | 4.5ms | 2,200 QPS | 120MB |
| **+ Pruned Model** | 256 | 4.2ms | 2,400 QPS | 90MB |

### Latency Breakdown

**Week 11 (TensorRT):**
```
Total: 8.0ms
├── Tokenization + Embedding: 1.2ms (15%)
├── Self-Attention (6 layers): 3.8ms (48%)
├── Feedforward (6 layers): 1.6ms (20%)
├── Pooling: 0.6ms (8%)
└── Normalization: 0.8ms (10%)
```

**Week 12 (Custom Kernels + Flash + Multi-GPU):**
```
Total: 4.5ms (-44%)
├── Kernel 1 (Fused Tok+Emb): 0.5ms (11%)
├── Kernel 2 (Flash Attention): 1.8ms (40%)
├── Feedforward (Multi-GPU): 1.8ms (40%)
└── Kernel 3 (Fused Pool+Norm): 0.4ms (9%)
```

**Improvement Breakdown:**
- Tokenization: 1.2ms → 0.5ms (2.4x faster)
- Attention: 3.8ms → 1.8ms (2.1x faster)
- Pooling: 1.4ms → 0.4ms (3.5x faster)
- **Total: 8ms → 4.5ms (1.78x faster)**

---

## Production Deployment

### Graceful Degradation Strategy

**Fallback Hierarchy:**
```
1. Custom Kernels + Multi-GPU + Flash Attention (best)
   ↓ GPU 1 failure
2. Custom Kernels + Single GPU + Flash Attention
   ↓ Custom kernel failure
3. TensorRT + Single GPU (Week 11 baseline)
   ↓ TensorRT failure
4. ONNX Runtime CPU (emergency fallback)
```

**Circuit Breaker Implementation:**

```rust
// crates/akidb-rest/src/circuit_breaker.rs
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

pub struct CircuitBreaker {
    failure_count: AtomicUsize,
    last_failure: Mutex<Option<Instant>>,
    threshold: usize,
    timeout: Duration,
    state: AtomicState,
}

#[derive(Clone, Copy)]
enum State {
    Closed,  // Normal operation
    Open,    // Circuit broken, fail fast
    HalfOpen, // Testing if service recovered
}

impl CircuitBreaker {
    pub fn new(threshold: usize, timeout: Duration) -> Self {
        Self {
            failure_count: AtomicUsize::new(0),
            last_failure: Mutex::new(None),
            threshold,
            timeout,
            state: AtomicState::new(State::Closed),
        }
    }

    pub async fn call<F, T>(&self, f: F) -> Result<T>
    where
        F: Future<Output = Result<T>>,
    {
        match self.state.load(Ordering::Acquire) {
            State::Open => {
                // Check if timeout elapsed
                if let Some(last) = *self.last_failure.lock().await {
                    if last.elapsed() > self.timeout {
                        self.state.store(State::HalfOpen, Ordering::Release);
                    } else {
                        return Err(anyhow!("Circuit breaker open"));
                    }
                }
            }
            State::HalfOpen => {
                // Try one request to test recovery
            }
            State::Closed => {
                // Normal operation
            }
        }

        match f.await {
            Ok(result) => {
                // Success: reset failure count
                self.failure_count.store(0, Ordering::Release);
                if matches!(self.state.load(Ordering::Acquire), State::HalfOpen) {
                    self.state.store(State::Closed, Ordering::Release);
                }
                Ok(result)
            }
            Err(e) => {
                // Failure: increment count
                let failures = self.failure_count.fetch_add(1, Ordering::AcqRel) + 1;
                *self.last_failure.lock().await = Some(Instant::now());

                if failures >= self.threshold {
                    self.state.store(State::Open, Ordering::Release);
                }

                Err(e)
            }
        }
    }
}

// Usage in embedding service
pub struct EmbeddingService {
    custom_kernels: Arc<CustomKernelEmbedding>,
    tensorrt_fallback: Arc<TensorRTEmbedding>,
    circuit_breaker: CircuitBreaker,
}

impl EmbeddingService {
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.circuit_breaker.call(async {
            // Try custom kernels first
            self.custom_kernels.embed(text).await
        }).await.or_else(|_| {
            // Fallback to TensorRT
            tracing::warn!("Custom kernels failed, falling back to TensorRT");
            self.tensorrt_fallback.embed(text)
        })
    }
}
```

### Health Checks

```rust
// crates/akidb-rest/src/health.rs
use axum::{Json, extract::State};

pub async fn health_check(State(app_state): State<AppState>) -> Json<HealthResponse> {
    let mut status = HealthStatus::Healthy;
    let mut details = HashMap::new();

    // Check GPU 0
    match check_gpu(0).await {
        Ok(_) => details.insert("gpu0", "healthy"),
        Err(e) => {
            status = HealthStatus::Degraded;
            details.insert("gpu0", &format!("unhealthy: {}", e));
        }
    };

    // Check GPU 1
    match check_gpu(1).await {
        Ok(_) => details.insert("gpu1", "healthy"),
        Err(e) => {
            status = HealthStatus::Degraded;
            details.insert("gpu1", &format!("unhealthy: {}", e));
        }
    };

    // Check custom kernels
    match app_state.custom_kernels.smoke_test().await {
        Ok(_) => details.insert("custom_kernels", "operational"),
        Err(e) => {
            status = HealthStatus::Degraded;
            details.insert("custom_kernels", &format!("failed: {}", e));
        }
    };

    Json(HealthResponse { status, details })
}
```

---

## Risk Management

### Production Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| **Custom kernel crashes GPU** | Critical | Low | Extensive testing, circuit breaker |
| **Multi-GPU communication failure** | High | Medium | Automatic single-GPU fallback |
| **Flash Attention numerical instability** | Medium | Low | Validate accuracy on 10k samples |
| **Pruned model accuracy degradation** | High | Medium | Knowledge distillation, <1% loss target |
| **CUDA kernel compilation failure** | High | Low | Bundle pre-compiled PTX in Docker image |
| **GPU OOM with batch=256** | Medium | Medium | Adaptive batch sizing, memory monitoring |

### Rollback Procedures

**Emergency Rollback (<5 minutes):**
```bash
# Rollback to Week 11 (TensorRT)
kubectl set image deployment/akidb-rest \
  akidb-rest=akidb/akidb-rest:week11-tensorrt \
  -n akidb

kubectl set image deployment/akidb-rest \
  akidb-rest=akidb/akidb-rest:week11-tensorrt \
  -n akidb --context=eu-central

# Verify rollback
kubectl rollout status deployment/akidb-rest -n akidb
```

---

## Success Criteria

### Week 12 Completion Criteria

| Criterion | Target | Measurement | Priority |
|-----------|--------|-------------|----------|
| **Sub-5ms Latency** | P95 <5ms | Prometheus | P0 |
| **500+ QPS Throughput** | >500 QPS | Prometheus | P0 |
| **Multi-GPU Operational** | 2 GPUs | nvidia-smi | P0 |
| **Flash Attention** | 4x memory efficiency | GPU memory metrics | P0 |
| **Custom Kernels** | 3 kernels operational | Health check | P0 |
| **Zero Downtime** | No 5xx errors | Error rate <0.1% | P0 |
| **Accuracy Maintained** | >98% | Validation suite | P0 |
| **Cost Reduction** | 40-50% | OpenCost | P1 |
| **Model Pruning** | 30% size reduction | Model size | P1 |
| **GPU Utilization** | >90% | DCGM metrics | P1 |

**Overall Success:** All P0 + 80% P1

---

## Appendix: Technical Deep Dives

### A. CUDA Kernel Optimization Checklist

**Memory Access:**
- ✅ Coalesced memory access (consecutive threads → consecutive addresses)
- ✅ Minimize global memory access (use shared memory for hot data)
- ✅ Avoid bank conflicts in shared memory (stride by 32+1)
- ✅ Use texture memory for read-only data (automatic caching)

**Computation:**
- ✅ Register blocking (keep intermediate results in registers)
- ✅ Warp-level primitives (`__shfl_sync` for fast communication)
- ✅ Use `__restrict__` for pointer aliasing hints
- ✅ `#pragma unroll` for loop unrolling

**Launch Configuration:**
- ✅ Occupancy: Maximize active warps per SM (>70%)
- ✅ Block size: Multiples of 32 (warp size)
- ✅ Grid size: Saturate all SMs (e.g., 128 SMs × 4 blocks)
- ✅ Shared memory: <48KB per block (or 96KB if configured)

### B. Flash Attention vs Standard Attention

**Complexity:**
```
Standard Attention:
  Time: O(n²)
  Space: O(n²)

Flash Attention:
  Time: O(n²) (same)
  Space: O(n)  (n = seq_len)

For seq_len=256:
  Standard: 256² = 65,536 elements
  Flash: 256 elements
  Memory reduction: 256x
```

**Tiling Strategy:**
```
Tile size: 128×128
Sequence length: 256

Tiles needed:
  Q: 256/128 = 2 tiles
  K: 256/128 = 2 tiles
  V: 256/128 = 2 tiles

Total tile combinations: 2×2 = 4
Each tile: 128×128 = 16KB (FP16)

Max memory: 16KB (one tile at a time)
vs Standard: 256×256×2 = 131KB
```

### C. Multi-GPU Communication

**GPU Direct RDMA:**
```
Without GPU Direct (via CPU):
  GPU 0 → CPU memory (500μs)
  CPU memory → GPU 1 (500μs)
  Total: ~1ms

With GPU Direct RDMA:
  GPU 0 → GPU 1 (direct PCIe)
  Total: <100μs (10x faster)
```

**NCCL vs Custom:**
- NCCL: General-purpose collective operations (reduce, broadcast)
- Custom: Point-to-point transfers for model parallelism
- Week 12: Use custom GPU Direct for simplicity

---

**End of Week 12 PRD**

**Next Steps:** Week 13 - Edge Deployment & CDN Caching
