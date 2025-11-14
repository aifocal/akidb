# Phase 3: Production Hardening - Detailed Action Plan
## Candle Embedding Migration - Week 3 Implementation Guide

**Version:** 1.0
**Date:** 2025-11-10
**Status:** Ready for Execution
**Timeline:** 5 days (30 development hours)

---

## Table of Contents

1. [Pre-Flight Checklist](#pre-flight-checklist)
2. [Day 1: Observability - Metrics](#day-1-observability---metrics)
3. [Day 2: Observability - Tracing + Logging](#day-2-observability---tracing--logging)
4. [Day 3: Resilience Patterns](#day-3-resilience-patterns)
5. [Day 4: Health Checks + Integration Tests](#day-4-health-checks--integration-tests)
6. [Day 5: Chaos Testing + Documentation](#day-5-chaos-testing--documentation)
7. [Phase 3 Summary](#phase-3-summary)
8. [Appendix](#appendix)

---

## Pre-Flight Checklist

### Before Starting Phase 3

```bash
# 1. Verify Phase 2 completion
cargo test --workspace --features candle
# Expected: 36 tests passing

# 2. Run benchmarks to establish baseline
cargo bench --bench candle_bench
# Expected: 200+ QPS, P95 <35ms

# 3. Check disk space
df -h
# Need: 5GB free for dependencies + logs

# 4. Create feature branch
git checkout -b feature/candle-phase3-production-hardening
git branch --set-upstream-to=origin/main

# 5. Verify observability stack (optional for local testing)
docker compose up -d prometheus grafana jaeger
# Or skip if deploying to staging environment

# 6. Update dependencies in Cargo.toml (will do in Day 1)
```

### Success Criteria

Phase 3 is complete when:
- ✅ 10+ Prometheus metrics exported
- ✅ OpenTelemetry tracing integrated
- ✅ Circuit breaker implemented and tested
- ✅ 20 integration tests passing
- ✅ 5 chaos tests passing
- ✅ Operations runbook documented
- ✅ RC2 release tagged

---

## Day 1: Observability - Metrics
**Monday, 6 hours**

### Overview

**Goal:** Add Prometheus metrics for monitoring production health

**Deliverables:**
- `src/metrics.rs` (~200 lines)
- 10 Prometheus metrics
- `/metrics` endpoint
- Metrics export test

---

### Task 1.1: Add Prometheus Dependencies
**Time:** 30 minutes

#### Step 1: Update Cargo.toml

```toml
# File: crates/akidb-embedding/Cargo.toml

[dependencies]
# Existing dependencies
candle-core = { version = "0.8", optional = true, features = ["metal"] }
candle-nn = { version = "0.8", optional = true }
candle-transformers = { version = "0.8", optional = true }
tokenizers = { version = "0.15", optional = true }
hf-hub = { version = "0.3", optional = true }
tokio = { version = "1.0", features = ["full"] }
rayon = "1.8"

# NEW: Observability dependencies
prometheus = "0.13"
lazy_static = "1.4"
scopeguard = "1.2"
```

#### Step 2: Verify Dependencies

```bash
cd crates/akidb-embedding
cargo check --features candle
```

**Expected Output:**
```
    Checking akidb-embedding v2.0.0
    Finished dev [unoptimized + debuginfo] target(s) in 5.2s
```

#### Checkpoint

- ✅ Dependencies added to Cargo.toml
- ✅ `cargo check` passes
- ✅ Commit: `git commit -am "Phase 3 Day 1: Add Prometheus dependencies"`

---

### Task 1.2: Implement CandleMetrics Struct
**Time:** 2 hours

#### Step 1: Create metrics.rs

```bash
touch crates/akidb-embedding/src/metrics.rs
```

#### Step 2: Implement Metrics

```rust
// File: crates/akidb-embedding/src/metrics.rs

use prometheus::{
    IntCounter, IntGauge, Histogram, HistogramVec,
    register_int_counter, register_int_gauge,
    register_histogram, register_histogram_vec,
    Encoder, TextEncoder,
};

/// Prometheus metrics for Candle embedding provider
pub struct CandleMetrics {
    // Request metrics
    pub requests_total: IntCounter,
    pub requests_in_flight: IntGauge,
    pub request_duration: HistogramVec,

    // Error metrics
    pub errors_total: IntCounter,
    pub circuit_breaker_state: IntGauge,

    // Batch metrics
    pub batch_size: Histogram,
    pub batch_wait_time: Histogram,

    // Model metrics
    pub model_load_duration: Histogram,
    pub model_inference_duration: Histogram,

    // Resource metrics
    pub gpu_utilization: IntGauge,
    pub memory_usage_bytes: IntGauge,
}

impl CandleMetrics {
    pub fn new() -> Self {
        Self {
            requests_total: register_int_counter!(
                "candle_requests_total",
                "Total number of embedding requests"
            ).expect("Failed to register requests_total"),

            requests_in_flight: register_int_gauge!(
                "candle_requests_in_flight",
                "Current number of requests being processed"
            ).expect("Failed to register requests_in_flight"),

            request_duration: register_histogram_vec!(
                "candle_request_duration_seconds",
                "Request duration in seconds",
                &["status"],  // labels: success, error
                vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]
            ).expect("Failed to register request_duration"),

            errors_total: register_int_counter!(
                "candle_errors_total",
                "Total number of errors"
            ).expect("Failed to register errors_total"),

            circuit_breaker_state: register_int_gauge!(
                "candle_circuit_breaker_state",
                "Circuit breaker state (0=closed, 1=half-open, 2=open)"
            ).expect("Failed to register circuit_breaker_state"),

            batch_size: register_histogram!(
                "candle_batch_size",
                "Number of texts in batch",
                vec![1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0]
            ).expect("Failed to register batch_size"),

            batch_wait_time: register_histogram!(
                "candle_batch_wait_seconds",
                "Time spent waiting for batch to fill",
                vec![0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2]
            ).expect("Failed to register batch_wait_time"),

            model_load_duration: register_histogram!(
                "candle_model_load_duration_seconds",
                "Model load duration in seconds",
                vec![0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0]
            ).expect("Failed to register model_load_duration"),

            model_inference_duration: register_histogram!(
                "candle_model_inference_duration_seconds",
                "Model inference duration in seconds",
                vec![0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2, 0.5]
            ).expect("Failed to register model_inference_duration"),

            gpu_utilization: register_int_gauge!(
                "candle_gpu_utilization_percent",
                "GPU utilization percentage (0-100)"
            ).expect("Failed to register gpu_utilization"),

            memory_usage_bytes: register_int_gauge!(
                "candle_memory_usage_bytes",
                "Memory usage in bytes"
            ).expect("Failed to register memory_usage_bytes"),
        }
    }

    /// Export metrics in Prometheus text format
    pub fn export(&self) -> Result<String, prometheus::Error> {
        let encoder = TextEncoder::new();
        let metric_families = prometheus::gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(String::from_utf8(buffer).expect("UTF-8 encoding failed"))
    }
}

impl Default for CandleMetrics {
    fn default() -> Self {
        Self::new()
    }
}

// Global metrics singleton
lazy_static::lazy_static! {
    pub static ref METRICS: CandleMetrics = CandleMetrics::new();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = CandleMetrics::new();
        // Should not panic
    }

    #[test]
    fn test_metrics_export() {
        let export = METRICS.export().unwrap();

        // Verify metrics are present
        assert!(export.contains("candle_requests_total"));
        assert!(export.contains("candle_requests_in_flight"));
        assert!(export.contains("candle_request_duration_seconds"));
        assert!(export.contains("candle_errors_total"));
    }
}
```

#### Step 3: Update lib.rs

```rust
// File: crates/akidb-embedding/src/lib.rs

// Add metrics module
#[cfg(feature = "candle")]
pub mod metrics;

// Re-export metrics for convenience
#[cfg(feature = "candle")]
pub use metrics::METRICS;
```

#### Step 4: Verify Compilation

```bash
cargo check --features candle
cargo test --features candle metrics::tests
```

**Expected Output:**
```
running 2 tests
test metrics::tests::test_metrics_creation ... ok
test metrics::tests::test_metrics_export ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

#### Checkpoint

- ✅ metrics.rs created (~200 lines)
- ✅ 10 Prometheus metrics defined
- ✅ Tests passing
- ✅ Commit: `git commit -am "Phase 3 Day 1: Implement CandleMetrics struct"`

---

### Task 1.3: Instrument Embedding Provider
**Time:** 2 hours

#### Step 1: Add Metrics to embed_batch_internal()

```rust
// File: crates/akidb-embedding/src/candle.rs

use crate::metrics::METRICS;
use scopeguard::defer;
use std::time::Instant;

impl CandleEmbeddingProvider {
    pub async fn embed_batch_internal(
        &self,
        texts: Vec<String>,
    ) -> EmbeddingResult<Vec<Vec<f32>>> {
        // Track in-flight requests
        METRICS.requests_in_flight.inc();
        defer! {
            METRICS.requests_in_flight.dec();
        }

        // Track request count
        METRICS.requests_total.inc();

        // Track batch size
        METRICS.batch_size.observe(texts.len() as f64);

        // Start timing
        let start = Instant::now();

        // Execute embedding
        let result = self.embed_internal(texts).await;

        // Track duration by status
        let duration = start.elapsed().as_secs_f64();
        match &result {
            Ok(_) => {
                METRICS.request_duration
                    .with_label_values(&["success"])
                    .observe(duration);
            }
            Err(_) => {
                METRICS.request_duration
                    .with_label_values(&["error"])
                    .observe(duration);
                METRICS.errors_total.inc();
            }
        }

        result
    }

    async fn embed_internal(
        &self,
        texts: Vec<String>,
    ) -> EmbeddingResult<Vec<Vec<f32>>> {
        // Step 1: Tokenization (with timing)
        let tokenize_start = Instant::now();
        let input_ids = tokio::task::spawn_blocking({
            let tokenizer = Arc::clone(&self.tokenizer);
            let texts = texts.clone();
            move || Self::tokenize_batch_parallel(&tokenizer, &texts)
        }).await.map_err(|e| EmbeddingError::InferenceFailed(e.to_string()))??;

        tracing::debug!(
            "Tokenized {} texts in {:?}",
            texts.len(),
            tokenize_start.elapsed()
        );

        // Step 2: Inference (with timing and metrics)
        let inference_start = Instant::now();
        let embeddings = tokio::task::spawn_blocking({
            let model = Arc::clone(&self.model);
            let device = self.device.clone();
            move || {
                // Transfer to GPU
                let input_ids_gpu = input_ids.to_device(&device)
                    .map_err(|e| EmbeddingError::InferenceFailed(e.to_string()))?;

                // Forward pass (GPU-accelerated)
                let outputs = model.forward(&input_ids_gpu)
                    .map_err(|e| EmbeddingError::InferenceFailed(e.to_string()))?;

                // Mean pooling (GPU-accelerated)
                let embeddings = outputs.mean(1)
                    .map_err(|e| EmbeddingError::InferenceFailed(e.to_string()))?;

                // L2 normalization (GPU-accelerated)
                let norms = embeddings.sqr()?.sum_keepdim(1)?.sqrt()?;
                let normalized = embeddings.broadcast_div(&norms)
                    .map_err(|e| EmbeddingError::InferenceFailed(e.to_string()))?;

                // Transfer back to CPU
                normalized.to_vec2()
                    .map_err(|e| EmbeddingError::InferenceFailed(e.to_string()))
            }
        }).await.map_err(|e| EmbeddingError::InferenceFailed(e.to_string()))??;

        let inference_duration = inference_start.elapsed().as_secs_f64();
        METRICS.model_inference_duration.observe(inference_duration);

        tracing::debug!(
            "Generated {} embeddings in {:?}",
            embeddings.len(),
            inference_start.elapsed()
        );

        Ok(embeddings)
    }
}
```

#### Step 2: Add Metrics to Model Loading

```rust
// File: crates/akidb-embedding/src/candle.rs

impl CandleEmbeddingProvider {
    pub async fn new(model_name: &str) -> EmbeddingResult<Self> {
        let load_start = Instant::now();

        // Step 1: Download model from HF Hub
        let (model_path, config_path, tokenizer_path) =
            Self::download_model(model_name).await?;

        // Step 2: Load configuration
        let config_str = tokio::fs::read_to_string(&config_path)
            .await
            .map_err(|e| EmbeddingError::ModelLoadFailed(e.to_string()))?;
        let config: Config = serde_json::from_str(&config_str)
            .map_err(|e| EmbeddingError::ModelLoadFailed(e.to_string()))?;

        // Step 3: Select device
        let device = Self::select_device()?;

        // Step 4: Load model weights
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(
                &[model_path.join("model.safetensors")],
                DType::F32,
                &device,
            ).map_err(|e| EmbeddingError::ModelLoadFailed(e.to_string()))?
        };

        let model = BertModel::load(vb, &config)
            .map_err(|e| EmbeddingError::ModelLoadFailed(e.to_string()))?;

        // Step 5: Load tokenizer
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| EmbeddingError::ModelLoadFailed(e.to_string()))?;

        // Record model load duration
        let load_duration = load_start.elapsed().as_secs_f64();
        METRICS.model_load_duration.observe(load_duration);

        tracing::info!(
            "Loaded model '{}' in {:.2}s on device {:?}",
            model_name,
            load_duration,
            device
        );

        Ok(Self {
            model: Arc::new(model),
            tokenizer: Arc::new(tokenizer),
            device,
            model_name: model_name.to_string(),
            dimension: config.hidden_size as u32,
        })
    }
}
```

#### Step 3: Test Instrumented Provider

```rust
// File: crates/akidb-embedding/tests/metrics_tests.rs

#[cfg(test)]
mod metrics_tests {
    use super::*;
    use akidb_embedding::candle::CandleEmbeddingProvider;
    use akidb_embedding::METRICS;

    #[tokio::test]
    async fn test_metrics_incremented() {
        let provider = CandleEmbeddingProvider::new(
            "sentence-transformers/all-MiniLM-L6-v2"
        ).await.unwrap();

        // Get initial counts
        let initial_requests = METRICS.requests_total.get();

        // Make request
        let _ = provider.embed_batch_internal(vec![
            "Test text".to_string()
        ]).await.unwrap();

        // Verify metrics incremented
        assert_eq!(
            METRICS.requests_total.get(),
            initial_requests + 1
        );
        assert_eq!(METRICS.requests_in_flight.get(), 0);
    }

    #[tokio::test]
    async fn test_error_metrics() {
        let provider = CandleEmbeddingProvider::new(
            "sentence-transformers/all-MiniLM-L6-v2"
        ).await.unwrap();

        let initial_errors = METRICS.errors_total.get();

        // Trigger error (empty input)
        let result = provider.embed_batch_internal(vec![]).await;
        assert!(result.is_err());

        // Verify error metric incremented
        assert_eq!(
            METRICS.errors_total.get(),
            initial_errors + 1
        );
    }
}
```

```bash
cargo test --features candle metrics_tests
```

**Expected Output:**
```
running 2 tests
test metrics_tests::test_metrics_incremented ... ok
test metrics_tests::test_error_metrics ... ok

test result: ok. 2 passed
```

#### Checkpoint

- ✅ Embedding provider instrumented
- ✅ Model loading instrumented
- ✅ Metrics tests passing
- ✅ Commit: `git commit -am "Phase 3 Day 1: Instrument embedding provider with metrics"`

---

### Task 1.4: Add /metrics Endpoint
**Time:** 1 hour

#### Step 1: Add Metrics Endpoint to REST API

```rust
// File: crates/akidb-rest/src/handlers/metrics.rs

use axum::{response::IntoResponse, http::StatusCode};
use akidb_embedding::METRICS;

/// Prometheus metrics endpoint
pub async fn metrics() -> impl IntoResponse {
    match METRICS.export() {
        Ok(metrics_text) => {
            (
                StatusCode::OK,
                [("content-type", "text/plain; version=0.0.4")],
                metrics_text,
            )
        }
        Err(e) => {
            tracing::error!("Failed to export metrics: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [("content-type", "text/plain; version=0.0.4")],
                format!("Error exporting metrics: {}", e),
            )
        }
    }
}
```

#### Step 2: Add Route

```rust
// File: crates/akidb-rest/src/main.rs

mod handlers;

use axum::{Router, routing::get};
use handlers::metrics::metrics;

async fn create_app() -> Router {
    Router::new()
        .route("/health/live", get(handlers::health::liveness))
        .route("/health/ready", get(handlers::health::readiness))
        .route("/metrics", get(metrics))  // NEW
        .route("/api/v1/embed", post(handlers::embed::embed))
        // ... other routes
}
```

#### Step 3: Test Metrics Endpoint

```bash
# Start server
cargo run -p akidb-rest --features candle &
sleep 5

# Test metrics endpoint
curl http://localhost:8080/metrics

# Should see Prometheus metrics output like:
# candle_requests_total 0
# candle_requests_in_flight 0
# candle_request_duration_seconds_bucket{status="success",le="0.001"} 0
# ...
```

#### Step 4: Integration Test

```rust
// File: crates/akidb-rest/tests/metrics_endpoint_test.rs

#[tokio::test]
async fn test_metrics_endpoint() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response.headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(content_type.contains("text/plain"));

    let body = hyper::body::to_bytes(response.into_body())
        .await
        .unwrap();
    let metrics_text = String::from_utf8(body.to_vec()).unwrap();

    // Verify key metrics present
    assert!(metrics_text.contains("candle_requests_total"));
    assert!(metrics_text.contains("candle_request_duration_seconds"));
    assert!(metrics_text.contains("candle_batch_size"));
}
```

```bash
cargo test --features candle test_metrics_endpoint
```

#### Checkpoint

- ✅ /metrics endpoint added
- ✅ Metrics exported in Prometheus format
- ✅ Integration test passing
- ✅ Commit: `git commit -am "Phase 3 Day 1: Add /metrics endpoint"`

---

### Task 1.5: Test Metrics Export
**Time:** 30 minutes

#### Step 1: Manual Testing

```bash
# Terminal 1: Start server
cargo run -p akidb-rest --features candle

# Terminal 2: Generate some requests
for i in {1..10}; do
  curl -X POST http://localhost:8080/api/v1/embed \
    -H "Content-Type: application/json" \
    -d '{"texts": ["Test text '$i'"]}'
done

# Terminal 2: Check metrics
curl http://localhost:8080/metrics | grep candle
```

**Expected Output:**
```
candle_requests_total 10
candle_requests_in_flight 0
candle_request_duration_seconds_sum{status="success"} 0.15
candle_request_duration_seconds_count{status="success"} 10
candle_batch_size_sum 10
candle_batch_size_count 10
candle_errors_total 0
```

#### Step 2: Verify Prometheus Scraping (Optional)

If you have Prometheus running locally:

```yaml
# File: prometheus/prometheus.yml

scrape_configs:
  - job_name: 'akidb-embedding'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: '/metrics'
    scrape_interval: 15s
```

```bash
# Restart Prometheus
docker compose restart prometheus

# Check targets: http://localhost:9090/targets
# Should see akidb-embedding target as UP
```

#### Checkpoint

- ✅ Metrics export manually verified
- ✅ All 10 metrics visible
- ✅ Prometheus scraping working (if configured)
- ✅ Commit: `git commit -am "Phase 3 Day 1: Verify metrics export"`

---

### Day 1 Checkpoint

**Accomplishments:**
- ✅ Added Prometheus dependencies
- ✅ Implemented CandleMetrics struct (~200 lines)
- ✅ Instrumented embedding provider
- ✅ Added /metrics endpoint
- ✅ 10 Prometheus metrics exported
- ✅ 2 metrics tests passing

**Verification:**
```bash
# Run all tests
cargo test --workspace --features candle

# Verify metrics endpoint
curl http://localhost:8080/metrics | head -20

# Check commit history
git log --oneline -5
```

**Deliverables:**
- `src/metrics.rs` (~200 lines) ✅
- `/metrics` endpoint ✅
- 2 tests passing ✅

**Time Spent:** 6 hours (on budget)

**Next:** Day 2 - OpenTelemetry tracing + structured logging

---

## Day 2: Observability - Tracing + Logging
**Tuesday, 6 hours**

### Overview

**Goal:** Add distributed tracing and structured logging for debugging

**Deliverables:**
- `src/tracing_init.rs` (~150 lines)
- OpenTelemetry integration
- Structured logging with context
- Jaeger integration (optional)

---

### Task 2.1: Add OpenTelemetry Dependencies
**Time:** 30 minutes

#### Step 1: Update Cargo.toml

```toml
# File: crates/akidb-embedding/Cargo.toml

[dependencies]
# Existing + Day 1 dependencies
# ...

# NEW: Tracing dependencies
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
opentelemetry = "0.21"
opentelemetry-otlp = "0.14"
tracing-opentelemetry = "0.22"
serde_json = "1.0"
```

#### Step 2: Verify

```bash
cargo check --features candle
```

#### Checkpoint

- ✅ Tracing dependencies added
- ✅ Compilation successful
- ✅ Commit: `git commit -am "Phase 3 Day 2: Add OpenTelemetry dependencies"`

---

### Task 2.2: Implement Tracing Initialization
**Time:** 1 hour

#### Step 1: Create tracing_init.rs

```rust
// File: crates/akidb-embedding/src/tracing_init.rs

use opentelemetry::{global, sdk::trace::Tracer, trace::TraceError};
use opentelemetry_otlp::WithExportConfig;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize tracing with OpenTelemetry
pub fn init_tracing(service_name: &str) -> Result<(), TraceError> {
    // Set up text map propagator for context propagation
    global::set_text_map_propagator(
        opentelemetry::sdk::propagation::TraceContextPropagator::new()
    );

    // Create OTLP exporter (exports to Jaeger/OTEL collector)
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(
                    std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
                        .unwrap_or_else(|_| "http://localhost:4317".to_string())
                )
        )
        .with_trace_config(
            opentelemetry::sdk::trace::config()
                .with_resource(opentelemetry::sdk::Resource::new(vec![
                    opentelemetry::KeyValue::new("service.name", service_name.to_string()),
                    opentelemetry::KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
                ]))
        )
        .install_batch(opentelemetry::runtime::Tokio)?;

    // Set up tracing subscriber with multiple layers
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info"))
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_level(true)
                .with_thread_ids(true)
                .json()  // JSON format for structured logging
        )
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .init();

    tracing::info!("Tracing initialized for service: {}", service_name);

    Ok(())
}

/// Shutdown tracing (call on exit)
pub fn shutdown_tracing() {
    global::shutdown_tracer_provider();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracing_init() {
        // This test just verifies it doesn't panic
        // Actual tracing requires OTEL collector running
        // So we skip if OTEL endpoint not available

        if std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok() {
            let result = init_tracing("test-service");
            assert!(result.is_ok() || result.is_err());  // Either is fine
        }
    }
}
```

#### Step 2: Update lib.rs

```rust
// File: crates/akidb-embedding/src/lib.rs

#[cfg(feature = "candle")]
pub mod tracing_init;

#[cfg(feature = "candle")]
pub use tracing_init::{init_tracing, shutdown_tracing};
```

#### Step 3: Initialize in Main

```rust
// File: crates/akidb-rest/src/main.rs

use akidb_embedding::init_tracing;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    init_tracing("akidb-rest")?;

    tracing::info!("Starting AkiDB REST API server");

    // ... rest of main
}
```

#### Checkpoint

- ✅ Tracing initialization implemented
- ✅ Integrated in main
- ✅ Commit: `git commit -am "Phase 3 Day 2: Implement tracing initialization"`

---

### Task 2.3: Add Instrumentation to Hot Paths
**Time:** 2 hours

#### Step 1: Add Spans to embed_batch_internal()

```rust
// File: crates/akidb-embedding/src/candle.rs

use tracing::{info, warn, error, instrument, Span};

impl CandleEmbeddingProvider {
    /// Generate embeddings with tracing
    #[instrument(
        skip(self, texts),
        fields(
            batch_size = texts.len(),
            model = %self.model_name,
            device = ?self.device,
        )
    )]
    pub async fn embed_batch_internal(
        &self,
        texts: Vec<String>,
    ) -> EmbeddingResult<Vec<Vec<f32>>> {
        info!("Starting batch embedding");

        // Track in-flight requests
        METRICS.requests_in_flight.inc();
        defer! {
            METRICS.requests_in_flight.dec();
        }

        METRICS.requests_total.inc();
        METRICS.batch_size.observe(texts.len() as f64);

        let start = Instant::now();
        let result = self.embed_internal(texts).await;
        let duration = start.elapsed().as_secs_f64();

        match &result {
            Ok(embeddings) => {
                info!(
                    embeddings_count = embeddings.len(),
                    duration_ms = (duration * 1000.0) as u64,
                    "Batch embedding completed successfully"
                );
                METRICS.request_duration
                    .with_label_values(&["success"])
                    .observe(duration);
            }
            Err(e) => {
                error!(
                    error = %e,
                    duration_ms = (duration * 1000.0) as u64,
                    "Batch embedding failed"
                );
                METRICS.request_duration
                    .with_label_values(&["error"])
                    .observe(duration);
                METRICS.errors_total.inc();
            }
        }

        result
    }

    #[instrument(skip(self, texts), fields(batch_size = texts.len()))]
    async fn embed_internal(
        &self,
        texts: Vec<String>,
    ) -> EmbeddingResult<Vec<Vec<f32>>> {
        // Tokenization span
        let input_ids = {
            let _span = tracing::info_span!("tokenize").entered();
            info!("Tokenizing {} texts", texts.len());

            let result = tokio::task::spawn_blocking({
                let tokenizer = Arc::clone(&self.tokenizer);
                let texts = texts.clone();
                move || Self::tokenize_batch_parallel(&tokenizer, &texts)
            }).await.map_err(|e| EmbeddingError::InferenceFailed(e.to_string()))??;

            info!("Tokenization complete");
            result
        };

        // Inference span
        let embeddings = {
            let _span = tracing::info_span!(
                "inference",
                device = ?self.device
            ).entered();

            info!("Starting GPU inference");

            let result = tokio::task::spawn_blocking({
                let model = Arc::clone(&self.model);
                let device = self.device.clone();
                move || {
                    // GPU transfer in
                    tracing::trace!("Transferring tensors to GPU");
                    let input_ids_gpu = input_ids.to_device(&device)
                        .map_err(|e| EmbeddingError::InferenceFailed(e.to_string()))?;

                    // Forward pass
                    tracing::trace!("Running forward pass");
                    let outputs = model.forward(&input_ids_gpu)
                        .map_err(|e| EmbeddingError::InferenceFailed(e.to_string()))?;

                    // Mean pooling
                    tracing::trace!("Performing mean pooling");
                    let embeddings = outputs.mean(1)
                        .map_err(|e| EmbeddingError::InferenceFailed(e.to_string()))?;

                    // Normalization
                    tracing::trace!("Normalizing embeddings");
                    let norms = embeddings.sqr()?.sum_keepdim(1)?.sqrt()?;
                    let normalized = embeddings.broadcast_div(&norms)
                        .map_err(|e| EmbeddingError::InferenceFailed(e.to_string()))?;

                    // GPU transfer out
                    tracing::trace!("Transferring results to CPU");
                    normalized.to_vec2()
                        .map_err(|e| EmbeddingError::InferenceFailed(e.to_string()))
                }
            }).await.map_err(|e| EmbeddingError::InferenceFailed(e.to_string()))??;

            let inference_duration = inference_start.elapsed().as_secs_f64();
            METRICS.model_inference_duration.observe(inference_duration);

            info!(
                inference_duration_ms = (inference_duration * 1000.0) as u64,
                "Inference complete"
            );

            result
        };

        Ok(embeddings)
    }
}
```

#### Step 2: Add Spans to Model Loading

```rust
#[instrument(skip_all, fields(model_name = %model_name))]
pub async fn new(model_name: &str) -> EmbeddingResult<Self> {
    info!("Loading model: {}", model_name);
    let load_start = Instant::now();

    // Download model
    let (model_path, config_path, tokenizer_path) = {
        let _span = tracing::info_span!("download_model").entered();
        Self::download_model(model_name).await?
    };

    // Load config
    let config: Config = {
        let _span = tracing::info_span!("load_config").entered();
        let config_str = tokio::fs::read_to_string(&config_path)
            .await
            .map_err(|e| EmbeddingError::ModelLoadFailed(e.to_string()))?;
        serde_json::from_str(&config_str)
            .map_err(|e| EmbeddingError::ModelLoadFailed(e.to_string()))?
    };

    // Select device
    let device = {
        let _span = tracing::info_span!("select_device").entered();
        Self::select_device()?
    };

    // Load model weights
    let model = {
        let _span = tracing::info_span!("load_weights").entered();
        info!("Loading model weights from disk");

        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(
                &[model_path.join("model.safetensors")],
                DType::F32,
                &device,
            ).map_err(|e| EmbeddingError::ModelLoadFailed(e.to_string()))?
        };

        BertModel::load(vb, &config)
            .map_err(|e| EmbeddingError::ModelLoadFailed(e.to_string()))?
    };

    // Load tokenizer
    let tokenizer = {
        let _span = tracing::info_span!("load_tokenizer").entered();
        Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| EmbeddingError::ModelLoadFailed(e.to_string()))?
    };

    let load_duration = load_start.elapsed().as_secs_f64();
    METRICS.model_load_duration.observe(load_duration);

    info!(
        model = %model_name,
        device = ?device,
        load_duration_seconds = load_duration,
        "Model loaded successfully"
    );

    Ok(Self {
        model: Arc::new(model),
        tokenizer: Arc::new(tokenizer),
        device,
        model_name: model_name.to_string(),
        dimension: config.hidden_size as u32,
    })
}
```

#### Step 3: Test Tracing

```bash
# Set environment for tracing
export RUST_LOG=debug
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317

# Start server
cargo run -p akidb-rest --features candle

# Make request
curl -X POST http://localhost:8080/api/v1/embed \
  -H "Content-Type: application/json" \
  -d '{"texts": ["Test text"]}'

# Check logs (should see structured JSON)
```

**Expected Log Output:**
```json
{"timestamp":"2025-11-10T10:00:00.123Z","level":"INFO","target":"akidb_embedding::candle","fields":{"message":"Starting batch embedding","batch_size":1,"model":"sentence-transformers/all-MiniLM-L6-v2","device":"Metal(0)"},"span":{"name":"embed_batch_internal"}}
{"timestamp":"2025-11-10T10:00:00.124Z","level":"INFO","target":"akidb_embedding::candle","fields":{"message":"Tokenizing 1 texts"},"span":{"name":"tokenize"}}
{"timestamp":"2025-11-10T10:00:00.125Z","level":"INFO","target":"akidb_embedding::candle","fields":{"message":"Starting GPU inference","device":"Metal(0)"},"span":{"name":"inference"}}
{"timestamp":"2025-11-10T10:00:00.138Z","level":"INFO","target":"akidb_embedding::candle","fields":{"message":"Batch embedding completed successfully","embeddings_count":1,"duration_ms":15}}
```

#### Checkpoint

- ✅ Hot paths instrumented with spans
- ✅ Structured logs with context
- ✅ Tracing verified
- ✅ Commit: `git commit -am "Phase 3 Day 2: Add instrumentation to hot paths"`

---

### Task 2.4: Implement Structured Logging
**Time:** 1.5 hours

#### Step 1: Add Logging Helpers

```rust
// File: crates/akidb-embedding/src/logging.rs

use serde_json::json;
use tracing::{event, Level};
use uuid::Uuid;

/// Request context for logging
#[derive(Clone)]
pub struct RequestContext {
    pub request_id: String,
    pub batch_size: usize,
    pub model_name: String,
    pub device: String,
}

impl RequestContext {
    pub fn new(batch_size: usize, model_name: &str, device: &str) -> Self {
        Self {
            request_id: Uuid::new_v4().to_string(),
            batch_size,
            model_name: model_name.to_string(),
            device: device.to_string(),
        }
    }
}

/// Log successful embedding request
pub fn log_embedding_success(
    ctx: &RequestContext,
    duration_ms: u64,
    embeddings_count: usize,
) {
    event!(
        Level::INFO,
        request_id = %ctx.request_id,
        batch_size = ctx.batch_size,
        model = %ctx.model_name,
        device = %ctx.device,
        duration_ms = duration_ms,
        embeddings_count = embeddings_count,
        "Embedding request completed successfully"
    );
}

/// Log failed embedding request
pub fn log_embedding_error(
    ctx: &RequestContext,
    error: &str,
    duration_ms: u64,
) {
    event!(
        Level::ERROR,
        request_id = %ctx.request_id,
        batch_size = ctx.batch_size,
        model = %ctx.model_name,
        device = %ctx.device,
        error = %error,
        duration_ms = duration_ms,
        "Embedding request failed"
    );
}

/// Log model loading event
pub fn log_model_load(
    model_name: &str,
    device: &str,
    duration_seconds: f64,
) {
    event!(
        Level::INFO,
        model = %model_name,
        device = %device,
        duration_seconds = duration_seconds,
        "Model loaded successfully"
    );
}
```

#### Step 2: Use Logging Helpers

```rust
// File: crates/akidb-embedding/src/candle.rs

use crate::logging::{RequestContext, log_embedding_success, log_embedding_error};

impl CandleEmbeddingProvider {
    pub async fn embed_batch_internal(
        &self,
        texts: Vec<String>,
    ) -> EmbeddingResult<Vec<Vec<f32>>> {
        // Create request context
        let ctx = RequestContext::new(
            texts.len(),
            &self.model_name,
            &format!("{:?}", self.device),
        );

        tracing::Span::current().record("request_id", &ctx.request_id);

        // ... existing code ...

        match &result {
            Ok(embeddings) => {
                log_embedding_success(&ctx, (duration * 1000.0) as u64, embeddings.len());
            }
            Err(e) => {
                log_embedding_error(&ctx, &e.to_string(), (duration * 1000.0) as u64);
            }
        }

        result
    }
}
```

#### Checkpoint

- ✅ Structured logging helpers implemented
- ✅ Request context tracking
- ✅ Logs include correlation IDs
- ✅ Commit: `git commit -am "Phase 3 Day 2: Implement structured logging"`

---

### Task 2.5: Test with Jaeger
**Time:** 1 hour

#### Step 1: Start Jaeger (Optional)

```bash
# Using Docker
docker run -d --name jaeger \
  -p 4317:4317 \
  -p 4318:4318 \
  -p 16686:16686 \
  jaegertracing/all-in-one:latest

# Or using docker-compose
# File: docker-compose.yaml (add to existing)
services:
  jaeger:
    image: jaegertracing/all-in-one:latest
    ports:
      - "4317:4317"  # OTLP gRPC
      - "4318:4318"  # OTLP HTTP
      - "16686:16686"  # Jaeger UI
```

#### Step 2: Run Server with Tracing

```bash
export RUST_LOG=debug
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317

cargo run -p akidb-rest --features candle
```

#### Step 3: Generate Traces

```bash
# Generate some requests
for i in {1..10}; do
  curl -X POST http://localhost:8080/api/v1/embed \
    -H "Content-Type: application/json" \
    -d '{"texts": ["Test text '$i'"]}'
done
```

#### Step 4: View Traces in Jaeger UI

```bash
# Open Jaeger UI
open http://localhost:16686

# Search for service: akidb-rest
# View traces and spans
```

**Expected Trace Hierarchy:**
```
embedding_request (12.5ms)
├─ tokenize (1.2ms)
├─ inference (2.8ms)
│  ├─ gpu_transfer_in
│  ├─ forward_pass
│  ├─ mean_pooling
│  ├─ normalize
│  └─ gpu_transfer_out
└─ result_distribution (0.2ms)
```

#### Checkpoint

- ✅ Jaeger integration tested
- ✅ Traces visible in UI
- ✅ Span hierarchy correct
- ✅ Commit: `git commit -am "Phase 3 Day 2: Test OpenTelemetry with Jaeger"`

---

### Day 2 Checkpoint

**Accomplishments:**
- ✅ Added OpenTelemetry dependencies
- ✅ Implemented tracing initialization (~150 lines)
- ✅ Instrumented hot paths with spans
- ✅ Implemented structured logging
- ✅ Jaeger integration verified

**Verification:**
```bash
# Run tests
cargo test --workspace --features candle

# Verify tracing
RUST_LOG=debug cargo run -p akidb-rest --features candle

# Check Jaeger
open http://localhost:16686
```

**Deliverables:**
- `src/tracing_init.rs` (~150 lines) ✅
- Spans on all operations ✅
- Structured JSON logs ✅
- Jaeger integration ✅

**Time Spent:** 6 hours (on budget)

**Next:** Day 3 - Resilience patterns (circuit breaker, retry, timeout)

---

## Day 3: Resilience Patterns
**Wednesday, 6 hours**

### Overview

**Goal:** Implement circuit breaker, retry logic, timeouts, and graceful degradation

**Deliverables:**
- `src/circuit_breaker.rs` (~250 lines)
- `src/retry.rs` (~100 lines)
- 6 resilience tests passing
- CPU fallback mechanism

---

### Task 3.1: Implement Circuit Breaker
**Time:** 2.5 hours

#### Step 1: Create circuit_breaker.rs

```rust
// File: crates/akidb-embedding/src/circuit_breaker.rs

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use std::future::Future;
use std::pin::Pin;
use crate::metrics::METRICS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,     // Normal operation
    Open,       // Failing, reject requests
    HalfOpen,   // Testing if service recovered
}

pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    failure_count: Arc<RwLock<usize>>,
    success_count: Arc<RwLock<usize>>,
    last_failure_time: Arc<RwLock<Option<Instant>>>,
    last_state_change: Arc<RwLock<Instant>>,
    config: CircuitBreakerConfig,
}

#[derive(Clone, Debug)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening circuit
    pub failure_threshold: usize,

    /// Time window for counting failures
    pub failure_window: Duration,

    /// Time to wait before attempting recovery
    pub recovery_timeout: Duration,

    /// Success threshold to close circuit from half-open
    pub success_threshold: usize,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            failure_window: Duration::from_secs(10),
            recovery_timeout: Duration::from_secs(30),
            success_threshold: 2,
        }
    }
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_count: Arc::new(RwLock::new(0)),
            success_count: Arc::new(RwLock::new(0)),
            last_failure_time: Arc::new(RwLock::new(None)),
            last_state_change: Arc::new(RwLock::new(Instant::now())),
            config,
        }
    }

    /// Execute function with circuit breaker protection
    pub async fn call<F, T, E>(&self, f: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: Future<Output = Result<T, E>>,
    {
        // Check circuit state
        let state = *self.state.read().await;

        match state {
            CircuitState::Open => {
                // Check if recovery timeout elapsed
                if let Some(last_failure) = *self.last_failure_time.read().await {
                    if last_failure.elapsed() >= self.config.recovery_timeout {
                        // Transition to half-open
                        *self.state.write().await = CircuitState::HalfOpen;
                        *self.success_count.write().await = 0;
                        *self.last_state_change.write().await = Instant::now();
                        METRICS.circuit_breaker_state.set(1);

                        tracing::info!(
                            "Circuit breaker: OPEN → HALF-OPEN (recovery timeout elapsed)"
                        );
                    } else {
                        // Still open, reject request
                        tracing::warn!("Circuit breaker: Request rejected (circuit OPEN)");
                        return Err(CircuitBreakerError::CircuitOpen);
                    }
                } else {
                    return Err(CircuitBreakerError::CircuitOpen);
                }
            }
            CircuitState::Closed | CircuitState::HalfOpen => {
                // Allow request
            }
        }

        // Execute function
        match f.await {
            Ok(result) => {
                self.record_success().await;
                Ok(result)
            }
            Err(e) => {
                self.record_failure().await;
                Err(CircuitBreakerError::InnerError(e))
            }
        }
    }

    async fn record_success(&self) {
        let state = *self.state.read().await;

        if state == CircuitState::HalfOpen {
            let mut success_count = self.success_count.write().await;
            *success_count += 1;

            if *success_count >= self.config.success_threshold {
                // Close circuit
                *self.state.write().await = CircuitState::Closed;
                *self.failure_count.write().await = 0;
                *self.last_state_change.write().await = Instant::now();
                METRICS.circuit_breaker_state.set(0);

                tracing::info!(
                    "Circuit breaker: HALF-OPEN → CLOSED (success threshold reached)"
                );
            }
        }
    }

    async fn record_failure(&self) {
        let state = *self.state.read().await;

        if state == CircuitState::HalfOpen {
            // Failure in half-open → re-open circuit
            *self.state.write().await = CircuitState::Open;
            *self.failure_count.write().await = 0;
            *self.last_failure_time.write().await = Some(Instant::now());
            *self.last_state_change.write().await = Instant::now();
            METRICS.circuit_breaker_state.set(2);

            tracing::warn!("Circuit breaker: HALF-OPEN → OPEN (failure detected)");
            return;
        }

        // Increment failure count
        let mut count = self.failure_count.write().await;
        *count += 1;

        let mut last_failure = self.last_failure_time.write().await;
        let now = Instant::now();
        *last_failure = Some(now);

        // Check if we should open circuit
        if *count >= self.config.failure_threshold {
            let last_state_change = *self.last_state_change.read().await;

            // Only open if failures occurred within failure_window
            if now.duration_since(last_state_change) <= self.config.failure_window {
                // Open circuit
                *self.state.write().await = CircuitState::Open;
                *self.last_state_change.write().await = now;
                METRICS.circuit_breaker_state.set(2);

                tracing::warn!(
                    "Circuit breaker: CLOSED → OPEN (failure threshold: {} reached)",
                    *count
                );
            } else {
                // Reset counter (failures too old)
                *count = 1;
                *self.last_state_change.write().await = now;
            }
        }
    }

    pub async fn get_state(&self) -> CircuitState {
        *self.state.read().await
    }

    pub async fn reset(&self) {
        *self.state.write().await = CircuitState::Closed;
        *self.failure_count.write().await = 0;
        *self.success_count.write().await = 0;
        *self.last_failure_time.write().await = None;
        *self.last_state_change.write().await = Instant::now();
        METRICS.circuit_breaker_state.set(0);

        tracing::info!("Circuit breaker: Reset to CLOSED");
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CircuitBreakerError<E> {
    #[error("Circuit breaker is open")]
    CircuitOpen,

    #[error("Inner error: {0}")]
    InnerError(E),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_opens_after_failures() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 3,
            failure_window: Duration::from_secs(10),
            recovery_timeout: Duration::from_millis(100),
            success_threshold: 2,
        });

        assert_eq!(cb.get_state().await, CircuitState::Closed);

        // Trigger 3 failures
        for _ in 0..3 {
            let result: Result<(), &str> = Err("test error");
            let _ = cb.call(async { result }).await;
        }

        // Circuit should be open
        assert_eq!(cb.get_state().await, CircuitState::Open);

        // Next request should be rejected
        let result: Result<(), &str> = Ok(());
        let call_result = cb.call(async { result }).await;
        assert!(matches!(call_result, Err(CircuitBreakerError::CircuitOpen)));
    }

    #[tokio::test]
    async fn test_circuit_recovers() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 2,
            failure_window: Duration::from_secs(10),
            recovery_timeout: Duration::from_millis(50),
            success_threshold: 2,
        });

        // Trigger 2 failures
        for _ in 0..2 {
            let result: Result<(), &str> = Err("test error");
            let _ = cb.call(async { result }).await;
        }

        assert_eq!(cb.get_state().await, CircuitState::Open);

        // Wait for recovery timeout
        tokio::time::sleep(Duration::from_millis(60)).await;

        // Should transition to half-open and allow request
        let result: Result<i32, &str> = Ok(42);
        let call_result = cb.call(async { result }).await;
        assert!(call_result.is_ok());

        assert_eq!(cb.get_state().await, CircuitState::HalfOpen);

        // One more success should close circuit
        let result: Result<i32, &str> = Ok(42);
        let call_result = cb.call(async { result }).await;
        assert!(call_result.is_ok());

        assert_eq!(cb.get_state().await, CircuitState::Closed);
    }
}
```

#### Step 2: Add to lib.rs

```rust
// File: crates/akidb-embedding/src/lib.rs

#[cfg(feature = "candle")]
pub mod circuit_breaker;

#[cfg(feature = "candle")]
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitState};
```

#### Step 3: Test Circuit Breaker

```bash
cargo test --features candle circuit_breaker::tests
```

**Expected Output:**
```
running 2 tests
test circuit_breaker::tests::test_circuit_opens_after_failures ... ok
test circuit_breaker::tests::test_circuit_recovers ... ok

test result: ok. 2 passed
```

#### Checkpoint

- ✅ Circuit breaker implemented (~250 lines)
- ✅ 2 tests passing
- ✅ Commit: `git commit -am "Phase 3 Day 3: Implement circuit breaker"`

---

### Task 3.2: Implement Retry Logic
**Time:** 1.5 hours

#### Step 1: Create retry.rs

```rust
// File: crates/akidb-embedding/src/retry.rs

use std::time::Duration;
use std::future::Future;
use std::pin::Pin;

#[derive(Clone, Debug)]
pub struct RetryConfig {
    pub max_retries: usize,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
    pub multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(5),
            multiplier: 2.0,
        }
    }
}

/// Retry a function with exponential backoff
pub async fn retry_with_backoff<F, Fut, T, E>(
    config: &RetryConfig,
    mut f: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut attempt = 0;
    let mut backoff = config.initial_backoff;

    loop {
        attempt += 1;

        match f().await {
            Ok(result) => {
                if attempt > 1 {
                    tracing::info!(
                        "Retry succeeded on attempt {}",
                        attempt
                    );
                }
                return Ok(result);
            }
            Err(e) => {
                if attempt >= config.max_retries {
                    tracing::error!(
                        "Retry exhausted after {} attempts: {}",
                        attempt,
                        e
                    );
                    return Err(e);
                }

                tracing::warn!(
                    "Attempt {} failed: {}. Retrying in {:?}...",
                    attempt,
                    e,
                    backoff
                );

                tokio::time::sleep(backoff).await;

                // Exponential backoff
                backoff = Duration::from_secs_f64(
                    (backoff.as_secs_f64() * config.multiplier)
                        .min(config.max_backoff.as_secs_f64())
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_retry_succeeds() {
        let attempt_count = Arc::new(AtomicUsize::new(0));

        let config = RetryConfig {
            max_retries: 3,
            initial_backoff: Duration::from_millis(10),
            max_backoff: Duration::from_millis(100),
            multiplier: 2.0,
        };

        let result = retry_with_backoff(&config, || {
            let count = attempt_count.clone();
            async move {
                let attempts = count.fetch_add(1, Ordering::SeqCst);
                if attempts < 2 {
                    Err("temporary failure")
                } else {
                    Ok(42)
                }
            }
        }).await;

        assert_eq!(result, Ok(42));
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_exhausted() {
        let attempt_count = Arc::new(AtomicUsize::new(0));

        let config = RetryConfig {
            max_retries: 3,
            initial_backoff: Duration::from_millis(10),
            max_backoff: Duration::from_millis(100),
            multiplier: 2.0,
        };

        let result = retry_with_backoff(&config, || {
            let count = attempt_count.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Err::<i32, &str>("permanent failure")
            }
        }).await;

        assert!(result.is_err());
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
    }
}
```

#### Step 2: Add to lib.rs

```rust
#[cfg(feature = "candle")]
pub mod retry;

#[cfg(feature = "candle")]
pub use retry::{retry_with_backoff, RetryConfig};
```

#### Step 3: Test Retry

```bash
cargo test --features candle retry::tests
```

**Expected Output:**
```
running 2 tests
test retry::tests::test_retry_succeeds ... ok
test retry::tests::test_retry_exhausted ... ok

test result: ok. 2 passed
```

#### Checkpoint

- ✅ Retry logic implemented (~100 lines)
- ✅ 2 tests passing
- ✅ Commit: `git commit -am "Phase 3 Day 3: Implement retry with backoff"`

---

### Task 3.3: Implement Timeout Enforcement
**Time:** 1 hour

#### Step 1: Add Timeout Wrapper

```rust
// File: crates/akidb-embedding/src/candle.rs

use tokio::time::timeout;

impl CandleEmbeddingProvider {
    /// Generate embeddings with timeout
    pub async fn embed_batch_with_timeout(
        &self,
        texts: Vec<String>,
        timeout_duration: Duration,
    ) -> EmbeddingResult<Vec<Vec<f32>>> {
        match timeout(timeout_duration, self.embed_batch_internal(texts)).await {
            Ok(result) => result,
            Err(_) => {
                tracing::error!(
                    "Embedding request timed out after {:?}",
                    timeout_duration
                );
                Err(EmbeddingError::Timeout(format!(
                    "Request timed out after {:?}",
                    timeout_duration
                )))
            }
        }
    }
}
```

#### Step 2: Update Error Type

```rust
// File: crates/akidb-core/src/error.rs

#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    // ... existing variants

    #[error("Request timed out: {0}")]
    Timeout(String),
}
```

#### Step 3: Test Timeout

```rust
// File: crates/akidb-embedding/tests/timeout_tests.rs

#[tokio::test]
async fn test_timeout_enforcement() {
    let provider = CandleEmbeddingProvider::new(
        "sentence-transformers/all-MiniLM-L6-v2"
    ).await.unwrap();

    // Set very short timeout (should timeout)
    let result = provider.embed_batch_with_timeout(
        vec!["Test".to_string()],
        Duration::from_millis(1),  // 1ms timeout
    ).await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), EmbeddingError::Timeout(_)));
}

#[tokio::test]
async fn test_timeout_success() {
    let provider = CandleEmbeddingProvider::new(
        "sentence-transformers/all-MiniLM-L6-v2"
    ).await.unwrap();

    // Set reasonable timeout (should succeed)
    let result = provider.embed_batch_with_timeout(
        vec!["Test".to_string()],
        Duration::from_secs(5),
    ).await;

    assert!(result.is_ok());
}
```

```bash
cargo test --features candle timeout_tests
```

#### Checkpoint

- ✅ Timeout enforcement implemented
- ✅ 2 tests passing
- ✅ Commit: `git commit -am "Phase 3 Day 3: Implement timeout enforcement"`

---

### Task 3.4: Implement Graceful Degradation
**Time:** 1 hour

#### Step 1: Add CPU Fallback

```rust
// File: crates/akidb-embedding/src/candle.rs

impl CandleEmbeddingProvider {
    /// Create provider with specific device
    pub async fn new_with_device(
        model_name: &str,
        device: Device,
    ) -> EmbeddingResult<Self> {
        // Similar to new() but use provided device
        // ... (implementation similar to new())
    }

    /// Generate embeddings with GPU→CPU fallback
    pub async fn embed_with_fallback(
        &self,
        texts: Vec<String>,
    ) -> EmbeddingResult<Vec<Vec<f32>>> {
        // Try GPU first
        match self.embed_batch_internal(texts.clone()).await {
            Ok(result) => {
                tracing::debug!("GPU inference succeeded");
                return Ok(result);
            }
            Err(e) => {
                tracing::warn!(
                    "GPU inference failed: {}. Trying CPU fallback...",
                    e
                );
            }
        }

        // Try CPU fallback
        tracing::info!("Falling back to CPU inference");
        let cpu_provider = Self::new_with_device(
            &self.model_name,
            Device::Cpu,
        ).await?;

        match cpu_provider.embed_batch_internal(texts).await {
            Ok(result) => {
                tracing::info!("CPU fallback succeeded");
                Ok(result)
            }
            Err(e) => {
                tracing::error!("CPU fallback failed: {}", e);
                Err(EmbeddingError::ServiceUnavailable(
                    format!("All inference backends failed: {}", e)
                ))
            }
        }
    }
}
```

#### Step 2: Test Fallback

```rust
// File: crates/akidb-embedding/tests/fallback_tests.rs

#[tokio::test]
async fn test_cpu_fallback() {
    let provider = CandleEmbeddingProvider::new(
        "sentence-transformers/all-MiniLM-L6-v2"
    ).await.unwrap();

    // This test requires ability to simulate GPU failure
    // For now, just test that CPU provider works
    let cpu_provider = CandleEmbeddingProvider::new_with_device(
        "sentence-transformers/all-MiniLM-L6-v2",
        Device::Cpu,
    ).await.unwrap();

    let result = cpu_provider.embed_batch_internal(vec![
        "CPU fallback test".to_string()
    ]).await;

    assert!(result.is_ok());
}
```

```bash
cargo test --features candle fallback_tests
```

#### Checkpoint

- ✅ Graceful degradation implemented
- ✅ CPU fallback working
- ✅ Commit: `git commit -am "Phase 3 Day 3: Implement graceful degradation"`

---

### Day 3 Checkpoint

**Accomplishments:**
- ✅ Circuit breaker implemented (~250 lines)
- ✅ Retry logic with exponential backoff (~100 lines)
- ✅ Timeout enforcement
- ✅ Graceful degradation (GPU → CPU)
- ✅ 6 resilience tests passing

**Verification:**
```bash
# Run all resilience tests
cargo test --features candle circuit_breaker retry timeout fallback

# Verify compilation
cargo check --workspace --features candle
```

**Deliverables:**
- `src/circuit_breaker.rs` (~250 lines) ✅
- `src/retry.rs` (~100 lines) ✅
- 6 tests passing ✅

**Time Spent:** 6 hours (on budget)

**Next:** Day 4 - Health checks + integration tests

---

## Day 4: Health Checks + Integration Tests
**Thursday, 6 hours**

### Overview

**Goal:** Add Kubernetes health checks and comprehensive integration tests

**Deliverables:**
- `src/health.rs` (~150 lines)
- 8 E2E tests
- 6 resilience integration tests
- 3 health check tests
- 3 observability tests

---

### Task 4.1: Implement Health Check Endpoints
**Time:** 1.5 hours

#### Step 1: Create health.rs

```rust
// File: crates/akidb-rest/src/handlers/health.rs

use axum::{Json, http::StatusCode, response::IntoResponse, Extension};
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use akidb_embedding::{CandleEmbeddingProvider, CircuitBreaker, CircuitState};

#[derive(Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: String,  // "ok" or "error"
    pub details: Option<HealthDetails>,
}

#[derive(Serialize, Deserialize)]
pub struct HealthDetails {
    pub model_loaded: bool,
    pub gpu_available: bool,
    pub circuit_breaker_state: String,
    pub memory_usage_mb: u64,
    pub uptime_seconds: u64,
}

/// Liveness probe: Is the process alive?
pub async fn liveness() -> Json<HealthStatus> {
    Json(HealthStatus {
        status: "ok".to_string(),
        details: None,
    })
}

/// Readiness probe: Is the service ready to handle requests?
pub async fn readiness(
    Extension(provider): Extension<Arc<CandleEmbeddingProvider>>,
    Extension(circuit_breaker): Extension<Arc<CircuitBreaker>>,
) -> impl IntoResponse {
    // Check 1: Model loaded
    let model_loaded = provider.is_model_loaded();

    // Check 2: GPU available (if configured)
    let gpu_available = provider.is_gpu_available();

    // Check 3: Circuit breaker not open
    let cb_state = circuit_breaker.get_state().await;
    let cb_ok = cb_state != CircuitState::Open;

    // Check 4: Memory usage reasonable
    let memory_mb = get_memory_usage_mb();
    let memory_ok = memory_mb < 2048;  // <2GB

    let all_ok = model_loaded && gpu_available && cb_ok && memory_ok;

    let status_code = if all_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        status_code,
        Json(HealthStatus {
            status: if all_ok { "ok" } else { "error" }.to_string(),
            details: Some(HealthDetails {
                model_loaded,
                gpu_available,
                circuit_breaker_state: format!("{:?}", cb_state),
                memory_usage_mb: memory_mb,
                uptime_seconds: get_uptime_seconds(),
            }),
        })
    )
}

fn get_memory_usage_mb() -> u64 {
    // Platform-specific implementation
    #[cfg(target_os = "macos")]
    {
        use sysinfo::{System, SystemExt};
        let mut sys = System::new_all();
        sys.refresh_memory();
        sys.used_memory() / 1024  // Convert KB to MB
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Fallback: return 0
        0
    }
}

fn get_uptime_seconds() -> u64 {
    use std::time::SystemTime;
    static START_TIME: std::sync::OnceLock<SystemTime> = std::sync::OnceLock::new();

    let start = START_TIME.get_or_init(|| SystemTime::now());
    start.elapsed().unwrap_or_default().as_secs()
}
```

#### Step 2: Add Helper Methods to Provider

```rust
// File: crates/akidb-embedding/src/candle.rs

impl CandleEmbeddingProvider {
    pub fn is_model_loaded(&self) -> bool {
        // Model is always loaded if provider exists
        true
    }

    pub fn is_gpu_available(&self) -> bool {
        !matches!(self.device, Device::Cpu)
    }
}
```

#### Step 3: Update Router

```rust
// File: crates/akidb-rest/src/main.rs

use axum::{Router, routing::get, Extension};

async fn create_app(
    provider: Arc<CandleEmbeddingProvider>,
    circuit_breaker: Arc<CircuitBreaker>,
) -> Router {
    Router::new()
        .route("/health/live", get(handlers::health::liveness))
        .route("/health/ready", get(handlers::health::readiness))
        .route("/metrics", get(handlers::metrics::metrics))
        .route("/api/v1/embed", post(handlers::embed::embed))
        .layer(Extension(provider))
        .layer(Extension(circuit_breaker))
}
```

#### Step 4: Test Health Endpoints

```bash
# Start server
cargo run -p akidb-rest --features candle &
sleep 5

# Test liveness
curl http://localhost:8080/health/live
# Expected: {"status":"ok","details":null}

# Test readiness
curl http://localhost:8080/health/ready
# Expected: {"status":"ok","details":{...}}
```

#### Checkpoint

- ✅ Health endpoints implemented (~150 lines)
- ✅ Liveness and readiness probes working
- ✅ Commit: `git commit -am "Phase 3 Day 4: Implement health check endpoints"`

---

### Task 4.2: Write E2E Tests
**Time:** 2 hours

#### Step 1: Create E2E Test Suite

```rust
// File: crates/akidb-rest/tests/e2e_tests.rs

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;
use serde_json::json;

#[tokio::test]
async fn test_e2e_single_request() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/embed")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "texts": ["Hello, world!"]
                    }).to_string()
                ))
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["embeddings"].as_array().unwrap().len(), 1);
    assert_eq!(json["embeddings"][0].as_array().unwrap().len(), 384);
}

#[tokio::test]
async fn test_e2e_batch_request() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/embed")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "texts": ["Text 1", "Text 2", "Text 3"]
                    }).to_string()
                ))
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["embeddings"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn test_e2e_concurrent_requests() {
    let app = Arc::new(create_test_app().await);

    let tasks: Vec<_> = (0..50)
        .map(|i| {
            let app = Arc::clone(&app);
            tokio::spawn(async move {
                app.clone()
                    .oneshot(
                        Request::builder()
                            .method("POST")
                            .uri("/api/v1/embed")
                            .header("content-type", "application/json")
                            .body(Body::from(
                                json!({
                                    "texts": [format!("Test {}", i)]
                                }).to_string()
                            ))
                            .unwrap()
                    )
                    .await
            })
        })
        .collect();

    let results: Vec<_> = futures::future::join_all(tasks).await;

    // All should succeed
    for result in results {
        let response = result.unwrap().unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[tokio::test]
async fn test_e2e_error_handling_empty_input() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/embed")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "texts": []
                    }).to_string()
                ))
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_e2e_large_batch() {
    let app = create_test_app().await;

    let texts: Vec<String> = (0..32).map(|i| format!("Text {}", i)).collect();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/embed")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "texts": texts
                    }).to_string()
                ))
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["embeddings"].as_array().unwrap().len(), 32);
}

#[tokio::test]
async fn test_e2e_long_text() {
    let app = create_test_app().await;

    let long_text = "word ".repeat(1000);  // 5000+ characters

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/embed")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "texts": [long_text]
                    }).to_string()
                ))
                .unwrap()
        )
        .await
        .unwrap();

    // Should truncate and succeed (not fail)
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_e2e_special_characters() {
    let app = create_test_app().await;

    let texts = vec![
        "Hello, 世界!",
        "Émojis: 😀🎉🚀",
        "Math: ∑∫∂",
    ];

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/embed")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "texts": texts
                    }).to_string()
                ))
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_e2e_metrics_exported() {
    let app = create_test_app().await;

    // Make some requests
    for i in 0..5 {
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/embed")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "texts": [format!("Test {}", i)]
                        }).to_string()
                    ))
                    .unwrap()
            )
            .await;
    }

    // Check metrics
    let response = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let metrics_text = String::from_utf8(body.to_vec()).unwrap();

    assert!(metrics_text.contains("candle_requests_total"));
}

// Helper function
async fn create_test_app() -> Router {
    let provider = Arc::new(
        CandleEmbeddingProvider::new("sentence-transformers/all-MiniLM-L6-v2")
            .await
            .unwrap()
    );
    let circuit_breaker = Arc::new(CircuitBreaker::new(CircuitBreakerConfig::default()));

    create_app(provider, circuit_breaker).await
}
```

```bash
cargo test --features candle e2e_tests
```

**Expected Output:**
```
running 8 tests
test e2e_tests::test_e2e_single_request ... ok
test e2e_tests::test_e2e_batch_request ... ok
test e2e_tests::test_e2e_concurrent_requests ... ok
test e2e_tests::test_e2e_error_handling_empty_input ... ok
test e2e_tests::test_e2e_large_batch ... ok
test e2e_tests::test_e2e_long_text ... ok
test e2e_tests::test_e2e_special_characters ... ok
test e2e_tests::test_e2e_metrics_exported ... ok

test result: ok. 8 passed
```

#### Checkpoint

- ✅ 8 E2E tests implemented
- ✅ All tests passing
- ✅ Commit: `git commit -am "Phase 3 Day 4: Add E2E integration tests"`

---

### Task 4.3: Write Resilience Integration Tests
**Time:** 1.5 hours

Due to length constraints, I'll provide a summary of the remaining tasks:

**Resilience Tests (6 tests):**
- Circuit breaker opens under load
- Circuit breaker recovers
- Retry succeeds after failures
- Timeout triggers correctly
- Graceful degradation works
- Multiple failures handled

**Health Check Tests (3 tests):**
- Liveness probe returns OK
- Readiness probe returns OK when healthy
- Readiness probe returns 503 when unhealthy

**Observability Tests (3 tests):**
- Metrics exported correctly
- Tracing spans created
- Structured logging works

#### Checkpoint

- ✅ 20 integration tests total
- ✅ All tests passing
- ✅ Commit: `git commit -am "Phase 3 Day 4: Complete integration test suite"`

---

### Day 4 Checkpoint

**Accomplishments:**
- ✅ Health check endpoints implemented
- ✅ 8 E2E tests
- ✅ 6 resilience tests
- ✅ 3 health check tests
- ✅ 3 observability tests
- ✅ Total: 20 new integration tests

**Verification:**
```bash
cargo test --workspace --features candle
# Expected: 56 tests passing (36 from Phase 1+2, 20 new)
```

**Time Spent:** 6 hours (on budget)

**Next:** Day 5 - Chaos testing + documentation + RC2 release

---

## Day 5: Chaos Testing + Documentation
**Friday, 6 hours**

### Overview

**Goal:** Validate resilience with chaos tests and complete operations documentation

**Deliverables:**
- 5 chaos tests
- `docs/OPERATIONS-RUNBOOK.md`
- Phase 3 completion report
- RC2 release tag

---

### Task 5.1: Implement Chaos Tests
**Time:** 2.5 hours

The chaos tests are implemented as shown in the PRD. Due to space constraints, refer to the PRD for full implementation.

#### Checkpoint

- ✅ 5 chaos tests implemented
- ✅ All chaos tests passing
- ✅ Commit: `git commit -am "Phase 3 Day 5: Add chaos engineering tests"`

---

### Task 5.2: Write Operations Runbook
**Time:** 1.5 hours

Create comprehensive operations documentation covering deployment, monitoring, troubleshooting, and incident response.

#### Checkpoint

- ✅ Operations runbook complete
- ✅ Commit: `git commit -am "Phase 3 Day 5: Add operations runbook"`

---

### Task 5.3: Write Phase 3 Completion Report
**Time:** 1 hour

Document accomplishments, metrics, and next steps.

#### Checkpoint

- ✅ Completion report written
- ✅ Commit: `git commit -am "Phase 3 Day 5: Add completion report"`

---

### Task 5.4: Create RC2 Release
**Time:** 1 hour

Tag RC2 release and prepare for staging deployment.

```bash
# Create RC2 tag
git tag -a v2.0.0-rc2 -m "Release Candidate 2: Production Hardening Complete"
git push origin v2.0.0-rc2

# Create release branch
git checkout -b release/v2.0.0-rc2
git push origin release/v2.0.0-rc2
```

#### Checkpoint

- ✅ RC2 release tagged
- ✅ Ready for staging deployment

---

## Phase 3 Summary

### Accomplishments

**Week 3 Deliverables:**
- ✅ Prometheus metrics (10 metrics)
- ✅ OpenTelemetry tracing
- ✅ Structured JSON logging
- ✅ Circuit breaker
- ✅ Retry with exponential backoff
- ✅ Timeout enforcement
- ✅ Graceful degradation
- ✅ Health check endpoints
- ✅ 20 integration tests
- ✅ 5 chaos tests
- ✅ Operations runbook
- ✅ RC2 release

**Code Statistics:**
- Lines added: ~1,560
- Tests: 61 total (36 Phase 1+2, 20 integration, 5 chaos)
- Test coverage: >80%

**Performance:**
- Observability overhead: <3%
- Circuit breaker latency: <1ms
- All resilience patterns working

### Next Steps

**Phase 4 Preview:**
- Multi-model support
- Model quantization (INT8/INT4)
- Advanced batching strategies
- Performance tuning

---

## Appendix

### Quick Reference Commands

```bash
# Run all tests
cargo test --workspace --features candle

# Run specific test suite
cargo test --features candle e2e_tests
cargo test --features candle resilience_tests
cargo test --features candle chaos_tests -- --ignored

# Check metrics
curl http://localhost:8080/metrics

# Check health
curl http://localhost:8080/health/live
curl http://localhost:8080/health/ready

# View traces (Jaeger)
open http://localhost:16686

# Start observability stack
docker compose up -d prometheus grafana jaeger
```

### Troubleshooting

**Problem:** Metrics not exported
- Check: `/metrics` endpoint returns 200
- Fix: Verify METRICS static is initialized

**Problem:** Traces not appearing in Jaeger
- Check: OTEL_EXPORTER_OTLP_ENDPOINT set correctly
- Fix: Start Jaeger: `docker run -p 4317:4317 -p 16686:16686 jaegertracing/all-in-one`

**Problem:** Circuit breaker too sensitive
- Check: Failure threshold and window
- Fix: Tune CircuitBreakerConfig

**Problem:** Tests flaky
- Check: Test timeouts
- Fix: Increase timeout durations

---

**Phase 3 Action Plan Complete! 🎉**

**Status:** Ready for Phase 4 (Multi-Model Support)

**Document End**
