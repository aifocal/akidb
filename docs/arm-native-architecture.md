# ARM-Native Architecture for AkiDB

**AkiDB: ARM-native, S3-backed vector database optimized for offline semantic retrieval and edge AI pipelines**

---

## 1. Core Positioning

### 1.1 Target Platforms

**Primary Tier:**
- **macOS Apple Silicon**: M2, M3, M4, M4 Max/Ultra
- **Development**: First-class developer experience on MacBook Pro/Studio/Mac Mini
- **Compute**: Mac Studio clusters for high-throughput workloads

**Edge Tier:**
- **NVIDIA Jetson**: Orin NX, Orin Nano, AGX Orin, Thor (2025)
- **Use Cases**: Offline inference, air-gapped deployments, edge RAG
- **Power**: ≤25W (Orin NX), ≤15W (Orin Nano), ≤60W (AGX Orin)

**Cloud Tier:**
- **AWS Graviton**: 3, 4 (arm64 Neoverse cores)
- **Oracle Cloud**: A1 (Ampere Altra, 4 OCPU free tier)
- **Google Cloud**: Tau T2A (Ampere Altra)
- **Azure**: Cobalt 100 (Arm-based VMs)
- **Use Cases**: Cold storage, backup replication, disaster recovery

### 1.2 Design Philosophy

**ARM-First, Not Cross-Platform**:
- **No x86_64 support**: Explicit focus on arm64 ecosystem
- **Native optimizations**: NEON, SVE2, Apple AMX, NVIDIA Tensor Cores
- **Power efficiency**: ≤50W per node target (vs 150W+ for x86 servers)
- **Cost efficiency**: Apple Silicon Mac Mini (~$600) + Jetson Orin Nano (~$499)

**S3-Native Storage**:
- **Primary storage**: MinIO/S3 (cold tier)
- **Local cache**: NVMe SSD (hot tier)
- **No distributed consensus**: Use S3 as source of truth

**Offline-First RAG**:
- **Air-gapped deployments**: No internet dependency after setup
- **Embedded models**: Run embeddings + LLMs locally
- **Data sovereignty**: All data stays on-premises

---

## 2. ComputeBackend Trait Architecture

### 2.1 Trait Definition

```rust
// crates/akidb-compute/src/backend.rs

use std::sync::Arc;
use serde::{Deserialize, Serialize};

/// Capabilities that a compute backend can provide
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    /// Basic SIMD operations (NEON, SVE)
    VectorSIMD,

    /// Matrix multiplication (AMX, Tensor Cores)
    MatrixMultiply,

    /// GPU-accelerated operations (Metal, CUDA)
    GPUAcceleration,

    /// Quantized operations (INT4, INT8)
    Quantization,

    /// Batch operations
    BatchProcessing,
}

/// Runtime information about the compute backend
#[derive(Debug, Clone)]
pub struct BackendInfo {
    pub id: &'static str,
    pub platform: Platform,
    pub capabilities: Vec<Capability>,
    pub max_batch_size: usize,
    pub memory_gb: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    AppleSilicon,
    NvidiaJetson,
    ArmCpu,
}

/// Vector operation types
pub enum VectorOp<'a> {
    /// L2 distance: ||a - b||²
    L2Distance { query: &'a [f32], vectors: &'a [f32], dim: usize },

    /// Cosine similarity: dot(a, b) / (||a|| * ||b||)
    CosineSimilarity { query: &'a [f32], vectors: &'a [f32], dim: usize },

    /// Dot product: dot(a, b)
    DotProduct { query: &'a [f32], vectors: &'a [f32], dim: usize },

    /// Matrix multiplication: A @ B
    MatMul { a: &'a [f32], b: &'a [f32], m: usize, n: usize, k: usize },
}

/// Result of a vector operation
pub enum VectorResult {
    Distances(Vec<f32>),
    Similarities(Vec<f32>),
    Matrix { data: Vec<f32>, rows: usize, cols: usize },
}

/// Error types for compute backends
#[derive(Debug, thiserror::Error)]
pub enum ComputeError {
    #[error("Backend not available: {0}")]
    Unavailable(String),

    #[error("Operation not supported: {0}")]
    Unsupported(String),

    #[error("Compute error: {0}")]
    Execution(String),

    #[error("Out of memory: requested {requested} GB, available {available} GB")]
    OutOfMemory { requested: f32, available: f32 },
}

/// Main trait for pluggable compute backends
#[async_trait::async_trait]
pub trait ComputeBackend: Send + Sync {
    /// Get backend information
    fn info(&self) -> &BackendInfo;

    /// Check if backend is currently available
    fn is_available(&self) -> bool;

    /// Execute a vector operation
    async fn execute(
        &self,
        op: VectorOp<'_>,
    ) -> Result<VectorResult, ComputeError>;

    /// Execute multiple operations in batch
    async fn execute_batch(
        &self,
        ops: &[VectorOp<'_>],
    ) -> Result<Vec<VectorResult>, ComputeError> {
        // Default implementation: sequential execution
        let mut results = Vec::with_capacity(ops.len());
        for op in ops {
            results.push(self.execute(*op).await?);
        }
        Ok(results)
    }

    /// Warm up the backend (preload models, JIT compile kernels, etc.)
    async fn warm_up(&self) -> Result<(), ComputeError> {
        Ok(())
    }

    /// Get current memory usage in GB
    fn memory_usage(&self) -> f32 {
        0.0
    }

    /// Get suggested fallback backend if this one fails
    fn fallback_backend_id(&self) -> Option<&'static str> {
        None
    }
}
```

### 2.2 Backend Implementations

#### 2.2.1 CPU SIMD Backend (Baseline)

```rust
// crates/akidb-compute/src/cpu_simd.rs

use super::{BackendInfo, Capability, ComputeBackend, Platform, VectorOp, VectorResult};

pub struct CpuSimdBackend {
    info: BackendInfo,
}

impl CpuSimdBackend {
    pub fn new() -> Self {
        Self {
            info: BackendInfo {
                id: "cpu_simd",
                platform: Platform::ArmCpu,
                capabilities: vec![
                    Capability::VectorSIMD,
                    Capability::BatchProcessing,
                ],
                max_batch_size: 1024,
                memory_gb: Self::detect_memory_gb(),
            },
        }
    }

    fn detect_memory_gb() -> f32 {
        // Use sysinfo or platform-specific APIs
        16.0 // Placeholder
    }

    #[cfg(target_arch = "aarch64")]
    fn l2_distance_neon(query: &[f32], vectors: &[f32], dim: usize) -> Vec<f32> {
        use std::arch::aarch64::*;

        let num_vectors = vectors.len() / dim;
        let mut distances = vec![0.0f32; num_vectors];

        unsafe {
            for i in 0..num_vectors {
                let vector = &vectors[i * dim..(i + 1) * dim];
                let mut sum = vdupq_n_f32(0.0);

                // Process 4 floats at a time
                for j in (0..dim).step_by(4) {
                    if j + 4 <= dim {
                        let q = vld1q_f32(query.as_ptr().add(j));
                        let v = vld1q_f32(vector.as_ptr().add(j));
                        let diff = vsubq_f32(q, v);
                        sum = vfmaq_f32(sum, diff, diff); // sum += diff * diff
                    } else {
                        // Handle remainder
                        for k in j..dim {
                            let diff = query[k] - vector[k];
                            distances[i] += diff * diff;
                        }
                    }
                }

                // Horizontal sum
                let sum_scalar = vaddvq_f32(sum);
                distances[i] += sum_scalar;
            }
        }

        distances
    }
}

#[async_trait::async_trait]
impl ComputeBackend for CpuSimdBackend {
    fn info(&self) -> &BackendInfo {
        &self.info
    }

    fn is_available(&self) -> bool {
        cfg!(target_arch = "aarch64")
    }

    async fn execute(&self, op: VectorOp<'_>) -> Result<VectorResult, ComputeError> {
        match op {
            VectorOp::L2Distance { query, vectors, dim } => {
                #[cfg(target_arch = "aarch64")]
                {
                    let distances = Self::l2_distance_neon(query, vectors, dim);
                    Ok(VectorResult::Distances(distances))
                }

                #[cfg(not(target_arch = "aarch64"))]
                {
                    Err(ComputeError::Unsupported("NEON only available on aarch64".to_string()))
                }
            }
            _ => Err(ComputeError::Unsupported(format!("Operation not implemented"))),
        }
    }
}
```

#### 2.2.2 MLX Metal Backend (macOS)

```rust
// crates/akidb-compute/src/mlx_metal.rs

#[cfg(target_os = "macos")]
use super::{BackendInfo, Capability, ComputeBackend, Platform, VectorOp, VectorResult, ComputeError};

#[cfg(target_os = "macos")]
pub struct MlxMetalBackend {
    info: BackendInfo,
    // FFI bindings to MLX C++ API or Swift bindings
}

#[cfg(target_os = "macos")]
impl MlxMetalBackend {
    pub fn new() -> Result<Self, ComputeError> {
        // Check Metal availability
        if !Self::is_metal_available() {
            return Err(ComputeError::Unavailable("Metal not available".to_string()));
        }

        Ok(Self {
            info: BackendInfo {
                id: "mlx_metal",
                platform: Platform::AppleSilicon,
                capabilities: vec![
                    Capability::VectorSIMD,
                    Capability::MatrixMultiply,
                    Capability::GPUAcceleration,
                    Capability::BatchProcessing,
                    Capability::Quantization,
                ],
                max_batch_size: 4096,
                memory_gb: Self::detect_unified_memory(),
            },
        })
    }

    fn is_metal_available() -> bool {
        // Check via metal-rs or system APIs
        true // Placeholder
    }

    fn detect_unified_memory() -> f32 {
        // Query system memory (shared between CPU and GPU on Apple Silicon)
        32.0 // Placeholder
    }
}

#[cfg(target_os = "macos")]
#[async_trait::async_trait]
impl ComputeBackend for MlxMetalBackend {
    fn info(&self) -> &BackendInfo {
        &self.info
    }

    fn is_available(&self) -> bool {
        Self::is_metal_available()
    }

    async fn execute(&self, op: VectorOp<'_>) -> Result<VectorResult, ComputeError> {
        // Use MLX for GPU-accelerated operations
        // FFI to mlx::core::array, mlx::core::Device
        todo!("MLX integration")
    }

    fn fallback_backend_id(&self) -> Option<&'static str> {
        Some("cpu_simd")
    }
}
```

#### 2.2.3 CUDA Tensor Core Backend (Jetson)

```rust
// crates/akidb-compute/src/cuda_tensorcore.rs

#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
use super::{BackendInfo, Capability, ComputeBackend, Platform, VectorOp, VectorResult, ComputeError};

#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
pub struct CudaTensorCoreBackend {
    info: BackendInfo,
    // CUDA context
}

#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
impl CudaTensorCoreBackend {
    pub fn new() -> Result<Self, ComputeError> {
        // Check CUDA availability via cudart or nvidia-smi
        if !Self::is_cuda_available() {
            return Err(ComputeError::Unavailable("CUDA not available".to_string()));
        }

        Ok(Self {
            info: BackendInfo {
                id: "cuda_tensorcore",
                platform: Platform::NvidiaJetson,
                capabilities: vec![
                    Capability::VectorSIMD,
                    Capability::MatrixMultiply,
                    Capability::GPUAcceleration,
                    Capability::BatchProcessing,
                    Capability::Quantization,
                ],
                max_batch_size: 2048,
                memory_gb: Self::detect_gpu_memory(),
            },
        })
    }

    fn is_cuda_available() -> bool {
        // Check via cudarc or nvidia-ml-rs
        std::path::Path::new("/usr/local/cuda").exists()
    }

    fn detect_gpu_memory() -> f32 {
        // Query via CUDA API
        8.0 // Orin Nano has 8GB
    }
}

#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
#[async_trait::async_trait]
impl ComputeBackend for CudaTensorCoreBackend {
    fn info(&self) -> &BackendInfo {
        &self.info
    }

    fn is_available(&self) -> bool {
        Self::is_cuda_available()
    }

    async fn execute(&self, op: VectorOp<'_>) -> Result<VectorResult, ComputeError> {
        // Use cuBLAS for GEMM, cuDNN for convolutions
        // Or use faer/ndarray with CUDA backend
        todo!("CUDA integration")
    }

    fn fallback_backend_id(&self) -> Option<&'static str> {
        Some("cpu_simd")
    }
}
```

### 2.3 Runtime Detection and Registry

```rust
// crates/akidb-compute/src/registry.rs

use super::{ComputeBackend, ComputeError};
use std::sync::Arc;
use once_cell::sync::Lazy;

pub struct ComputeRegistry {
    backends: Vec<Arc<dyn ComputeBackend>>,
}

impl ComputeRegistry {
    pub fn new() -> Self {
        let mut backends: Vec<Arc<dyn ComputeBackend>> = Vec::new();

        // Try to register backends in priority order

        #[cfg(target_os = "macos")]
        {
            if let Ok(mlx) = crate::mlx_metal::MlxMetalBackend::new() {
                tracing::info!("Registered MLX Metal backend");
                backends.push(Arc::new(mlx));
            }
        }

        #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
        {
            if let Ok(cuda) = crate::cuda_tensorcore::CudaTensorCoreBackend::new() {
                tracing::info!("Registered CUDA Tensor Core backend");
                backends.push(Arc::new(cuda));
            }
        }

        // Always register CPU SIMD as fallback
        let cpu = crate::cpu_simd::CpuSimdBackend::new();
        tracing::info!("Registered CPU SIMD backend");
        backends.push(Arc::new(cpu));

        Self { backends }
    }

    pub fn get_best_backend(&self) -> Option<Arc<dyn ComputeBackend>> {
        self.backends.iter().find(|b| b.is_available()).cloned()
    }

    pub fn get_backend_by_id(&self, id: &str) -> Option<Arc<dyn ComputeBackend>> {
        self.backends.iter().find(|b| b.info().id == id).cloned()
    }

    pub fn list_available(&self) -> Vec<Arc<dyn ComputeBackend>> {
        self.backends.iter().filter(|b| b.is_available()).cloned().collect()
    }
}

pub static COMPUTE_REGISTRY: Lazy<ComputeRegistry> = Lazy::new(ComputeRegistry::new);
```

---

## 3. Compilation & Testing Matrix

### 3.1 Target Triples

| Platform | Target Triple | Compiler | Notes |
|----------|---------------|----------|-------|
| **macOS Apple Silicon** | `aarch64-apple-darwin` | `rustc` + Xcode | M2/M3/M4, Metal, MLX |
| **NVIDIA Jetson (Ubuntu)** | `aarch64-unknown-linux-gnu` | `rustc` + `aarch64-linux-gnu-gcc` | CUDA 12.x, L4T |
| **ARM Cloud (AWS Graviton)** | `aarch64-unknown-linux-gnu` | `rustc` | AL2023, no GPU |
| **ARM Cloud (Oracle A1)** | `aarch64-unknown-linux-gnu` | `rustc` | OL8, no GPU |

### 3.2 Conditional Compilation Strategy

```toml
# Cargo.toml

[features]
default = ["cpu_simd"]

# Compute backends
cpu_simd = []
mlx_metal = ["metal", "mlx-sys"]  # macOS only
cuda_tensorcore = ["cudarc", "nvidia-ml-rs"]  # Jetson only

# Platform detection
apple_silicon = []
nvidia_jetson = []
arm_cloud = []

[target.'cfg(target_os = "macos")'.dependencies]
metal = { version = "0.28", optional = true }
mlx-sys = { version = "0.1", optional = true }  # Hypothetical MLX bindings

[target.'cfg(all(target_arch = "aarch64", target_os = "linux"))'.dependencies]
cudarc = { version = "0.11", optional = true, features = ["cuda-12"] }
nvidia-ml-rs = { version = "0.1", optional = true }

[dependencies]
# Common dependencies
async-trait = "0.1"
thiserror = "1.0"
tracing = "0.1"
serde = { version = "1.0", features = ["derive"] }
once_cell = "1.19"
```

```rust
// Build script: build.rs

fn main() {
    // Detect platform
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    if target_os == "macos" && target_arch == "aarch64" {
        println!("cargo:rustc-cfg=apple_silicon");

        // Check for Metal availability
        if std::process::Command::new("xcrun")
            .args(&["--find", "metal"])
            .output()
            .is_ok()
        {
            println!("cargo:rustc-cfg=has_metal");
        }
    }

    if target_os == "linux" && target_arch == "aarch64" {
        // Check for CUDA
        if std::path::Path::new("/usr/local/cuda").exists() {
            println!("cargo:rustc-cfg=nvidia_jetson");
            println!("cargo:rustc-link-search=native=/usr/local/cuda/lib64");
        } else {
            println!("cargo:rustc-cfg=arm_cloud");
        }
    }
}
```

### 3.3 Testing Matrix

```yaml
# .github/workflows/ci.yml

name: CI

on: [push, pull_request]

jobs:
  test-macos:
    runs-on: macos-14  # macOS 14 (Sonoma) on Apple Silicon
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: aarch64-apple-darwin

      - name: Build with MLX Metal
        run: cargo build --release --features mlx_metal

      - name: Run tests
        run: cargo test --all --features mlx_metal

      - name: Benchmark
        run: cargo bench --bench vector_ops

  test-jetson:
    # Self-hosted runner on Jetson Orin
    runs-on: [self-hosted, linux, ARM64, jetson]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: aarch64-unknown-linux-gnu

      - name: Build with CUDA
        run: cargo build --release --features cuda_tensorcore

      - name: Run tests
        run: cargo test --all --features cuda_tensorcore

  test-arm-cloud:
    # GitHub-hosted or self-hosted ARM runner
    runs-on: ubuntu-latest-arm
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: aarch64-unknown-linux-gnu

      - name: Build CPU-only
        run: cargo build --release --features cpu_simd

      - name: Run tests
        run: cargo test --all --features cpu_simd
```

### 3.4 Cross-Compilation

```bash
# From macOS, build for Jetson
cargo build --release --target aarch64-unknown-linux-gnu --features cuda_tensorcore

# From Jetson, build for macOS (not recommended, use macOS native)
# N/A - use native builds on each platform
```

---

## 4. Power Optimization

### 4.1 Dynamic Frequency Scaling

```rust
// crates/akidb-compute/src/power.rs

use std::fs;
use std::path::Path;

pub struct PowerManager {
    governor: Governor,
}

#[derive(Debug, Clone, Copy)]
pub enum Governor {
    Performance,  // Max frequency, no throttling
    Balanced,     // Dynamic scaling
    PowerSave,    // Min frequency
}

impl PowerManager {
    pub fn new() -> Self {
        Self {
            governor: Governor::Balanced,
        }
    }

    #[cfg(target_os = "linux")]
    pub fn set_governor(&mut self, governor: Governor) -> Result<(), std::io::Error> {
        let governor_str = match governor {
            Governor::Performance => "performance",
            Governor::Balanced => "schedutil",  // or "ondemand"
            Governor::PowerSave => "powersave",
        };

        // Find all CPU cores
        for cpu in 0..Self::num_cpus() {
            let path = format!("/sys/devices/system/cpu/cpu{}/cpufreq/scaling_governor", cpu);
            if Path::new(&path).exists() {
                fs::write(&path, governor_str)?;
            }
        }

        self.governor = governor;
        tracing::info!("Set CPU governor to {:?}", governor);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    pub fn set_governor(&mut self, governor: Governor) -> Result<(), std::io::Error> {
        // macOS manages power automatically, no user control
        tracing::warn!("CPU governor control not available on macOS");
        Ok(())
    }

    fn num_cpus() -> usize {
        num_cpus::get()
    }
}
```

### 4.2 Batch Coalescing

```rust
// Coalesce small queries into larger batches to amortize overhead

pub struct BatchCoalescer {
    pending: Vec<VectorOp<'static>>,
    max_batch_size: usize,
    timeout: Duration,
}

impl BatchCoalescer {
    pub async fn submit(&mut self, op: VectorOp<'static>) -> Result<VectorResult, ComputeError> {
        self.pending.push(op);

        if self.pending.len() >= self.max_batch_size {
            return self.flush().await;
        }

        // Wait for more ops or timeout
        tokio::time::sleep(self.timeout).await;
        self.flush().await
    }

    async fn flush(&mut self) -> Result<VectorResult, ComputeError> {
        let batch = std::mem::take(&mut self.pending);
        let backend = COMPUTE_REGISTRY.get_best_backend().unwrap();
        let results = backend.execute_batch(&batch).await?;
        // Return first result (simplified)
        Ok(results.into_iter().next().unwrap())
    }
}
```

### 4.3 Idle Sleep States

```rust
// Put cores to sleep when idle

pub struct IdleMonitor {
    last_activity: Instant,
    idle_threshold: Duration,
}

impl IdleMonitor {
    pub async fn run(mut self) {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;

            if self.last_activity.elapsed() > self.idle_threshold {
                // Enter low-power state
                PowerManager::new().set_governor(Governor::PowerSave).ok();

                // Wait for activity
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        }
    }

    pub fn record_activity(&mut self) {
        self.last_activity = Instant::now();
        PowerManager::new().set_governor(Governor::Balanced).ok();
    }
}
```

---

## 5. Cluster Integration Architecture

### 5.1 Deployment Topology

```
┌─────────────────────────────────────────────────────────────────┐
│                      Mac Studio Cluster                        │
│                     (Primary Compute Tier)                      │
│                                                                 │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐      │
│  │ Mac Studio 1  │  │ Mac Studio 2  │  │ Mac Studio 3  │      │
│  │ M2 Ultra      │  │ M2 Ultra      │  │ M2 Ultra      │      │
│  │ 128GB RAM     │  │ 128GB RAM     │  │ 128GB RAM     │      │
│  │ 2TB NVMe      │  │ 2TB NVMe      │  │ 2TB NVMe      │      │
│  │               │  │               │  │               │      │
│  │ AkiDB Node    │  │ AkiDB Node    │  │ AkiDB Node    │      │
│  │ + MLX Metal   │  │ + MLX Metal   │  │ + MLX Metal   │      │
│  └───────┬───────┘  └───────┬───────┘  └───────┬───────┘      │
│          │                  │                  │                │
└──────────┼──────────────────┼──────────────────┼────────────────┘
           │                  │                  │
           └──────────────────┼──────────────────┘
                              ↓
                    ┌──────────────────┐
                    │   NATS Cluster   │
                    │   (3+ nodes)     │
                    │                  │
                    │   JetStream      │
                    │   + KV Store     │
                    └────────┬─────────┘
                             │
           ┌─────────────────┼─────────────────┐
           ↓                 ↓                 ↓
┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│ Jetson Orin 1    │  │ Jetson Orin 2    │  │ Jetson Orin 3    │
│ AGX Orin 64GB    │  │ Orin NX 16GB     │  │ Orin Nano 8GB    │
│ 1TB NVMe         │  │ 512GB NVMe       │  │ 256GB NVMe       │
│                  │  │                  │  │                  │
│ AkiDB Edge Node  │  │ AkiDB Edge Node  │  │ AkiDB Edge Node  │
│ + CUDA Tensor    │  │ + CUDA Tensor    │  │ + CUDA Tensor    │
│                  │  │                  │  │                  │
│ Use Case:        │  │ Use Case:        │  │ Use Case:        │
│ Offline RAG      │  │ Edge Inference   │  │ IoT Gateway      │
└──────────────────┘  └──────────────────┘  └──────────────────┘
           │                  │                  │
           └──────────────────┼──────────────────┘
                              ↓
                    ┌──────────────────┐
                    │  MinIO Cluster   │
                    │  (Cold Storage)  │
                    │                  │
                    │  4+ nodes        │
                    │  EC: 12D + 4P    │
                    │  Capacity: 100TB │
                    └────────┬─────────┘
                             │
                    ┌────────┴─────────┐
                    │  ARM Cloud       │
                    │  (Backup/DR)     │
                    │                  │
                    │  AWS Graviton    │
                    │  Oracle A1       │
                    └──────────────────┘
```

### 5.2 Communication Flow

```rust
// gRPC service definitions

// akidb.proto
syntax = "proto3";

package akidb.v1;

service VectorSearch {
  // Search vectors using HNSW index
  rpc Search(SearchRequest) returns (SearchResponse);

  // Batch search
  rpc BatchSearch(BatchSearchRequest) returns (BatchSearchResponse);

  // Insert vectors
  rpc Insert(InsertRequest) returns (InsertResponse);

  // Node health check
  rpc Health(HealthRequest) returns (HealthResponse);
}

message SearchRequest {
  string collection = 1;
  repeated float query = 2;
  uint32 k = 3;
  optional string filter = 4;
}

message SearchResponse {
  repeated SearchResult results = 1;
  float latency_ms = 2;
  string node_id = 3;
}

message SearchResult {
  uint64 id = 1;
  float distance = 2;
  map<string, string> metadata = 3;
}
```

### 5.3 Routing Strategy (Rendezvous Hashing)

```rust
// crates/akidb-cluster/src/routing.rs

use sha2::{Sha256, Digest};
use std::collections::HashMap;

pub struct RendezvousRouter {
    nodes: Vec<NodeInfo>,
}

#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub id: String,
    pub platform: Platform,
    pub address: String,
    pub weight: f32,  // Based on compute capability
}

impl RendezvousRouter {
    pub fn route(&self, collection: &str) -> &NodeInfo {
        let mut best_node = &self.nodes[0];
        let mut best_score = 0u64;

        for node in &self.nodes {
            let score = self.hash_score(collection, &node.id, node.weight);
            if score > best_score {
                best_score = score;
                best_node = node;
            }
        }

        best_node
    }

    fn hash_score(&self, key: &str, node_id: &str, weight: f32) -> u64 {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        hasher.update(node_id.as_bytes());
        let hash = hasher.finalize();

        // Convert first 8 bytes to u64
        let hash_u64 = u64::from_be_bytes(hash[0..8].try_into().unwrap());

        // Apply weight
        (hash_u64 as f64 * weight as f64) as u64
    }
}
```

### 5.4 Node Discovery (NATS KV)

```rust
// crates/akidb-cluster/src/discovery.rs

use async_nats::jetstream::kv::Store;

pub struct NodeDiscovery {
    kv_store: Store,
}

impl NodeDiscovery {
    pub async fn register(&self, node: NodeInfo) -> Result<(), anyhow::Error> {
        let key = format!("nodes/{}", node.id);
        let value = serde_json::to_vec(&node)?;

        self.kv_store.put(&key, value.into()).await?;

        // Set TTL (heartbeat every 10s)
        tokio::spawn(self.heartbeat_loop(node.id.clone()));

        Ok(())
    }

    pub async fn discover(&self) -> Result<Vec<NodeInfo>, anyhow::Error> {
        let mut nodes = Vec::new();

        let keys = self.kv_store.keys("nodes/*").await?;
        for key in keys {
            if let Ok(entry) = self.kv_store.get(&key).await {
                let node: NodeInfo = serde_json::from_slice(&entry.value)?;
                nodes.push(node);
            }
        }

        Ok(nodes)
    }

    async fn heartbeat_loop(&self, node_id: String) {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;

            let key = format!("nodes/{}/heartbeat", node_id);
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            self.kv_store
                .put(&key, timestamp.to_string().into())
                .await
                .ok();
        }
    }
}
```

---

## 6. Storage Strategy

### 6.1 Two-Tier Storage

**Hot Tier (Local NVMe)**:
- Purpose: LRU cache for active segments
- Size: 10-20% of total dataset
- Format: Native HNSW index files
- Eviction: LRU with pinning support

**Cold Tier (MinIO/S3)**:
- Purpose: Primary storage for all segments
- Size: 100% of dataset
- Format: Compressed SEGv1 (Zstd level 9)
- Durability: Erasure coding (12D + 4P)

```rust
// crates/akidb-storage/src/tiered.rs

pub struct TieredStorage {
    hot: HotTier,
    cold: ColdTier,
}

pub struct HotTier {
    cache_dir: PathBuf,
    max_size_gb: f32,
    lru: LruCache<String, Arc<HnswIndex>>,
}

pub struct ColdTier {
    s3_backend: S3StorageBackend,
    compression_level: i32,
}

impl TieredStorage {
    pub async fn load_segment(&mut self, segment_id: &str) -> Result<Arc<HnswIndex>, StorageError> {
        // Check hot tier
        if let Some(index) = self.hot.lru.get(segment_id) {
            return Ok(index.clone());
        }

        // Load from cold tier
        let data = self.cold.s3_backend.read_segment(segment_id).await?;
        let decompressed = zstd::decode_all(&data[..])?;
        let index = HnswIndex::deserialize(&decompressed)?;
        let arc_index = Arc::new(index);

        // Cache in hot tier
        self.hot.lru.put(segment_id.to_string(), arc_index.clone());

        Ok(arc_index)
    }

    pub async fn write_segment(&mut self, segment_id: &str, index: HnswIndex) -> Result<(), StorageError> {
        // Write to cold tier first (durability)
        let serialized = index.serialize()?;
        let compressed = zstd::encode_all(&serialized[..], self.cold.compression_level)?;
        self.cold.s3_backend.write_segment(segment_id, &compressed).await?;

        // Then cache in hot tier
        self.hot.lru.put(segment_id.to_string(), Arc::new(index));

        Ok(())
    }
}
```

### 6.2 Configuration

```toml
# akidb.toml

[storage]
# Hot tier (local NVMe)
hot_tier_path = "/mnt/nvme/akidb/cache"
hot_tier_size_gb = 200  # 200GB cache

# Cold tier (MinIO)
cold_tier_endpoint = "http://minio.local:9000"
cold_tier_bucket = "akidb"
cold_tier_access_key = "akidb"
cold_tier_secret_key = "akidb_secret"
cold_tier_compression_level = 9  # Zstd (1-22)
```

---

## 7. Implementation Roadmap

### Phase 1: ComputeBackend Foundation (4 weeks)

**Week 1-2: Trait Definition & CPU SIMD**
- Define `ComputeBackend` trait and error types
- Implement `CpuSimdBackend` with NEON intrinsics
- Unit tests for L2/Cosine/Dot distance calculations
- Benchmark against naive Rust implementation

**Week 3: MLX Metal Backend (macOS)**
- Research MLX C++ API or create Swift bindings
- Implement `MlxMetalBackend::execute()` for basic ops
- Integration test on Mac Studio (M2 Ultra)
- Performance comparison: MLX vs CPU SIMD

**Week 4: CUDA Tensor Core Backend (Jetson)**
- Set up cross-compilation for Jetson Orin
- Implement `CudaTensorCoreBackend` using cuBLAS
- Test on Jetson Orin Nano dev kit
- Power measurement (idle, search, batch)

**Deliverables**:
- ✅ `crates/akidb-compute` with 3 backends
- ✅ CI pipeline for macOS + Jetson
- ✅ Performance report (QPS, latency, power)

---

### Phase 2: Cluster Integration (3 weeks)

**Week 5: NATS + Node Discovery**
- Deploy NATS JetStream cluster (3 nodes)
- Implement `NodeDiscovery` with KV store
- Heartbeat mechanism (10s interval)
- Test node join/leave scenarios

**Week 6: Rendezvous Hashing + gRPC**
- Implement `RendezvousRouter`
- Generate gRPC stubs from `akidb.proto`
- Basic RPC handlers (Search, Insert, Health)
- Load balancing test (100K requests)

**Week 7: Integration Testing**
- Mac cluster (3 Mac Studios) + Jetson (2 Orin NX)
- Multi-node search benchmark
- Failover test (kill 1 Mac node)
- Network partition simulation

**Deliverables**:
- ✅ `crates/akidb-cluster` with routing + discovery
- ✅ gRPC service implementation
- ✅ Cluster deployment guide

---

### Phase 3: Power Optimization (2 weeks)

**Week 8: Dynamic Frequency Scaling**
- Implement `PowerManager` for Linux (cpufreq)
- Batch coalescing logic
- Idle detection and sleep states
- Power profiling (measure watts with hardware monitor)

**Week 9: Optimization & Validation**
- Tune batch sizes for max throughput/watt
- Compare power consumption: Performance vs Balanced vs PowerSave
- Document best practices for edge deployment
- Validate ≤50W target on Jetson Orin NX

**Deliverables**:
- ✅ Power optimization framework
- ✅ Power consumption report
- ✅ Edge deployment guide

---

### Phase 4: Storage & End-to-End (3 weeks)

**Week 10: Tiered Storage**
- Implement `TieredStorage` with LRU cache
- MinIO cold tier integration
- Compression benchmarks (Zstd levels 1-22)
- Cache hit rate measurement

**Week 11: End-to-End Testing**
- Ingest 1M vectors (768D) across cluster
- Search workload (100 QPS sustained)
- Failure recovery (kill node, verify cache rebuild)
- Monitor metrics (Prometheus + Grafana)

**Week 12: Documentation & Release**
- Update README with ARM-native positioning
- Write deployment guides (Mac cluster, Jetson edge, ARM cloud)
- Create example docker-compose configs
- Release v0.2.0-alpha (ARM-native)

**Deliverables**:
- ✅ Production-ready tiered storage
- ✅ Complete documentation
- ✅ v0.2.0-alpha release on GitHub

---

## 8. Success Metrics

### 8.1 Performance Targets

| Metric | Target | Measured On |
|--------|--------|-------------|
| **Search Latency (P95)** | < 50ms | 1M vectors, 768D, k=50 |
| **Throughput** | > 200 QPS | Mac Studio M2 Ultra |
| **Power Consumption** | ≤ 50W | Jetson Orin NX under load |
| **Cache Hit Rate** | > 80% | Hot tier with 10% dataset cached |
| **Cluster Scalability** | Linear to 10 nodes | Mac cluster + Jetson edge |

### 8.2 Platform Validation

- ✅ macOS: M2/M3/M4 (Mac Studio, Mac Mini, MacBook Pro)
- ✅ Jetson: Orin Nano, Orin NX, AGX Orin
- ✅ ARM Cloud: AWS Graviton 3, Oracle A1

### 8.3 Deployment Validation

- ✅ Air-gapped: No internet after initial setup
- ✅ Single Binary: `akidb-server` runs standalone
- ✅ Docker Compose: 1-command cluster deployment
- ✅ Offline RAG: Embedded models + local inference

---

## 9. Example Configuration

```toml
# akidb.toml - Production Configuration

[cluster]
node_id = "mac-studio-01"
platform = "AppleSilicon"  # or "NvidiaJetson" or "ArmCloud"

[cluster.discovery]
nats_urls = ["nats://nats1.local:4222", "nats://nats2.local:4222", "nats://nats3.local:4222"]
heartbeat_interval_secs = 10

[cluster.routing]
# Rendezvous Hashing weights
weight = 1.0  # Higher for more powerful nodes (e.g., 2.0 for M2 Ultra)

[compute]
# Backend priority: try in order
backends = ["mlx_metal", "cuda_tensorcore", "cpu_simd"]

[compute.mlx_metal]
enabled = true
max_batch_size = 4096

[compute.cuda_tensorcore]
enabled = false  # Only on Jetson
max_batch_size = 2048

[compute.cpu_simd]
enabled = true
max_batch_size = 1024

[storage]
# Hot tier (local NVMe cache)
hot_tier_path = "/mnt/nvme/akidb/cache"
hot_tier_size_gb = 200

# Cold tier (MinIO)
cold_tier_type = "s3"
cold_tier_endpoint = "http://minio.local:9000"
cold_tier_bucket = "akidb"
cold_tier_region = "us-east-1"
cold_tier_access_key_env = "AKIDB_S3_ACCESS_KEY"
cold_tier_secret_key_env = "AKIDB_S3_SECRET_KEY"
cold_tier_compression_level = 9

[power]
# Power management (Linux only)
governor = "Balanced"  # Performance, Balanced, PowerSave
idle_threshold_secs = 60

[server]
bind_address = "0.0.0.0:8080"
grpc_bind_address = "0.0.0.0:9090"

[observability]
metrics_enabled = true
metrics_bind_address = "0.0.0.0:9091"
log_level = "info"
```

---

## 10. Next Steps

1. **Immediate** (This Week):
   - Create `crates/akidb-compute` directory structure
   - Implement `ComputeBackend` trait
   - Write NEON SIMD implementation for L2 distance
   - Set up CI for macOS (GitHub Actions)

2. **Short-Term** (Next 2 Weeks):
   - Research MLX integration (C++ FFI or Swift bindings)
   - Set up Jetson Orin Nano dev kit
   - Implement CUDA backend skeleton
   - Benchmark all 3 backends

3. **Medium-Term** (Next 4 Weeks):
   - NATS cluster deployment
   - gRPC service implementation
   - Cluster routing logic
   - Multi-node testing

4. **Long-Term** (Next 12 Weeks):
   - Complete all 4 phases
   - Production validation on real workloads
   - Documentation and examples
   - Public release (v0.2.0-alpha)

---

## Appendix A: Hardware Recommendations

### A.1 Development Setup

**Primary Workstation**:
- Mac Studio M2 Ultra (24-core CPU, 76-core GPU, 128GB RAM, 2TB SSD)
- Cost: ~$5,000
- Use: Main development, testing, benchmarking

**Edge Testing**:
- NVIDIA Jetson Orin Nano Developer Kit (8GB)
- Cost: ~$499
- Use: CUDA backend development, power profiling

**Cloud Testing**:
- Oracle Cloud A1 instance (4 OCPU, 24GB RAM)
- Cost: FREE (always free tier)
- Use: ARM cloud validation, CPU-only baseline

### A.2 Production Deployment (Small Scale)

**Compute Tier** (3 nodes):
- 3x Mac Mini M2 Pro (12-core CPU, 19-core GPU, 32GB RAM, 1TB SSD)
- Cost: ~$1,800/node = $5,400 total
- Power: ~40W each = 120W total

**Edge Tier** (2 nodes):
- 2x Jetson Orin NX (16GB)
- Cost: ~$899/node = $1,798 total
- Power: ~25W each = 50W total

**Storage Tier**:
- 4x Raspberry Pi 5 (8GB) + 4x 4TB HDD (MinIO cluster)
- Cost: ~$500 total
- Or: Oracle Cloud Object Storage (10GB free, $0.0255/GB/month beyond)

**Total Cost**: ~$7,700 (one-time) + minimal cloud costs
**Total Power**: ~170W (vs 600W+ for x86 equivalent)

---

## Appendix B: References

### B.1 Technical Documentation

- **Apple MLX**: https://github.com/ml-explore/mlx
- **NVIDIA Jetson**: https://developer.nvidia.com/embedded/jetson-orin
- **ARM NEON Intrinsics**: https://developer.arm.com/architectures/instruction-sets/intrinsics/
- **NATS JetStream**: https://docs.nats.io/nats-concepts/jetstream
- **MinIO**: https://min.io/docs

### B.2 Rust Libraries

- **async-trait**: https://crates.io/crates/async-trait
- **thiserror**: https://crates.io/crates/thiserror
- **tokio**: https://tokio.rs/
- **tonic** (gRPC): https://github.com/hyperium/tonic
- **object_store** (S3): https://crates.io/crates/object_store
- **zstd**: https://crates.io/crates/zstd

### B.3 Benchmarking

- **Criterion.rs**: https://github.com/bheisler/criterion.rs
- **perf** (Linux profiling): https://perf.wiki.kernel.org/
- **Instruments** (macOS profiling): Xcode built-in

---

**Document Version**: 1.0
**Last Updated**: 2025-10-29
**Status**: Draft for Review
