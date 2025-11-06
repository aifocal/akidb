# AkiDB 2.0 Testing Tools & Utilities PRD

**Product:** Testing Infrastructure for AkiDB 2.0
**Platform:** macOS MLX (Apple Silicon)
**Timeline:** 16 weeks (parallel with main development)
**Owner:** QA Engineering + DevTools Team

---

## Executive Summary

Comprehensive testing of AkiDB 2.0 requires specialized tools for data generation, cluster management, performance benchmarking, chaos engineering, and observability validation. This PRD defines a suite of testing utilities that enable efficient, reproducible, and automated testing across all phases of development.

### Key Deliverables

1. **`akidb-test-harness`** - Rust crate for test cluster lifecycle management
2. **`akidb-datagen`** - CLI tool for synthetic data generation
3. **`akidb-bench`** - Performance benchmarking suite
4. **`akidb-chaos`** - Chaos engineering framework
5. **`akidb-testkube`** - Test orchestration and reporting
6. **`akidb-fixtures`** - Curated test datasets and scenarios

**Total Tools:** 6 packages + supporting scripts
**LOC Estimate:** ~8,000 lines of Rust + 2,000 lines of scripts

---

## 1. Test Harness (`akidb-test-harness`)

### 1.1 Purpose

Provide a unified API for managing AkiDB test clusters, enabling developers to write integration and E2E tests without manual cluster setup.

### 1.2 Features

**Core Capabilities:**
- [x] Start/stop AkiDB clusters with configurable services
- [x] Tenant/database/collection CRUD via typed API
- [x] Ingest and query operations with automatic validation
- [x] Service lifecycle management (kill, restart, health checks)
- [x] Network partition simulation
- [x] Resource limit enforcement (memory, CPU, disk)
- [x] Log and metrics collection
- [x] Cleanup and teardown

**API Design:**

```rust
// akidb-test-harness/src/lib.rs

pub struct TestCluster {
    config: ClusterConfig,
    services: HashMap<String, ServiceHandle>,
    minio: Option<MinioContainer>,
    prometheus: Option<PrometheusContainer>,
}

#[derive(Debug, Clone)]
pub struct ClusterConfig {
    pub bind_address: String,           // "127.0.0.1:8080"
    pub data_dir: PathBuf,               // Temp directory for test data
    pub enable_minio: bool,              // Start S3-compatible storage
    pub enable_prometheus: bool,         // Start metrics collection
    pub enable_mlx_embeddings: bool,     // Start MLX embedding service
    pub memory_limit: Option<u64>,      // Memory limit in bytes
    pub log_level: LogLevel,             // "debug", "info", "warn", "error"
}

impl TestCluster {
    /// Start a new test cluster with default config
    pub async fn start() -> Result<Self> {
        Self::start_with_config(ClusterConfig::default()).await
    }

    /// Start with custom configuration
    pub async fn start_with_config(config: ClusterConfig) -> Result<Self> {
        // Create temp data directory
        let data_dir = tempdir()?;

        // Start MinIO if enabled
        let minio = if config.enable_minio {
            Some(MinioContainer::start().await?)
        } else {
            None
        };

        // Start AkiDB services
        let mut services = HashMap::new();
        services.insert("api".to_string(), start_akidb_api(&config).await?);

        if config.enable_mlx_embeddings {
            services.insert("embed".to_string(), start_akidb_embed(&config).await?);
        }

        // Wait for services to be healthy
        Self::wait_for_health(&services).await?;

        Ok(Self {
            config,
            services,
            minio,
            prometheus: None,
        })
    }

    /// Create a test tenant
    pub async fn create_tenant(&self, name: &str) -> Result<TenantDescriptor> {
        self.create_tenant_with_quota(name, TenantQuota::default()).await
    }

    /// Create tenant with custom quota
    pub async fn create_tenant_with_quota(&self, name: &str, quota: TenantQuota) -> Result<TenantDescriptor> {
        let client = self.http_client();
        let response = client
            .post(&format!("{}/api/v2/tenants", self.base_url()))
            .json(&json!({ "name": name, "quotas": quota }))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::ApiError(response.text().await?));
        }

        Ok(response.json().await?)
    }

    /// Create a database
    pub async fn create_database(&self, tenant_id: Uuid, name: &str) -> Result<DatabaseDescriptor> {
        // Implementation...
    }

    /// Create a collection
    pub async fn create_collection(&self, database_id: Uuid, config: CollectionConfig) -> Result<CollectionDescriptor> {
        // Implementation...
    }

    /// Ingest vectors with automatic MLX embeddings
    pub async fn ingest_with_embeddings(&self, collection_id: Uuid, documents: Vec<Document>) -> Result<IngestResponse> {
        let client = self.http_client();
        let response = client
            .post(&format!("{}/api/v2/collections/{}/ingest", self.base_url(), collection_id))
            .json(&json!({ "documents": documents, "embed": true }))
            .send()
            .await?;

        Ok(response.json().await?)
    }

    /// Query collection
    pub async fn query(&self, collection_id: Uuid, request: QueryRequest) -> Result<QueryResponse> {
        let client = self.http_client();
        let response = client
            .post(&format!("{}/api/v2/collections/{}/query", self.base_url(), collection_id))
            .json(&request)
            .send()
            .await?;

        Ok(response.json().await?)
    }

    /// Kill a specific service (for chaos testing)
    pub async fn kill_service(&mut self, name: &str) -> Result<()> {
        if let Some(mut service) = self.services.remove(name) {
            service.kill().await?;
        }
        Ok(())
    }

    /// Restart a service
    pub async fn restart_service(&mut self, name: &str) -> Result<()> {
        self.kill_service(name).await?;
        tokio::time::sleep(Duration::from_secs(1)).await;

        let service = match name {
            "api" => start_akidb_api(&self.config).await?,
            "embed" => start_akidb_embed(&self.config).await?,
            _ => return Err(Error::UnknownService(name.to_string())),
        };

        self.services.insert(name.to_string(), service);
        Ok(())
    }

    /// Simulate network partition between two services
    pub async fn network_partition(&self, service1: &str, service2: &str) -> Result<()> {
        // Use iptables/pfctl to block traffic between services
        #[cfg(target_os = "macos")]
        {
            // macOS: Use pfctl packet filter
            let rule = format!("block drop from {} to {}", service1, service2);
            Command::new("pfctl").args(&["-e", "-f", &rule]).output()?;
        }
        Ok(())
    }

    /// Heal network partition
    pub async fn network_heal(&self) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            Command::new("pfctl").args(&["-d"]).output()?;
        }
        Ok(())
    }

    /// Get Prometheus metrics
    pub async fn scrape_metrics(&self) -> Result<Metrics> {
        let client = self.http_client();
        let response = client.get(&format!("{}/metrics", self.base_url())).send().await?;
        let text = response.text().await?;

        Metrics::parse_prometheus(&text)
    }

    /// Get service logs
    pub async fn get_logs(&self) -> Vec<LogEntry> {
        // Read from log files or capture stdout/stderr
        Vec::new()  // Implementation...
    }

    /// Health check all services
    pub async fn health_check(&self) -> Result<HealthStatus> {
        let mut statuses = HashMap::new();

        for (name, service) in &self.services {
            let health = service.health_check().await?;
            statuses.insert(name.clone(), health);
        }

        if statuses.values().all(|h| h.is_healthy()) {
            Ok(HealthStatus::Healthy)
        } else if statuses.values().any(|h| h.is_healthy()) {
            Ok(HealthStatus::Degraded)
        } else {
            Ok(HealthStatus::Unhealthy)
        }
    }

    /// Shutdown cluster and cleanup resources
    pub async fn shutdown(mut self) -> Result<()> {
        for (name, mut service) in self.services.drain() {
            info!("Stopping service: {}", name);
            service.kill().await?;
        }

        if let Some(minio) = self.minio.take() {
            minio.stop().await?;
        }

        // Clean up temp directory
        fs::remove_dir_all(&self.config.data_dir).ok();

        Ok(())
    }

    fn base_url(&self) -> String {
        format!("http://{}", self.config.bind_address)
    }

    fn http_client(&self) -> reqwest::Client {
        reqwest::Client::new()
    }
}

// Implement Drop to ensure cleanup
impl Drop for TestCluster {
    fn drop(&mut self) {
        // Best-effort cleanup if user forgets to call shutdown()
        for (_, service) in self.services.drain() {
            let _ = futures::executor::block_on(service.kill());
        }
    }
}
```

**Usage Example:**

```rust
// tests/integration/basic_flow.rs

#[tokio::test]
async fn test_basic_ingest_query_flow() {
    let cluster = TestCluster::start().await.unwrap();

    // Create tenant, database, collection
    let tenant = cluster.create_tenant("test-org").await.unwrap();
    let database = cluster.create_database(tenant.id, "documents").await.unwrap();
    let collection = cluster.create_collection(database.id, CollectionConfig {
        name: "articles".to_string(),
        vector_dim: 512,
        distance: DistanceMetric::Cosine,
        hnsw_params: HnswParams { M: 16, ef_construction: 200 },
    }).await.unwrap();

    // Ingest documents with MLX embeddings
    let documents = vec![
        Document { id: "1".to_string(), text: "Apple Silicon performance".to_string(), metadata: json!({"category": "tech"}) },
        Document { id: "2".to_string(), text: "MLX framework benchmarks".to_string(), metadata: json!({"category": "ml"}) },
    ];

    let ingest_result = cluster.ingest_with_embeddings(collection.id, documents).await.unwrap();
    assert_eq!(ingest_result.ingested_count, 2);

    // Query
    let query_result = cluster.query(collection.id, QueryRequest {
        query_text: Some("M3 benchmarks".to_string()),
        top_k: 10,
        filter: None,
    }).await.unwrap();

    assert_eq!(query_result.matches.len(), 2);
    assert!(query_result.latency_ms < 25.0);

    cluster.shutdown().await.unwrap();
}
```

### 1.3 Implementation Plan

**Phase 1 (Week 1-2):**
- [ ] Core `TestCluster` struct and lifecycle management
- [ ] Service start/stop with health checks
- [ ] Basic API wrappers (tenant, database, collection CRUD)

**Phase 2 (Week 3-4):**
- [ ] Ingest and query operations
- [ ] MinIO container integration
- [ ] Prometheus container integration

**Phase 3 (Week 5-8):**
- [ ] Chaos engineering primitives (kill, restart, network partition)
- [ ] Resource limit enforcement
- [ ] Log and metrics collection

**Phase 4 (Week 9-12):**
- [ ] Advanced features (circuit breaker testing, failover scenarios)
- [ ] Performance profiling integration
- [ ] Documentation and examples

---

## 2. Data Generator (`akidb-datagen`)

### 2.1 Purpose

Generate synthetic test data (vectors, documents, metadata, tenants) for load testing, performance benchmarking, and regression testing.

### 2.2 Features

**Capabilities:**
- [x] Generate random vectors (f32, normalized, with configurable distributions)
- [x] Generate realistic documents (titles, bodies, metadata)
- [x] Generate tenant/user/role hierarchies
- [x] Export to CSV, JSON, Parquet formats
- [x] Configurable data distributions (uniform, normal, zipf)
- [x] Reproducible seeds for deterministic generation

**CLI Design:**

```bash
# Generate 1M random vectors (512-dim, normalized, cosine distance)
akidb-datagen vectors \
  --count 1000000 \
  --dim 512 \
  --normalize \
  --distribution normal \
  --seed 42 \
  --output vectors_1m_512d.csv

# Generate realistic documents with metadata
akidb-datagen documents \
  --count 10000 \
  --categories tech,science,business \
  --min-words 50 \
  --max-words 500 \
  --output documents_10k.json

# Generate tenant hierarchy (3 tenants × 10 users × 5 roles)
akidb-datagen tenants \
  --count 3 \
  --users-per-tenant 10 \
  --roles admin,developer,viewer,auditor,support \
  --output tenants.json

# Generate query workload (60% vector, 30% metadata, 10% hybrid)
akidb-datagen workload \
  --queries 10000 \
  --vector-ratio 0.6 \
  --metadata-ratio 0.3 \
  --hybrid-ratio 0.1 \
  --output queries.json
```

**Rust API:**

```rust
// akidb-datagen/src/lib.rs

pub struct VectorGenerator {
    dim: usize,
    distribution: Distribution,
    normalize: bool,
    rng: ChaCha8Rng,  // Deterministic RNG
}

impl VectorGenerator {
    pub fn new(dim: usize, seed: u64) -> Self {
        Self {
            dim,
            distribution: Distribution::Normal { mean: 0.0, std: 1.0 },
            normalize: false,
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }

    pub fn with_distribution(mut self, dist: Distribution) -> Self {
        self.distribution = dist;
        self
    }

    pub fn with_normalization(mut self) -> Self {
        self.normalize = true;
        self
    }

    pub fn generate(&mut self) -> Vec<f32> {
        let mut vector: Vec<f32> = match self.distribution {
            Distribution::Normal { mean, std } => {
                (0..self.dim).map(|_| {
                    let normal = Normal::new(mean, std).unwrap();
                    normal.sample(&mut self.rng) as f32
                }).collect()
            }
            Distribution::Uniform { min, max } => {
                (0..self.dim).map(|_| {
                    self.rng.gen_range(min..max)
                }).collect()
            }
        };

        if self.normalize {
            let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
            vector.iter_mut().for_each(|x| *x /= norm);
        }

        vector
    }

    pub fn generate_batch(&mut self, count: usize) -> Vec<Vec<f32>> {
        (0..count).map(|_| self.generate()).collect()
    }
}

pub struct DocumentGenerator {
    categories: Vec<String>,
    word_range: std::ops::Range<usize>,
    corpus: Vec<String>,  // Pre-loaded word corpus
    rng: ChaCha8Rng,
}

impl DocumentGenerator {
    pub fn new(categories: Vec<String>, word_range: std::ops::Range<usize>, seed: u64) -> Self {
        let corpus = Self::load_corpus();  // Load from embedded resource
        Self {
            categories,
            word_range,
            corpus,
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }

    pub fn generate(&mut self) -> Document {
        let category = self.categories.choose(&mut self.rng).unwrap();
        let word_count = self.rng.gen_range(self.word_range.clone());

        let title = self.generate_title();
        let body = self.generate_text(word_count);
        let metadata = json!({
            "category": category,
            "word_count": word_count,
            "timestamp": Utc::now().to_rfc3339(),
        });

        Document {
            id: Uuid::new_v4().to_string(),
            text: format!("{}\n\n{}", title, body),
            metadata,
        }
    }

    fn generate_title(&mut self) -> String {
        let words: Vec<_> = self.corpus.choose_multiple(&mut self.rng, 5).cloned().collect();
        words.join(" ").to_title_case()
    }

    fn generate_text(&mut self, word_count: usize) -> String {
        let words: Vec<_> = self.corpus.choose_multiple(&mut self.rng, word_count).cloned().collect();
        words.join(" ")
    }

    fn load_corpus() -> Vec<String> {
        // Embedded English word corpus (10k most common words)
        include_str!("../data/corpus.txt")
            .lines()
            .map(String::from)
            .collect()
    }
}

pub struct TenantGenerator {
    tenant_names: Vec<String>,
    roles: Vec<String>,
    users_per_tenant: usize,
    rng: ChaCha8Rng,
}

impl TenantGenerator {
    pub fn new(tenant_names: Vec<String>, roles: Vec<String>, users_per_tenant: usize, seed: u64) -> Self {
        Self {
            tenant_names,
            roles,
            users_per_tenant,
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }

    pub fn generate(&mut self) -> Vec<TenantDescriptor> {
        self.tenant_names.iter().map(|name| {
            let users = self.generate_users(name);
            TenantDescriptor {
                tenant_id: Uuid::new_v4(),
                name: name.clone(),
                status: TenantStatus::Active,
                quotas: TenantQuota::default(),
                users,
                created_at: Utc::now(),
            }
        }).collect()
    }

    fn generate_users(&mut self, tenant_name: &str) -> Vec<UserDescriptor> {
        (0..self.users_per_tenant).map(|i| {
            let role = self.roles.choose(&mut self.rng).unwrap();
            UserDescriptor {
                user_id: Uuid::new_v4(),
                tenant_id: Uuid::new_v4(),  // Will be set by caller
                email: format!("{}-user-{}@{}.akidb.dev", tenant_name, i + 1, tenant_name),
                role: role.clone(),
                created_at: Utc::now(),
            }
        }).collect()
    }
}
```

### 2.3 Implementation Plan

**Phase 1 (Week 1-2):**
- [ ] Vector generator with distributions (normal, uniform)
- [ ] CSV/JSON export
- [ ] CLI scaffolding

**Phase 2 (Week 3-4):**
- [ ] Document generator with realistic text
- [ ] Metadata generation (categories, timestamps)
- [ ] Parquet export support

**Phase 3 (Week 5-6):**
- [ ] Tenant/user/role hierarchy generator
- [ ] Workload generator (query patterns)
- [ ] Integration with test harness

---

## 3. Benchmarking Suite (`akidb-bench`)

### 3.1 Purpose

Comprehensive performance benchmarking for all AkiDB components, with automated regression detection and reporting.

### 3.2 Features

**Benchmarks:**
- [x] MLX embedding throughput (vectors/sec)
- [x] HNSW index build time (vectors, construction latency)
- [x] HNSW query latency (P50/P95/P99 at various scales)
- [x] Ingest throughput (vectors/sec, with/without embeddings)
- [x] SQLite metadata operations (CRUD, FTS5 search)
- [x] Cedar policy evaluation (latency at 1k/10k policy scale)
- [x] Storage layer (WAL write, S3 sync, snapshot creation)
- [x] API layer (REST vs gRPC latency comparison)

**CLI Design:**

```bash
# Run all benchmarks
akidb-bench all --output results/

# Run specific benchmark
akidb-bench mlx-embedding \
  --model mlx-community/qwen3-embedding-8b-int8 \
  --batch-sizes 1,10,100,1000 \
  --iterations 100

# Compare vs baseline
akidb-bench compare \
  --baseline results/v1.x-baseline.json \
  --current results/v2.0-latest.json \
  --alert-threshold 10  # Alert if >10% regression
```

**Implementation (Criterion Integration):**

```rust
// benches/mlx_embedding.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_mlx_embedding_throughput(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let service = rt.block_on(setup_mlx_service());

    let mut group = c.benchmark_group("mlx_embedding");

    for batch_size in [1, 10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("batch_size", batch_size),
            batch_size,
            |b, &batch_size| {
                b.iter(|| {
                    let texts: Vec<String> = (0..batch_size)
                        .map(|i| format!("Test document {}", i))
                        .collect();
                    rt.block_on(service.embed_batch(black_box(texts), 100))
                });
            },
        );
    }

    group.finish();
}

fn bench_hnsw_query_latency(c: &mut Criterion) {
    let index = build_test_index(1_000_000, 512);  // 1M vectors, 512-dim
    let query = generate_random_vector(512);

    let mut group = c.benchmark_group("hnsw_query");

    for ef_search in [32, 64, 128, 256].iter() {
        group.bench_with_input(
            BenchmarkId::new("ef_search", ef_search),
            ef_search,
            |b, &ef_search| {
                b.iter(|| {
                    index.search(black_box(&query), 10, ef_search)
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_mlx_embedding_throughput, bench_hnsw_query_latency);
criterion_main!(benches);
```

### 3.3 Regression Detection

**Automated Alerts:**

```rust
// akidb-bench/src/regression.rs

pub struct RegressionDetector {
    baseline: BenchmarkResults,
    threshold: f64,  // e.g., 0.10 for 10% regression
}

impl RegressionDetector {
    pub fn detect(&self, current: &BenchmarkResults) -> Vec<Regression> {
        let mut regressions = Vec::new();

        for (name, current_result) in &current.benchmarks {
            if let Some(baseline_result) = self.baseline.benchmarks.get(name) {
                let change = (current_result.mean - baseline_result.mean) / baseline_result.mean;

                if change > self.threshold {
                    regressions.push(Regression {
                        benchmark: name.clone(),
                        baseline_mean: baseline_result.mean,
                        current_mean: current_result.mean,
                        change_percent: change * 100.0,
                        severity: Self::classify_severity(change),
                    });
                }
            }
        }

        regressions
    }

    fn classify_severity(change: f64) -> Severity {
        if change > 0.50 { Severity::Critical }
        else if change > 0.25 { Severity::High }
        else if change > 0.10 { Severity::Medium }
        else { Severity::Low }
    }
}

pub struct Regression {
    pub benchmark: String,
    pub baseline_mean: f64,
    pub current_mean: f64,
    pub change_percent: f64,
    pub severity: Severity,
}

pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}
```

---

## 4. Chaos Engineering Framework (`akidb-chaos`)

### 4.1 Purpose

Systematic failure injection and resilience testing for AkiDB components.

### 4.2 Features

**Chaos Scenarios:**
- [x] Process crashes (kill, SIGKILL, SIGTERM)
- [x] Network partitions (split-brain, asymmetric)
- [x] Resource exhaustion (OOM, disk full, CPU saturation)
- [x] Latency injection (slow network, slow disk I/O)
- [x] File corruption (SQLite, WAL, index files)
- [x] Clock skew (time drift between components)

**Framework Design:**

```rust
// akidb-chaos/src/lib.rs

pub struct ChaosScenario {
    pub name: String,
    pub failure: FailureType,
    pub duration: Duration,
    pub validation: Box<dyn Fn(&TestCluster) -> Result<bool>>,
}

pub enum FailureType {
    ProcessCrash { service: String, signal: Signal },
    NetworkPartition { from: String, to: String },
    FileCorruption { path: PathBuf, corruption_type: CorruptionType },
    ResourceExhaustion { resource: ResourceType, limit: u64 },
    LatencyInjection { service: String, latency_ms: u64 },
}

pub enum Signal {
    SIGKILL,   // Immediate termination (no cleanup)
    SIGTERM,   // Graceful shutdown
}

pub enum CorruptionType {
    RandomBytes { count: usize },
    Truncate { at_byte: usize },
    ZeroOut { start: usize, end: usize },
}

pub enum ResourceType {
    Memory,
    DiskSpace,
    CPU,
}

impl ChaosScenario {
    pub async fn run(&self, cluster: &mut TestCluster) -> Result<ChaosReport> {
        info!("Running chaos scenario: {}", self.name);

        // Record initial state
        let initial_state = self.capture_state(cluster).await?;

        // Inject failure
        self.failure.inject(cluster).await?;

        // Wait for specified duration
        tokio::time::sleep(self.duration).await;

        // Validate system behavior
        let validation_passed = (self.validation)(cluster)?;

        // Capture final state
        let final_state = self.capture_state(cluster).await?;

        Ok(ChaosReport {
            scenario: self.name.clone(),
            initial_state,
            final_state,
            validation_passed,
            duration: self.duration,
        })
    }

    async fn capture_state(&self, cluster: &TestCluster) -> Result<SystemState> {
        Ok(SystemState {
            health: cluster.health_check().await?,
            metrics: cluster.scrape_metrics().await?,
            logs: cluster.get_logs().await,
        })
    }
}

// Example usage
pub fn embedding_service_crash_scenario() -> ChaosScenario {
    ChaosScenario {
        name: "Embedding Service Crash".to_string(),
        failure: FailureType::ProcessCrash {
            service: "akidb-embed".to_string(),
            signal: Signal::SIGKILL,
        },
        duration: Duration::from_secs(30),
        validation: Box::new(|cluster| {
            // Validate circuit breaker opened
            let cb_status = cluster.get_circuit_breaker_status("embedding")?;
            Ok(cb_status.state == CircuitBreakerState::Open)
        }),
    }
}
```

### 4.3 Chaos Test Suite

```rust
// akidb-chaos/tests/suite.rs

#[tokio::test]
async fn test_chaos_suite() {
    let scenarios = vec![
        embedding_service_crash_scenario(),
        s3_network_partition_scenario(),
        metadata_corruption_scenario(),
        memory_exhaustion_scenario(),
        cedar_latency_injection_scenario(),
    ];

    for scenario in scenarios {
        let mut cluster = TestCluster::start().await.unwrap();
        let report = scenario.run(&mut cluster).await.unwrap();

        assert!(
            report.validation_passed,
            "Chaos scenario failed: {} - validation did not pass",
            report.scenario
        );

        cluster.shutdown().await.unwrap();
    }
}
```

---

## 5. Test Orchestration (`akidb-testkube`)

### 5.1 Purpose

Centralized test execution, reporting, and visualization for CI/CD integration.

### 5.2 Features

**Capabilities:**
- [x] Test suite organization (unit, integration, E2E, performance, chaos)
- [x] Parallel test execution with resource management
- [x] Test result aggregation and reporting
- [x] Historical trend analysis
- [x] Slack/email notifications on failures
- [x] Dashboard with live test status

**CLI Design:**

```bash
# Run all test suites
akidb-testkube run --all

# Run specific suite
akidb-testkube run --suite integration

# Run with parallelism
akidb-testkube run --all --parallel 4

# Generate report
akidb-testkube report --format html --output test-report.html

# Start dashboard server
akidb-testkube dashboard --port 8080
```

**Dashboard (Web UI):**

```
┌─────────────────────────────────────────────────┐
│ AkiDB 2.0 Test Dashboard                       │
├─────────────────────────────────────────────────┤
│                                                  │
│ ✅ Unit Tests: 487/487 (100%)                   │
│ ✅ Integration Tests: 45/45 (100%)              │
│ ✅ E2E Tests: 12/12 (100%)                      │
│ ✅ Performance Tests: 8/8 (100%)                │
│ ✅ Chaos Tests: 5/5 (100%)                      │
│                                                  │
│ Code Coverage: 82%                               │
│ Performance: All targets met                     │
│ Last Run: 2 minutes ago                          │
│                                                  │
│ [View Detailed Report] [Historical Trends]      │
└─────────────────────────────────────────────────┘
```

---

## 6. Test Fixtures (`akidb-fixtures`)

### 6.1 Purpose

Curated, version-controlled test datasets and scenarios for consistent testing across the team.

### 6.2 Structure

```
akidb-fixtures/
├── vectors/
│   ├── random_1k_512d.csv          # 1k random vectors, 512-dim
│   ├── random_10k_512d.csv
│   ├── random_100k_512d.parquet
│   ├── random_1m_512d.parquet
│   └── normalized_1m_512d.parquet
├── documents/
│   ├── tech_articles_1k.json       # 1k tech articles
│   ├── ml_papers_500.json          # 500 ML paper abstracts
│   └── mixed_corpus_10k.json
├── tenants/
│   ├── single_tenant.json          # 1 tenant, 5 users
│   ├── multi_tenant_3.json         # 3 tenants, 10 users each
│   └── enterprise_hierarchy.json   # Complex tenant/role hierarchy
├── queries/
│   ├── simple_queries_100.json     # 100 simple similarity searches
│   ├── filtered_queries_100.json   # Metadata filters
│   └── hybrid_queries_100.json     # Vector + filter
└── scenarios/
    ├── basic_flow.yaml             # New user onboarding
    ├── multi_tenant_isolation.yaml
    └── embedding_e2e.yaml
```

### 6.3 Usage

```rust
// tests/integration/using_fixtures.rs

use akidb_fixtures::Fixtures;

#[tokio::test]
async fn test_with_fixtures() {
    let fixtures = Fixtures::load();

    let cluster = TestCluster::start().await.unwrap();
    let tenant = cluster.create_tenant("test").await.unwrap();
    let database = cluster.create_database(tenant.id, "docs").await.unwrap();
    let collection = cluster.create_collection(database.id, CollectionConfig::default()).await.unwrap();

    // Load pre-generated vectors
    let vectors = fixtures.vectors("random_1k_512d.csv").unwrap();
    cluster.ingest_vectors(collection.id, vectors).await.unwrap();

    // Load pre-generated queries
    let queries = fixtures.queries("simple_queries_100.json").unwrap();
    for query in queries {
        let result = cluster.query(collection.id, query).await.unwrap();
        assert!(!result.matches.is_empty());
    }

    cluster.shutdown().await.unwrap();
}
```

---

## 7. Implementation Timeline

### Phase 1: Foundation (Week 1-4)
- [ ] `akidb-test-harness` core features
- [ ] `akidb-datagen` vector and document generators
- [ ] Basic CI integration

### Phase 2: Performance (Week 5-8)
- [ ] `akidb-bench` benchmarking suite
- [ ] Regression detection
- [ ] Performance dashboards

### Phase 3: Chaos (Week 9-12)
- [ ] `akidb-chaos` framework
- [ ] Chaos scenarios (crash, partition, corruption)
- [ ] Resilience validation

### Phase 4: Orchestration (Week 13-16)
- [ ] `akidb-testkube` test orchestration
- [ ] Web dashboard
- [ ] `akidb-fixtures` curated datasets
- [ ] Documentation and examples

---

## 8. Success Criteria

- [ ] `akidb-test-harness` used in 100% of integration tests
- [ ] `akidb-datagen` generates 1M vectors in <10 seconds
- [ ] `akidb-bench` detects regressions >10% automatically
- [ ] `akidb-chaos` validates 5 chaos scenarios
- [ ] `akidb-testkube` integrates with GitHub Actions CI
- [ ] Documentation complete with examples
- [ ] Developer onboarding time <1 hour (using tools)

---

## 9. Cost Analysis

**Engineering Effort:**
- Test Harness: 2 weeks (1 engineer)
- Data Generator: 1 week (1 engineer)
- Benchmarking Suite: 2 weeks (1 engineer)
- Chaos Framework: 2 weeks (1 engineer)
- Test Orchestration: 1 week (1 engineer)
- Fixtures & Docs: 1 week (1 engineer)

**Total:** 9 engineer-weeks (~$45k @ $5k/week)

**ROI:**
- **Time Savings:** 50% reduction in test writing time (2x productivity)
- **Quality Improvement:** 30% increase in bug detection (fewer production issues)
- **Cost Avoidance:** $100k/year in prevented outages (uptime improvement)

**Payback Period:** <3 months

---

## 10. Dependencies

- **Rust:** 1.75+ (stable)
- **MLX:** Python MLX bindings
- **External Services:** MinIO, Prometheus, Grafana
- **CI/CD:** GitHub Actions (ARM64 runners)

---

## Prepared By

**QA Engineering Team**
**DevTools Team**
**Date:** 2025-11-06
**Version:** 1.0
**Confidentiality:** Internal Use Only

---

**This PRD defines a comprehensive testing toolkit that enables efficient, reproducible, and automated testing for AkiDB 2.0 on macOS MLX.**
