# Pluggable Acceleration Architecture

AkiDB 採用**可插拔加速架構**，將核心功能與硬體加速解耦，確保可攜性的同時提供選擇性加速能力。

---

## 設計原則

### 1. **平台無關的核心**
- I/O、索引、快取必須在所有平台高效運作
- 不依賴特定硬體或作業系統
- 支援 x86/ARM/Mac/Linux

### 2. **可插拔加速**
- MLX、CUDA、SIMD 只是加速選項，非核心依賴
- Runtime detection + automatic fallback
- 配置驅動，用戶可選擇啟用/停用

### 3. **效能隔離**
- 加速層失敗不影響基礎功能
- QoS 分類確保關鍵路徑優先
- CPU fallback 永遠可用

---

## 三層架構

```
┌──────────────────────────────────────────────────────────────┐
│ Layer 3: Acceleration Adapters (可插拔)                      │
│  MLX / CUDA / OpenCL / CPU-SIMD / Future Plugins             │
│  → Accelerator Orchestrator (task classifier + scheduler)   │
├──────────────────────────────────────────────────────────────┤
│ Layer 2: Index & Cache Services (跨平台)                     │
│  Adaptive B-Tree / Columnar Index / TTL & Query Cache        │
│  → Planner integration, cost models, hot-set tracking       │
├──────────────────────────────────────────────────────────────┤
│ Layer 1: Core I/O & Storage (跨平台)                         │
│  WAL, segmented log, page cache, compression, encryption     │
│  → File/Block abstraction, async IO reactor, checksum stack │
├──────────────────────────────────────────────────────────────┤
│ Platform Abstraction (FS, threads, SIMD, NUMA hints)         │
└──────────────────────────────────────────────────────────────┘
```

### Layer 1: Core I/O & Storage (必須跨平台)
- **WAL 系統**: 日誌結構化頁面，可移植 I/O 原語
- **Segment 管理**: 壓縮、加密、校驗和
- **MinIO 整合**: S3-compatible storage backend
- **非同步 I/O**: Tokio reactor, zero-copy where possible

### Layer 2: Index & Cache (必須跨平台)
- **HNSW Index**: 純 Rust 實現 (hnsw_rs)
- **分層快取**: Hot (NVMe) → Warm (RocksDB) → Cold (MinIO)
- **Query Planner**: 成本模型、熱集追蹤
- **Filter Pushdown**: 3-tier strategy based on selectivity

### Layer 3: Acceleration (可插拔)
- **Task Classifier**: 識別可加速的批量任務
- **Accelerator Orchestrator**: 調度、fallback、QoS
- **Pluggable Backends**:
  - `CpuSimdAccelerator`: SSE/AVX/NEON (baseline)
  - `MlxAccelerator`: Apple Silicon (optional)
  - `CudaAccelerator`: NVIDIA GPU (future)
  - `VulkanAccelerator`: Cross-platform GPU (future)

---

## VectorAccelerator Trait

### Trait 定義

```rust
use std::sync::Arc;
use bitflags::bitflags;
use thiserror::Error;

bitflags! {
    /// Accelerator capabilities
    pub struct CapabilitySet: u32 {
        const DOT_PRODUCT   = 0b0001;  // 向量點積
        const MATMUL        = 0b0010;  // 矩陣乘法
        const SIMD_SORT     = 0b0100;  // SIMD 排序
        const VECTOR_INDEX  = 0b1000;  // 向量索引構建
    }
}

#[derive(Debug, Clone)]
pub struct VectorBatch<'a> {
    pub task_id: uuid::Uuid,
    pub workload: WorkloadClass,
    pub payload: BatchPayload<'a>,
    pub qos: QualityOfService,
}

#[derive(Debug)]
pub struct AcceleratorContext<'a> {
    pub io: Arc<dyn IoPortal>,
    pub metrics: &'a dyn MetricsSink,
    pub config: &'a AcceleratorConfig,
}

#[derive(Debug)]
pub struct AcceleratorOutcome {
    pub duration: std::time::Duration,
    pub throughput: f64,
    pub cpu_fallback_used: bool,
}

#[derive(Debug, Error)]
pub enum AcceleratorError {
    #[error("capability unsupported")]
    Unsupported,
    #[error("transient error: {0}")]
    Transient(String),
    #[error("fatal error: {0}")]
    Fatal(String),
}

/// Pluggable vector acceleration trait
pub trait VectorAccelerator: Send + Sync {
    /// Unique accelerator identifier
    fn id(&self) -> &'static str;

    /// Priority for selection (higher = preferred)
    fn priority(&self) -> u8;

    /// Supported operations
    fn capabilities(&self) -> CapabilitySet;

    /// Runtime availability check
    fn is_available(&self) -> bool;

    /// Warmup/initialization
    fn warm_up(&self, ctx: &AcceleratorContext<'_>) -> Result<(), AcceleratorError>;

    /// Execute batch workload
    fn execute_batch(
        &self,
        batch: &VectorBatch<'_>,
        ctx: &AcceleratorContext<'_>,
    ) -> Result<AcceleratorOutcome, AcceleratorError>;

    /// Fallback recommendation on failure
    fn fallback_hint(&self) -> Option<FallbackAction>;
}
```

### 實現範例

#### CPU SIMD Accelerator (Baseline)

```rust
pub struct CpuSimdAccelerator {
    simd_level: SimdLevel, // SSE, AVX2, AVX512, NEON
}

impl VectorAccelerator for CpuSimdAccelerator {
    fn id(&self) -> &'static str {
        "cpu-simd"
    }

    fn priority(&self) -> u8 {
        50 // Baseline priority
    }

    fn capabilities(&self) -> CapabilitySet {
        CapabilitySet::DOT_PRODUCT | CapabilitySet::SIMD_SORT
    }

    fn is_available(&self) -> bool {
        // Always available
        true
    }

    fn warm_up(&self, _ctx: &AcceleratorContext<'_>) -> Result<(), AcceleratorError> {
        // No warmup needed
        Ok(())
    }

    fn execute_batch(
        &self,
        batch: &VectorBatch<'_>,
        ctx: &AcceleratorContext<'_>,
    ) -> Result<AcceleratorOutcome, AcceleratorError> {
        let start = std::time::Instant::now();

        match batch.workload {
            WorkloadClass::DotProduct => {
                // Use SIMD intrinsics
                let result = simd::dot_product_batch(&batch.payload)?;
                Ok(AcceleratorOutcome {
                    duration: start.elapsed(),
                    throughput: batch.payload.len() as f64 / start.elapsed().as_secs_f64(),
                    cpu_fallback_used: false,
                })
            }
            _ => Err(AcceleratorError::Unsupported),
        }
    }

    fn fallback_hint(&self) -> Option<FallbackAction> {
        None // CPU is the fallback
    }
}
```

#### MLX Accelerator (Apple Silicon)

```rust
#[cfg(target_os = "macos")]
pub struct MlxAccelerator {
    device: mlx::Device,
    stream: mlx::Stream,
}

#[cfg(target_os = "macos")]
impl VectorAccelerator for MlxAccelerator {
    fn id(&self) -> &'static str {
        "mlx"
    }

    fn priority(&self) -> u8 {
        100 // High priority on macOS
    }

    fn capabilities(&self) -> CapabilitySet {
        CapabilitySet::all() // MLX supports all operations
    }

    fn is_available(&self) -> bool {
        mlx::runtime_available()
    }

    fn warm_up(&self, ctx: &AcceleratorContext<'_>) -> Result<(), AcceleratorError> {
        // Pre-allocate buffers, load kernels
        self.device.synchronize()
            .map_err(|e| AcceleratorError::Fatal(e.to_string()))
    }

    fn execute_batch(
        &self,
        batch: &VectorBatch<'_>,
        ctx: &AcceleratorContext<'_>,
    ) -> Result<AcceleratorOutcome, AcceleratorError> {
        let start = std::time::Instant::now();

        match batch.workload {
            WorkloadClass::MatMul => {
                // Transfer to GPU, compute, transfer back
                let result = mlx::matmul(&batch.payload, &self.stream)
                    .map_err(|e| AcceleratorError::Transient(e.to_string()))?;

                self.stream.synchronize()
                    .map_err(|e| AcceleratorError::Transient(e.to_string()))?;

                Ok(AcceleratorOutcome {
                    duration: start.elapsed(),
                    throughput: batch.payload.len() as f64 / start.elapsed().as_secs_f64(),
                    cpu_fallback_used: false,
                })
            }
            _ => Err(AcceleratorError::Unsupported),
        }
    }

    fn fallback_hint(&self) -> Option<FallbackAction> {
        Some(FallbackAction::UseCpu)
    }
}
```

---

## Runtime Detection

### Detection Flow

```rust
pub async fn detect_accelerators(cfg: &AcceleratorConfig) -> AcceleratorRegistry {
    let mut registry = AcceleratorRegistry::new();

    // 1. CPU SIMD (always available)
    if cfg.backends.contains(&BackendKind::CpuSimd) && cpu::has_required_simd() {
        registry.register(Box::new(CpuSimdAccelerator::new()));
    }

    // 2. MLX (macOS only, opt-in)
    #[cfg(target_os = "macos")]
    if cfg.backends.contains(&BackendKind::Mlx) && env::var("AKIDB_ENABLE_MLX") == Ok("1".into()) {
        if mlx::runtime_available().await {
            registry.register(Box::new(MlxAccelerator::new()));
        } else {
            registry.mark_disabled("mlx", DisableReason::MissingRuntime);
        }
    }

    // 3. CUDA (opt-in, future)
    #[cfg(feature = "cuda")]
    if cfg.backends.contains(&BackendKind::Cuda) {
        match cuda::Runtime::probe() {
            Ok(rt) => registry.register(Box::new(CudaAccelerator::with_runtime(rt))),
            Err(e) => registry.mark_disabled("cuda", DisableReason::ProbeFailed(e.to_string())),
        }
    }

    // 4. Ensure CPU fallback always exists
    registry.ensure_cpu_fallback();

    registry
}
```

### Health Monitoring

```rust
pub struct AcceleratorRegistry {
    accelerators: Vec<Box<dyn VectorAccelerator>>,
    health_monitor: HealthMonitor,
}

impl AcceleratorRegistry {
    pub async fn monitor_health(&mut self) {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;

            for accel in &self.accelerators {
                if !accel.is_available() {
                    self.health_monitor.demote(accel.id());
                } else {
                    self.health_monitor.promote(accel.id());
                }
            }
        }
    }
}
```

---

## Configuration System

### TOML Configuration

```toml
# config/accelerators.toml
[accelerators]
# Preferred order (first available wins)
preferred_order = ["mlx", "cuda", "cpu-simd"]

# Enable runtime detection
auto_detect = true

[accelerators.cpu-simd]
enabled = true
max_parallel_batches = 8
simd_level = "avx2"  # auto, sse, avx2, avx512, neon

[accelerators.mlx]
enabled = true
require_runtime = true
warmup_timeout_ms = 250
device = "gpu"  # gpu, cpu
max_memory_gb = 4

[accelerators.cuda]
enabled = false  # Opt-in by operator
device_whitelist = [0]
memory_fraction = 0.8

[qos]
# Latency-critical workloads bypass queue
latency_critical_threshold_ms = 10

# Throughput workloads can be batched
throughput_batch_size = 64
```

### Environment Variables

```bash
# Enable/disable specific accelerators
AKIDB_ENABLE_MLX=1
AKIDB_ENABLE_CUDA=0

# Force specific accelerator (bypass detection)
AKIDB_FORCE_ACCELERATOR=cpu-simd

# Debug mode (log all accelerator decisions)
AKIDB_ACCEL_DEBUG=1
```

### CLI Introspection

```bash
# Show detected accelerators
$ akidb --show-accelerators

Detected Accelerators:
  ✓ cpu-simd (priority: 50, capabilities: DOT_PRODUCT | SIMD_SORT)
  ✓ mlx (priority: 100, capabilities: ALL)
  ✗ cuda (disabled: MissingRuntime)

Active: mlx
Fallback: cpu-simd
```

---

## Performance Isolation

### QoS Classification

```rust
pub enum QualityOfService {
    LatencyCritical,  // Real-time queries (< 10ms SLA)
    Throughput,       // Batch operations (best effort)
    Background,       // Index rebuilds (lowest priority)
}

pub struct TaskClassifier;

impl TaskClassifier {
    pub fn classify(req: &QueryRequest) -> QualityOfService {
        if req.timeout_ms < 10 {
            QualityOfService::LatencyCritical
        } else if req.batch_size > 32 {
            QualityOfService::Throughput
        } else {
            QualityOfService::Background
        }
    }
}
```

### Admission Control

```rust
pub struct AcceleratorOrchestrator {
    registry: Arc<AcceleratorRegistry>,
    token_buckets: HashMap<String, TokenBucket>,
    cpu_fallback_pool: ThreadPool,
}

impl AcceleratorOrchestrator {
    pub async fn submit(&self, batch: VectorBatch<'_>) -> Result<AcceleratorOutcome, Error> {
        // 1. Classify workload
        let qos = TaskClassifier::classify(&batch);

        // 2. Select accelerator
        let accel = self.registry.select_for_workload(&batch.workload, qos)?;

        // 3. Check admission (token bucket)
        if !self.token_buckets.get(accel.id()).unwrap().try_acquire() {
            // Queue full, fallback to CPU
            return self.cpu_fallback_pool.execute(batch).await;
        }

        // 4. Execute with circuit breaker
        match accel.execute_batch(&batch, &ctx).await {
            Ok(outcome) => Ok(outcome),
            Err(AcceleratorError::Transient(_)) => {
                // Retry once on CPU
                self.cpu_fallback_pool.execute(batch).await
            }
            Err(e) => Err(e.into()),
        }
    }
}
```

### Circuit Breaker

```rust
pub struct CircuitBreaker {
    failure_threshold: usize,
    failures: AtomicUsize,
    state: AtomicU8, // Closed, Open, HalfOpen
}

impl CircuitBreaker {
    pub fn record_success(&self) {
        self.failures.store(0, Ordering::Relaxed);
        self.state.store(State::Closed as u8, Ordering::Relaxed);
    }

    pub fn record_failure(&self) {
        let failures = self.failures.fetch_add(1, Ordering::Relaxed);
        if failures >= self.failure_threshold {
            self.state.store(State::Open as u8, Ordering::Relaxed);
            tracing::warn!("Circuit breaker tripped");
        }
    }

    pub fn is_open(&self) -> bool {
        self.state.load(Ordering::Relaxed) == State::Open as u8
    }
}
```

---

## Metrics & Observability

### Prometheus Metrics

```rust
lazy_static! {
    static ref ACCEL_DURATION: HistogramVec = register_histogram_vec!(
        "akidb_accelerator_duration_seconds",
        "Accelerator execution duration",
        &["accelerator", "workload"]
    ).unwrap();

    static ref ACCEL_FALLBACK: CounterVec = register_counter_vec!(
        "akidb_accelerator_fallback_total",
        "Fallback to CPU count",
        &["accelerator", "reason"]
    ).unwrap();

    static ref ACCEL_QUEUE_DEPTH: GaugeVec = register_gauge_vec!(
        "akidb_accelerator_queue_depth",
        "Current queue depth",
        &["accelerator"]
    ).unwrap();
}
```

### Structured Logging

```rust
#[instrument(skip(batch, ctx))]
async fn execute_with_logging(
    batch: &VectorBatch<'_>,
    accel: &dyn VectorAccelerator,
    ctx: &AcceleratorContext<'_>,
) -> Result<AcceleratorOutcome, AcceleratorError> {
    tracing::info!(
        accelerator = accel.id(),
        workload = ?batch.workload,
        batch_size = batch.payload.len(),
        "Executing batch"
    );

    let result = accel.execute_batch(batch, ctx).await;

    match &result {
        Ok(outcome) => {
            tracing::info!(
                duration_ms = outcome.duration.as_millis(),
                throughput = outcome.throughput,
                fallback = outcome.cpu_fallback_used,
                "Batch completed"
            );
        }
        Err(e) => {
            tracing::warn!(error = ?e, "Batch failed");
        }
    }

    result
}
```

---

## Workload Identification

### 適合加速的操作

| 操作 | 適合加速 | 理由 | 加速器選擇 |
|------|----------|------|------------|
| **Index Building** | ✅ Yes | 大量向量距離計算 | MLX > CUDA > CPU-SIMD |
| **Batch Search** | ✅ Yes | 並行 k-NN 查詢 | MLX > CUDA > CPU-SIMD |
| **Vector Normalization** | ✅ Yes | SIMD-friendly | CPU-SIMD (sufficient) |
| **Single Query** | ❌ No | 傳輸開銷 > 計算 | CPU only |
| **Metadata Filter** | ❌ No | I/O bound | CPU only |
| **WAL Replay** | ❌ No | Sequential I/O | CPU only |

### Batch Size Thresholds

```rust
pub struct BatchingPolicy {
    min_batch_size: usize,
    max_batch_size: usize,
    timeout: Duration,
}

impl BatchingPolicy {
    pub fn should_batch(&self, pending: usize) -> bool {
        pending >= self.min_batch_size
    }

    pub fn is_accelerator_worthy(&self, batch_size: usize) -> bool {
        // Only use accelerator for large batches
        batch_size >= 32
    }
}
```

---

## Migration Path

### Phase 1: Foundation (Current → M1)
- ✅ Implement `VectorAccelerator` trait
- ✅ Create `AcceleratorRegistry` with detection
- ✅ Add `CpuSimdAccelerator` baseline
- ✅ Configuration system (TOML + env vars)

### Phase 2: MLX Integration (M2)
- ⏳ Implement `MlxAccelerator` behind feature flag
- ⏳ Runtime detection for Apple Silicon
- ⏳ Benchmark index building and batch search
- ⏳ Tune batch size thresholds

### Phase 3: Production Hardening (M3)
- ⏳ Circuit breaker + health monitoring
- ⏳ QoS classification + admission control
- ⏳ Metrics + observability
- ⏳ Integration tests with failure injection

### Phase 4: Future Accelerators (M4+)
- ⏳ CUDA support (NVIDIA GPU)
- ⏳ Vulkan compute (cross-platform GPU)
- ⏳ Custom FPGA/ASIC plugins

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cpu_simd_always_available() {
        let accel = CpuSimdAccelerator::new();
        assert!(accel.is_available());
        assert_eq!(accel.id(), "cpu-simd");
    }

    #[tokio::test]
    async fn test_fallback_on_failure() {
        let mut registry = AcceleratorRegistry::new();
        registry.register(Box::new(FakeFailingAccelerator));
        registry.register(Box::new(CpuSimdAccelerator::new()));

        let orchestrator = AcceleratorOrchestrator::new(registry);
        let batch = create_test_batch();

        let outcome = orchestrator.submit(batch).await.unwrap();
        assert!(outcome.cpu_fallback_used);
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_mlx_detection_on_macos() {
    #[cfg(target_os = "macos")]
    {
        let cfg = AcceleratorConfig::default();
        let registry = detect_accelerators(&cfg).await;

        if mlx::runtime_available() {
            assert!(registry.get("mlx").is_some());
        } else {
            assert!(registry.is_disabled("mlx"));
        }
    }
}
```

---

## References

- [Pluggable Architecture Pattern](https://en.wikipedia.org/wiki/Plug-in_(computing))
- [MLX Framework](https://github.com/ml-explore/mlx)
- [Rust SIMD](https://doc.rust-lang.org/std/simd/)
- [Circuit Breaker Pattern](https://martinfowler.com/bliki/CircuitBreaker.html)
