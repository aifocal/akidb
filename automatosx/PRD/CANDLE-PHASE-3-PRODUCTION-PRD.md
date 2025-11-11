# Phase 3: Production Hardening PRD
## Candle Embedding Migration - Week 3

**Version:** 1.0
**Date:** 2025-11-10
**Status:** Ready for Implementation
**Owner:** Backend Team
**Timeline:** 5 days (Week 3, Monday-Friday)

---

## Executive Summary

**Goal:** Harden Candle embedding provider for production deployment with **monitoring, error handling, graceful degradation, and comprehensive integration testing** to achieve RC2 (Release Candidate 2) quality.

**Phase 3 Context:** Building on Phase 1's foundation and Phase 2's performance optimizations, this phase focuses on **production readiness**. We'll add observability, resilience patterns (circuit breakers, retries), health checks, and comprehensive integration/E2E tests to ensure the system can handle real-world failure scenarios.

**Success Criteria:**
- ✅ Prometheus metrics exported (10+ metrics)
- ✅ OpenTelemetry tracing integrated
- ✅ Circuit breaker prevents cascade failures
- ✅ Health checks validate readiness
- ✅ 20+ integration tests passing
- ✅ 99.9% uptime under chaos testing
- ✅ RC2 release ready for staging deployment

---

## Table of Contents

1. [Problem Statement](#problem-statement)
2. [Goals & Non-Goals](#goals--non-goals)
3. [Technical Design](#technical-design)
4. [Observability Strategy](#observability-strategy)
5. [Resilience Patterns](#resilience-patterns)
6. [Health Check Design](#health-check-design)
7. [Integration Testing](#integration-testing)
8. [Chaos Engineering](#chaos-engineering)
9. [Success Criteria](#success-criteria)
10. [Risks & Mitigation](#risks--mitigation)
11. [Timeline & Milestones](#timeline--milestones)
12. [Dependencies](#dependencies)
13. [Deliverables](#deliverables)

---

## Problem Statement

### Current State (Post Phase 2)

Phase 2 delivered **high-performance inference** with:
- ✅ 200+ QPS throughput
- ✅ P95 <35ms latency
- ✅ Dynamic batching
- ✅ Model caching
- ✅ GPU optimization (>70% utilization)
- ✅ 36 tests passing

**However**, the Phase 2 implementation is **not production-ready**:

| Gap | Impact | Risk Level |
|-----|--------|-----------|
| **No monitoring** | Cannot detect failures or performance degradation | 🔴 Critical |
| **No circuit breaker** | Cascade failures can take down entire system | 🔴 Critical |
| **Basic error handling** | Errors lack context, hard to debug | 🟡 High |
| **No health checks** | K8s cannot determine if service is ready | 🟡 High |
| **Limited integration tests** | Regressions may slip to production | 🟡 High |
| **No chaos testing** | Unknown behavior under failure conditions | 🟡 Medium |
| **No distributed tracing** | Cannot diagnose latency issues in production | 🟡 Medium |

### Why Production Hardening Matters

**Business Impact:**
- **Reliability:** 99.9% uptime → customer trust → revenue retention
- **Debuggability:** Fast incident resolution → lower MTTR → lower ops cost
- **Scalability:** Graceful degradation → survive traffic spikes → better UX
- **Compliance:** Audit logs + metrics → meet SLA requirements

**Technical Impact:**
- **Observability:** Know what's happening in production before customers report issues
- **Resilience:** Survive partial failures (GPU unavailable, model load failures)
- **Operations:** Easy deployment, rollback, and troubleshooting
- **Confidence:** Comprehensive tests → safe to deploy to production

---

## Goals & Non-Goals

### Goals (In Scope)

**Primary Goals:**
1. ✅ **Observability:** Prometheus metrics + OpenTelemetry tracing
2. ✅ **Resilience:** Circuit breaker + retry logic + timeouts
3. ✅ **Health Checks:** Liveness + readiness probes for K8s
4. ✅ **Error Handling:** Structured errors with context
5. ✅ **Integration Tests:** 20+ E2E tests covering failure scenarios
6. ✅ **Chaos Testing:** Inject failures to validate resilience

**Secondary Goals:**
7. ✅ **Resource Limits:** Graceful handling of OOM, GPU unavailable
8. ✅ **Configuration:** Runtime config without recompilation
9. ✅ **Logging:** Structured logging with context
10. ✅ **Documentation:** Operations runbook

### Non-Goals (Out of Scope)

**Deferred to Later Phases:**
- ❌ Multi-model support (Phase 4)
- ❌ Kubernetes deployment (Phase 5)
- ❌ Production rollout (Phase 6)
- ❌ Grafana dashboards (Phase 5)
- ❌ Alerting rules (Phase 5)
- ❌ Multi-region deployment (Future)
- ❌ Custom model training (Future)

**Explicitly Out of Scope:**
- ❌ Performance optimization (done in Phase 2)
- ❌ API changes (maintain EmbeddingProvider trait)
- ❌ MLX improvements (deprecated path)

---

## Technical Design

### Architecture Overview

```
┌──────────────────────────────────────────────────────────────────┐
│                      Observability Layer (NEW)                    │
│  ┌──────────────────┐  ┌──────────────────┐  ┌────────────────┐ │
│  │ Prometheus       │  │ OpenTelemetry    │  │ Structured     │ │
│  │ Metrics          │  │ Tracing          │  │ Logging        │ │
│  │ - QPS, Latency   │  │ - Request spans  │  │ - JSON logs    │ │
│  │ - Error rates    │  │ - Batch spans    │  │ - Context      │ │
│  │ - GPU util       │  │ - Model spans    │  │ - Correlation  │ │
│  └──────────────────┘  └──────────────────┘  └────────────────┘ │
└────────────────────────┬─────────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────────┐
│                    REST/gRPC API Layer                            │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Health Checks (NEW)                                       │   │
│  │ - GET /health/live   → "ok" (process alive)              │   │
│  │ - GET /health/ready  → "ok" (model loaded, GPU ready)    │   │
│  └──────────────────────────────────────────────────────────┘   │
└────────────────────────┬─────────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────────┐
│                 Resilience Layer (NEW)                            │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Circuit Breaker                                           │   │
│  │ - Failure threshold: 5 failures in 10s → OPEN            │   │
│  │ - Half-open after 30s → test with 1 request              │   │
│  │ - Success → CLOSED, Failure → OPEN again                 │   │
│  └──────────────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Retry Logic                                               │   │
│  │ - Exponential backoff: 100ms, 200ms, 400ms               │   │
│  │ - Max 3 retries for transient failures                   │   │
│  └──────────────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Timeout Enforcement                                       │   │
│  │ - Inference timeout: 5 seconds                            │   │
│  │ - Request timeout: 10 seconds                             │   │
│  └──────────────────────────────────────────────────────────┘   │
└────────────────────────┬─────────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────────┐
│              CandleEmbeddingProvider (Phase 1+2)                  │
│  - Dynamic batching                                               │
│  - Model caching                                                  │
│  - GPU optimization                                               │
└──────────────────────────────────────────────────────────────────┘
```

### Component Design

#### 1. Observability Layer

**Purpose:** Monitor system health and performance in production

##### 1.1 Prometheus Metrics

```rust
use prometheus::{
    IntCounter, IntGauge, Histogram, HistogramVec,
    register_int_counter, register_int_gauge,
    register_histogram_vec, Encoder, TextEncoder,
};

/// Metrics for Candle embedding provider
pub struct CandleMetrics {
    // Request metrics
    pub requests_total: IntCounter,
    pub requests_in_flight: IntGauge,
    pub request_duration: HistogramVec,

    // Error metrics
    pub errors_total: IntCounter,
    pub circuit_breaker_state: IntGauge,  // 0=closed, 1=half-open, 2=open

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
            ).unwrap(),

            requests_in_flight: register_int_gauge!(
                "candle_requests_in_flight",
                "Current number of requests being processed"
            ).unwrap(),

            request_duration: register_histogram_vec!(
                "candle_request_duration_seconds",
                "Request duration in seconds",
                &["status"],  // success, error
                vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]
            ).unwrap(),

            errors_total: register_int_counter!(
                "candle_errors_total",
                "Total number of errors"
            ).unwrap(),

            circuit_breaker_state: register_int_gauge!(
                "candle_circuit_breaker_state",
                "Circuit breaker state (0=closed, 1=half-open, 2=open)"
            ).unwrap(),

            batch_size: register_histogram!(
                "candle_batch_size",
                "Number of texts in batch",
                vec![1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0]
            ).unwrap(),

            batch_wait_time: register_histogram!(
                "candle_batch_wait_seconds",
                "Time spent waiting for batch to fill",
                vec![0.001, 0.005, 0.01, 0.02, 0.05, 0.1]
            ).unwrap(),

            model_load_duration: register_histogram!(
                "candle_model_load_duration_seconds",
                "Model load duration in seconds",
                vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0]
            ).unwrap(),

            model_inference_duration: register_histogram!(
                "candle_model_inference_duration_seconds",
                "Model inference duration in seconds",
                vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1]
            ).unwrap(),

            gpu_utilization: register_int_gauge!(
                "candle_gpu_utilization_percent",
                "GPU utilization percentage"
            ).unwrap(),

            memory_usage_bytes: register_int_gauge!(
                "candle_memory_usage_bytes",
                "Memory usage in bytes"
            ).unwrap(),
        }
    }

    /// Export metrics in Prometheus format
    pub fn export(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = prometheus::gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    }
}

// Global metrics instance
lazy_static::lazy_static! {
    pub static ref METRICS: CandleMetrics = CandleMetrics::new();
}
```

**Usage Example:**
```rust
impl CandleEmbeddingProvider {
    pub async fn embed_batch_internal(
        &self,
        texts: Vec<String>,
    ) -> EmbeddingResult<Vec<Vec<f32>>> {
        // Track in-flight requests
        METRICS.requests_in_flight.inc();
        let _guard = scopeguard::guard((), |_| {
            METRICS.requests_in_flight.dec();
        });

        // Track request count
        METRICS.requests_total.inc();

        // Track batch size
        METRICS.batch_size.observe(texts.len() as f64);

        let start = Instant::now();
        let result = self.embed_internal(texts).await;
        let duration = start.elapsed().as_secs_f64();

        // Track duration by status
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
}
```

##### 1.2 OpenTelemetry Tracing

```rust
use tracing::{info, warn, error, instrument, Span};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use opentelemetry::{global, sdk::trace::Tracer, trace::TraceError};
use opentelemetry_otlp::WithExportConfig;

/// Initialize tracing with OpenTelemetry
pub fn init_tracing(service_name: &str) -> Result<(), TraceError> {
    global::set_text_map_propagator(
        opentelemetry::sdk::propagation::TraceContextPropagator::new()
    );

    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint("http://localhost:4317")
        )
        .with_trace_config(
            opentelemetry::sdk::trace::config()
                .with_resource(opentelemetry::sdk::Resource::new(vec![
                    opentelemetry::KeyValue::new("service.name", service_name.to_string()),
                ]))
        )
        .install_batch(opentelemetry::runtime::Tokio)?;

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into())
        ))
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .init();

    Ok(())
}

impl CandleEmbeddingProvider {
    /// Generate embeddings with tracing
    #[instrument(
        skip(self, texts),
        fields(
            batch_size = texts.len(),
            model = %self.model_name,
        )
    )]
    pub async fn embed_batch_internal(
        &self,
        texts: Vec<String>,
    ) -> EmbeddingResult<Vec<Vec<f32>>> {
        info!("Starting batch embedding");

        // Tokenization span
        let embeddings = {
            let _span = tracing::info_span!("tokenize").entered();
            let input_ids = self.tokenize_batch(&texts)?;

            // Inference span
            let _span = tracing::info_span!(
                "inference",
                device = ?self.device
            ).entered();

            self.run_inference(input_ids).await?
        };

        info!("Completed batch embedding successfully");
        Ok(embeddings)
    }
}
```

##### 1.3 Structured Logging

```rust
use serde_json::json;
use tracing::{event, Level};

/// Log with structured context
pub fn log_embedding_request(
    request_id: &str,
    batch_size: usize,
    duration_ms: u64,
    success: bool,
) {
    event!(
        Level::INFO,
        request_id = %request_id,
        batch_size = batch_size,
        duration_ms = duration_ms,
        success = success,
        "Embedding request completed"
    );
}

/// Log error with full context
pub fn log_embedding_error(
    request_id: &str,
    error: &EmbeddingError,
    context: &ErrorContext,
) {
    event!(
        Level::ERROR,
        request_id = %request_id,
        error = %error,
        model = %context.model_name,
        batch_size = context.batch_size,
        device = ?context.device,
        "Embedding request failed"
    );
}
```

#### 2. Resilience Layer

**Purpose:** Handle failures gracefully and prevent cascade failures

##### 2.1 Circuit Breaker

```rust
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,     // Normal operation
    Open,       // Failing, reject requests
    HalfOpen,   // Testing if service recovered
}

pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    failure_count: Arc<RwLock<usize>>,
    last_failure_time: Arc<RwLock<Option<Instant>>>,
    config: CircuitBreakerConfig,
}

#[derive(Clone)]
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
            last_failure_time: Arc::new(RwLock::new(None)),
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
                        METRICS.circuit_breaker_state.set(1);
                        tracing::info!("Circuit breaker: OPEN → HALF-OPEN");
                    } else {
                        // Still open, reject request
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
            // Success in half-open → close circuit
            *self.state.write().await = CircuitState::Closed;
            *self.failure_count.write().await = 0;
            METRICS.circuit_breaker_state.set(0);
            tracing::info!("Circuit breaker: HALF-OPEN → CLOSED");
        }
    }

    async fn record_failure(&self) {
        let mut count = self.failure_count.write().await;
        *count += 1;

        let mut last_failure = self.last_failure_time.write().await;
        *last_failure = Some(Instant::now());

        if *count >= self.config.failure_threshold {
            // Open circuit
            *self.state.write().await = CircuitState::Open;
            METRICS.circuit_breaker_state.set(2);
            tracing::warn!(
                "Circuit breaker: CLOSED → OPEN (failures: {})",
                *count
            );
        }
    }

    pub async fn get_state(&self) -> CircuitState {
        *self.state.read().await
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CircuitBreakerError<E> {
    #[error("Circuit breaker is open")]
    CircuitOpen,

    #[error("Inner error: {0}")]
    InnerError(E),
}
```

**Usage Example:**
```rust
pub struct ResilientCandleProvider {
    inner: Arc<CandleEmbeddingProvider>,
    circuit_breaker: Arc<CircuitBreaker>,
}

impl ResilientCandleProvider {
    pub async fn embed_batch(
        &self,
        texts: Vec<String>,
    ) -> Result<Vec<Vec<f32>>, CircuitBreakerError<EmbeddingError>> {
        self.circuit_breaker
            .call(self.inner.embed_batch_internal(texts))
            .await
    }
}
```

##### 2.2 Retry Logic with Exponential Backoff

```rust
use std::time::Duration;

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

pub async fn retry_with_backoff<F, T, E>(
    config: &RetryConfig,
    mut f: F,
) -> Result<T, E>
where
    F: FnMut() -> Pin<Box<dyn Future<Output = Result<T, E>>>>,
    E: std::fmt::Display,
{
    let mut attempt = 0;
    let mut backoff = config.initial_backoff;

    loop {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                attempt += 1;

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
                backoff = (backoff.as_secs_f64() * config.multiplier)
                    .min(config.max_backoff.as_secs_f64());
                backoff = Duration::from_secs_f64(backoff);
            }
        }
    }
}
```

**Usage Example:**
```rust
pub async fn embed_with_retry(
    provider: &CandleEmbeddingProvider,
    texts: Vec<String>,
) -> EmbeddingResult<Vec<Vec<f32>>> {
    let config = RetryConfig::default();

    retry_with_backoff(&config, || {
        Box::pin(provider.embed_batch_internal(texts.clone()))
    }).await
}
```

##### 2.3 Timeout Enforcement

```rust
use tokio::time::timeout;

pub async fn embed_with_timeout(
    provider: &CandleEmbeddingProvider,
    texts: Vec<String>,
    timeout_duration: Duration,
) -> EmbeddingResult<Vec<Vec<f32>>> {
    match timeout(timeout_duration, provider.embed_batch_internal(texts)).await {
        Ok(result) => result,
        Err(_) => {
            tracing::error!("Embedding request timed out after {:?}", timeout_duration);
            Err(EmbeddingError::Timeout)
        }
    }
}
```

#### 3. Health Check System

**Purpose:** Enable Kubernetes to determine if service is ready

```rust
use axum::{Router, routing::get, Json};
use serde::{Serialize, Deserialize};

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
    provider: Arc<CandleEmbeddingProvider>,
    circuit_breaker: Arc<CircuitBreaker>,
) -> Json<HealthStatus> {
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
}

/// Add health check routes to router
pub fn health_routes() -> Router {
    Router::new()
        .route("/health/live", get(liveness))
        .route("/health/ready", get(readiness))
}
```

**Kubernetes Configuration:**
```yaml
apiVersion: v1
kind: Pod
spec:
  containers:
  - name: akidb-embedding
    image: akidb/embedding:candle
    livenessProbe:
      httpGet:
        path: /health/live
        port: 8080
      initialDelaySeconds: 10
      periodSeconds: 30
    readinessProbe:
      httpGet:
        path: /health/ready
        port: 8080
      initialDelaySeconds: 30
      periodSeconds: 10
```

---

## Observability Strategy

### Metrics to Track

| Category | Metric | Type | Purpose |
|----------|--------|------|---------|
| **Requests** | `candle_requests_total` | Counter | Total requests processed |
| | `candle_requests_in_flight` | Gauge | Current concurrent requests |
| | `candle_request_duration_seconds` | Histogram | Request latency distribution |
| **Errors** | `candle_errors_total` | Counter | Total errors encountered |
| | `candle_circuit_breaker_state` | Gauge | Circuit breaker state |
| **Batching** | `candle_batch_size` | Histogram | Batch size distribution |
| | `candle_batch_wait_seconds` | Histogram | Batching wait time |
| **Model** | `candle_model_load_duration_seconds` | Histogram | Model load time |
| | `candle_model_inference_duration_seconds` | Histogram | Inference time |
| **Resources** | `candle_gpu_utilization_percent` | Gauge | GPU utilization |
| | `candle_memory_usage_bytes` | Gauge | Memory usage |

### Tracing Strategy

**Trace Hierarchy:**
```
embedding_request (span)
├─ validation (span)
├─ batching (span)
│  └─ batch_wait (event)
├─ tokenize (span)
│  └─ parallel_tokenize (event)
├─ inference (span)
│  ├─ gpu_transfer_in (event)
│  ├─ forward_pass (event)
│  ├─ mean_pooling (event)
│  ├─ normalize (event)
│  └─ gpu_transfer_out (event)
└─ result_distribution (span)
```

### Logging Strategy

**Log Levels:**
- **TRACE:** Fine-grained debugging (tensor shapes, intermediate values)
- **DEBUG:** Developer diagnostics (batch formation, cache hits)
- **INFO:** Normal operations (request completed, model loaded)
- **WARN:** Recoverable errors (retry, fallback to CPU)
- **ERROR:** Unrecoverable errors (model load failed, GPU error)

**Structured Fields:**
```rust
tracing::info!(
    request_id = %request_id,
    batch_size = batch_size,
    duration_ms = duration.as_millis(),
    model = %model_name,
    device = ?device,
    "Embedding request completed"
);
```

---

## Resilience Patterns

### Pattern 1: Circuit Breaker

**When to Use:** Protect against cascade failures when downstream service (GPU, model) is failing

**Configuration:**
```rust
CircuitBreakerConfig {
    failure_threshold: 5,              // Open after 5 failures
    failure_window: Duration::from_secs(10),  // Within 10 seconds
    recovery_timeout: Duration::from_secs(30), // Try recovery after 30s
    success_threshold: 2,              // Close after 2 successes
}
```

**State Transitions:**
```
CLOSED (normal) --[5 failures in 10s]--> OPEN (failing)
OPEN --[30s elapsed]--> HALF-OPEN (testing)
HALF-OPEN --[2 successes]--> CLOSED
HALF-OPEN --[1 failure]--> OPEN
```

### Pattern 2: Retry with Exponential Backoff

**When to Use:** Handle transient failures (temporary GPU unavailability)

**Configuration:**
```rust
RetryConfig {
    max_retries: 3,
    initial_backoff: Duration::from_millis(100),  // 100ms
    max_backoff: Duration::from_secs(5),          // 5s cap
    multiplier: 2.0,  // 100ms → 200ms → 400ms
}
```

**Retry Decision Matrix:**

| Error Type | Retry? | Reason |
|------------|--------|--------|
| Model not loaded | ✅ Yes | Transient, may load soon |
| GPU unavailable | ✅ Yes | May recover (driver issue) |
| Invalid input | ❌ No | User error, won't change |
| Out of memory | ❌ No | Won't recover without restart |
| Timeout | ✅ Yes | May be temporary congestion |

### Pattern 3: Graceful Degradation

**Fallback Chain:**
```
1. Try GPU inference (Metal/CUDA)
   ↓ (failure)
2. Try CPU inference
   ↓ (failure)
3. Return cached result (if available)
   ↓ (failure)
4. Return error to client
```

**Implementation:**
```rust
pub async fn embed_with_fallback(
    provider: &CandleEmbeddingProvider,
    texts: Vec<String>,
) -> EmbeddingResult<Vec<Vec<f32>>> {
    // Try GPU
    match provider.embed_batch_internal(texts.clone()).await {
        Ok(result) => return Ok(result),
        Err(e) => {
            tracing::warn!("GPU inference failed: {}. Trying CPU...", e);
        }
    }

    // Try CPU
    let cpu_provider = provider.clone_with_device(Device::Cpu);
    match cpu_provider.embed_batch_internal(texts.clone()).await {
        Ok(result) => {
            tracing::warn!("Using CPU fallback");
            return Ok(result);
        }
        Err(e) => {
            tracing::error!("CPU fallback failed: {}", e);
        }
    }

    // No fallback available
    Err(EmbeddingError::ServiceUnavailable(
        "All inference backends failed".to_string()
    ))
}
```

### Pattern 4: Timeout Enforcement

**Timeout Hierarchy:**
- **Inference timeout:** 5 seconds (per batch)
- **Request timeout:** 10 seconds (end-to-end)
- **Model load timeout:** 60 seconds (initialization)

```rust
// Request-level timeout
pub async fn handle_request(
    provider: Arc<CandleEmbeddingProvider>,
    request: EmbeddingRequest,
) -> Result<EmbeddingResponse> {
    tokio::time::timeout(
        Duration::from_secs(10),
        process_request(provider, request)
    )
    .await
    .map_err(|_| Error::RequestTimeout)?
}

// Inference-level timeout
async fn process_request(
    provider: Arc<CandleEmbeddingProvider>,
    request: EmbeddingRequest,
) -> Result<EmbeddingResponse> {
    let embeddings = tokio::time::timeout(
        Duration::from_secs(5),
        provider.embed_batch_internal(request.texts)
    )
    .await
    .map_err(|_| Error::InferenceTimeout)??;

    Ok(EmbeddingResponse { embeddings })
}
```

---

## Health Check Design

### Liveness Probe

**Purpose:** Kubernetes uses this to determine if container should be restarted

**Check:** Process is alive and HTTP server is responding

**Endpoint:** `GET /health/live`

**Response:**
```json
{
  "status": "ok"
}
```

**Failure Condition:** HTTP server not responding (process crashed)

**K8s Action:** Restart container

### Readiness Probe

**Purpose:** Kubernetes uses this to determine if service should receive traffic

**Checks:**
1. ✅ Model is loaded
2. ✅ GPU is available (if required)
3. ✅ Circuit breaker is not open
4. ✅ Memory usage < 2GB

**Endpoint:** `GET /health/ready`

**Response (Healthy):**
```json
{
  "status": "ok",
  "details": {
    "model_loaded": true,
    "gpu_available": true,
    "circuit_breaker_state": "Closed",
    "memory_usage_mb": 512,
    "uptime_seconds": 3600
  }
}
```

**Response (Unhealthy):**
```json
{
  "status": "error",
  "details": {
    "model_loaded": true,
    "gpu_available": false,
    "circuit_breaker_state": "Open",
    "memory_usage_mb": 2048,
    "uptime_seconds": 300
  }
}
```

**Failure Condition:** Any check fails

**K8s Action:** Remove from load balancer, stop sending traffic

---

## Integration Testing

### Test Categories

#### 1. End-to-End Tests (8 tests)

**Purpose:** Validate full request flow from API to response

```rust
#[tokio::test]
async fn test_e2e_single_request() {
    let provider = setup_test_provider().await;

    let request = EmbeddingRequest {
        texts: vec!["Hello, world!".to_string()],
    };

    let response = provider.embed_batch(request).await.unwrap();

    assert_eq!(response.embeddings.len(), 1);
    assert_eq!(response.embeddings[0].len(), 384);
    assert!(response.model.contains("MiniLM"));
}

#[tokio::test]
async fn test_e2e_batch_request() {
    let provider = setup_test_provider().await;

    let request = EmbeddingRequest {
        texts: vec![
            "Text 1".to_string(),
            "Text 2".to_string(),
            "Text 3".to_string(),
        ],
    };

    let response = provider.embed_batch(request).await.unwrap();

    assert_eq!(response.embeddings.len(), 3);
}

#[tokio::test]
async fn test_e2e_concurrent_requests() {
    let provider = Arc::new(setup_test_provider().await);

    let tasks: Vec<_> = (0..50)
        .map(|i| {
            let provider = Arc::clone(&provider);
            tokio::spawn(async move {
                let request = EmbeddingRequest {
                    texts: vec![format!("Test {}", i)],
                };
                provider.embed_batch(request).await
            })
        })
        .collect();

    let results: Vec<_> = futures::future::join_all(tasks).await;

    // All should succeed
    for result in results {
        assert!(result.unwrap().is_ok());
    }
}

#[tokio::test]
async fn test_e2e_error_handling() {
    let provider = setup_test_provider().await;

    let request = EmbeddingRequest {
        texts: vec![],  // Invalid: empty
    };

    let result = provider.embed_batch(request).await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), EmbeddingError::InvalidInput(_)));
}

// TODO: Add 4 more E2E tests
// - test_e2e_large_batch
// - test_e2e_long_text
// - test_e2e_special_characters
// - test_e2e_metrics_exported
```

#### 2. Resilience Tests (6 tests)

**Purpose:** Validate circuit breaker, retry, timeout behavior

```rust
#[tokio::test]
async fn test_circuit_breaker_opens() {
    let provider = Arc::new(FailingProvider::new());
    let cb = Arc::new(CircuitBreaker::new(CircuitBreakerConfig::default()));

    // Trigger 5 failures
    for _ in 0..5 {
        let _ = cb.call(provider.embed_batch(vec!["test".to_string()])).await;
    }

    // Circuit should be open
    assert_eq!(cb.get_state().await, CircuitState::Open);

    // Next request should be rejected immediately
    let result = cb.call(provider.embed_batch(vec!["test".to_string()])).await;
    assert!(matches!(result, Err(CircuitBreakerError::CircuitOpen)));
}

#[tokio::test]
async fn test_circuit_breaker_recovers() {
    let provider = Arc::new(RecoveringProvider::new());
    let cb = Arc::new(CircuitBreaker::new(CircuitBreakerConfig {
        failure_threshold: 3,
        recovery_timeout: Duration::from_millis(100),
        ..Default::default()
    }));

    // Trigger 3 failures
    for _ in 0..3 {
        let _ = cb.call(provider.fail_next()).await;
    }

    assert_eq!(cb.get_state().await, CircuitState::Open);

    // Wait for recovery timeout
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Provider recovers
    provider.recover();

    // Should transition to half-open and succeed
    let result = cb.call(provider.embed_batch(vec!["test".to_string()])).await;
    assert!(result.is_ok());

    // Should be closed now
    assert_eq!(cb.get_state().await, CircuitState::Closed);
}

#[tokio::test]
async fn test_retry_with_backoff() {
    let provider = Arc::new(TransientFailureProvider::new(2));  // Fail 2 times

    let result = retry_with_backoff(
        &RetryConfig::default(),
        || Box::pin(provider.embed_batch(vec!["test".to_string()]))
    ).await;

    // Should succeed after 2 retries
    assert!(result.is_ok());
    assert_eq!(provider.attempt_count(), 3);  // 1 initial + 2 retries
}

#[tokio::test]
async fn test_timeout_enforcement() {
    let provider = Arc::new(SlowProvider::new(Duration::from_secs(10)));

    let result = tokio::time::timeout(
        Duration::from_secs(1),
        provider.embed_batch(vec!["test".to_string()])
    ).await;

    assert!(result.is_err());  // Timeout
}

#[tokio::test]
async fn test_graceful_degradation() {
    let provider = setup_test_provider_with_failing_gpu().await;

    // GPU should fail, CPU should succeed
    let result = provider.embed_with_fallback(vec!["test".to_string()]).await;

    assert!(result.is_ok());
    assert_eq!(provider.last_device_used(), Device::Cpu);
}

// TODO: Add 1 more resilience test
// - test_retry_exhaustion
```

#### 3. Health Check Tests (3 tests)

```rust
#[tokio::test]
async fn test_liveness_probe() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health/live")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let health: HealthStatus = serde_json::from_slice(&body).unwrap();

    assert_eq!(health.status, "ok");
}

#[tokio::test]
async fn test_readiness_probe_healthy() {
    let app = create_test_app_healthy().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health/ready")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let health: HealthStatus = serde_json::from_slice(&body).unwrap();

    assert_eq!(health.status, "ok");
    assert!(health.details.unwrap().model_loaded);
}

#[tokio::test]
async fn test_readiness_probe_unhealthy() {
    let app = create_test_app_unhealthy().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health/ready")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let health: HealthStatus = serde_json::from_slice(&body).unwrap();

    assert_eq!(health.status, "error");
}
```

#### 4. Observability Tests (3 tests)

```rust
#[tokio::test]
async fn test_metrics_exported() {
    let provider = setup_test_provider().await;

    // Generate some requests
    for _ in 0..10 {
        let _ = provider.embed_batch(vec!["test".to_string()]).await;
    }

    // Export metrics
    let metrics_text = METRICS.export();

    // Verify key metrics present
    assert!(metrics_text.contains("candle_requests_total"));
    assert!(metrics_text.contains("candle_request_duration_seconds"));
    assert!(metrics_text.contains("candle_batch_size"));
}

#[tokio::test]
async fn test_tracing_spans_created() {
    // This requires integration with opentelemetry test utilities
    // TODO: Implement when opentelemetry test helpers available
}

#[tokio::test]
async fn test_structured_logging() {
    // This requires capturing log output
    // TODO: Implement log capture and verification
}
```

### Total Integration Tests

**Phase 3 Test Count:**
- E2E tests: 8
- Resilience tests: 6
- Health check tests: 3
- Observability tests: 3
- **Total new tests: 20**

**Cumulative Test Count:**
- Phase 1: 15 tests
- Phase 2: 21 tests
- Phase 3: 20 tests
- **Total: 56 tests**

---

## Chaos Engineering

### Chaos Tests

**Purpose:** Validate system behavior under adverse conditions

#### 1. CPU Exhaustion Test

```rust
#[tokio::test]
#[ignore]  // Run manually, resource-intensive
async fn chaos_cpu_exhaustion() {
    let provider = Arc::new(setup_test_provider().await);

    // Spawn CPU-intensive tasks
    let cpu_tasks: Vec<_> = (0..num_cpus::get())
        .map(|_| {
            tokio::task::spawn_blocking(|| {
                // Burn CPU
                loop {
                    let _ = (0..1_000_000).sum::<i64>();
                }
            })
        })
        .collect();

    // Try embedding requests under CPU load
    let mut success_count = 0;
    for _ in 0..100 {
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            provider.embed_batch(vec!["test".to_string()])
        ).await;

        if result.is_ok() {
            success_count += 1;
        }
    }

    // Should have >80% success rate even under CPU stress
    assert!(
        success_count >= 80,
        "Success rate too low: {}/100",
        success_count
    );

    // Cleanup
    for task in cpu_tasks {
        task.abort();
    }
}
```

#### 2. Memory Pressure Test

```rust
#[tokio::test]
#[ignore]
async fn chaos_memory_pressure() {
    let provider = Arc::new(setup_test_provider().await);

    // Allocate large memory blocks
    let mut memory_hog = Vec::new();
    for _ in 0..10 {
        memory_hog.push(vec![0u8; 100 * 1024 * 1024]);  // 100MB each
    }

    // Try embedding requests under memory pressure
    let result = provider.embed_batch(vec!["test".to_string()]).await;

    // Should either succeed or return clear OOM error
    if let Err(e) = result {
        assert!(matches!(e, EmbeddingError::OutOfMemory(_)));
    }
}
```

#### 3. Network Partition Test

```rust
#[tokio::test]
#[ignore]
async fn chaos_network_partition() {
    // Simulate network partition during model download
    // This requires mocking the HF Hub API

    let provider_result = CandleEmbeddingProvider::new_with_config(
        "sentence-transformers/all-MiniLM-L6-v2",
        Config {
            hf_hub_url: "http://unreachable:9999",  // Unreachable
            download_timeout: Duration::from_secs(5),
            ..Default::default()
        }
    ).await;

    // Should fail gracefully with clear error
    assert!(provider_result.is_err());
    assert!(matches!(
        provider_result.unwrap_err(),
        EmbeddingError::ModelDownloadFailed(_)
    ));
}
```

#### 4. GPU Failure Test

```rust
#[tokio::test]
#[ignore]
async fn chaos_gpu_failure() {
    let provider = setup_test_provider_with_gpu().await;

    // Simulate GPU failure mid-request
    // (Implementation depends on ability to trigger GPU errors)

    // Should fall back to CPU
    let result = provider.embed_with_fallback(vec!["test".to_string()]).await;

    assert!(result.is_ok());
    assert_eq!(provider.last_device_used(), Device::Cpu);
}
```

#### 5. Thundering Herd Test

```rust
#[tokio::test]
#[ignore]
async fn chaos_thundering_herd() {
    let provider = Arc::new(setup_test_provider().await);

    // Simulate 1000 concurrent requests arriving simultaneously
    let tasks: Vec<_> = (0..1000)
        .map(|i| {
            let provider = Arc::clone(&provider);
            tokio::spawn(async move {
                provider.embed_batch(vec![format!("Test {}", i)]).await
            })
        })
        .collect();

    let results: Vec<_> = futures::future::join_all(tasks).await;

    let success_count = results.iter()
        .filter(|r| r.as_ref().unwrap().is_ok())
        .count();

    // Should handle gracefully with >95% success
    assert!(
        success_count >= 950,
        "Success rate too low: {}/1000",
        success_count
    );
}
```

---

## Success Criteria

### Functional Requirements

✅ **FR1:** Prometheus metrics exported at `/metrics` endpoint
✅ **FR2:** OpenTelemetry tracing integrated with Jaeger
✅ **FR3:** Circuit breaker prevents cascade failures
✅ **FR4:** Retry logic with exponential backoff
✅ **FR5:** Graceful degradation (GPU → CPU fallback)
✅ **FR6:** Health checks for liveness and readiness
✅ **FR7:** 20+ integration tests passing
✅ **FR8:** 5 chaos tests validating resilience

### Non-Functional Requirements

✅ **NFR1: Observability**
- 10+ Prometheus metrics exported
- Distributed tracing with <1ms overhead
- Structured logging with correlation IDs

✅ **NFR2: Reliability**
- 99.9% uptime under normal load
- Circuit breaker opens within 10s of failures
- Graceful degradation to CPU fallback

✅ **NFR3: Debuggability**
- All errors include context and correlation IDs
- Traces show end-to-end request flow
- Metrics enable troubleshooting without logs

✅ **NFR4: Resilience**
- Survive 80% CPU utilization
- Survive memory pressure up to 90%
- Survive thundering herd (1000 concurrent requests)
- Recover from transient GPU failures

✅ **NFR5: Production Readiness**
- K8s readiness probe works correctly
- No crash under chaos tests
- Clear error messages for operators
- Operations runbook documented

---

## Risks & Mitigation

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| **Metrics overhead >5%** | Medium | Medium | • Use counters/gauges (not histograms) for hot path<br>• Batch metric updates<br>• Benchmark with/without metrics |
| **Circuit breaker too sensitive** | High | High | • Tune thresholds based on real traffic<br>• Add adaptive thresholds<br>• Manual override option |
| **Tracing overhead >2%** | Low | Medium | • Use sampling (10% of requests)<br>• Disable in benchmarks<br>• Make configurable |
| **Health checks too slow** | Medium | High | • Cache health status (5s TTL)<br>• Parallel checks<br>• Timeout after 1s |
| **Integration tests flaky** | High | Medium | • Use deterministic test providers<br>• Increase timeouts<br>• Retry flaky tests 3x |
| **Chaos tests break CI** | High | Low | • Mark as `#[ignore]`<br>• Run in separate CI job<br>• Only run on demand |
| **Observability complexity** | Medium | Medium | • Start simple (key metrics only)<br>• Add gradually<br>• Document clearly |

---

## Timeline & Milestones

### Week 3 Schedule (5 days, Monday-Friday)

#### **Day 1 (Monday): Observability - Metrics (6 hours)**

**Tasks:**
1. ☐ Add Prometheus dependencies (30 min)
2. ☐ Implement CandleMetrics struct (2 hours)
3. ☐ Instrument embedding provider (2 hours)
4. ☐ Add /metrics endpoint (1 hour)
5. ☐ Test metrics export (30 min)

**Deliverables:**
- `src/metrics.rs` (~200 lines)
- 10 Prometheus metrics
- `/metrics` endpoint working
- Metrics export test

**Success Criteria:**
- ✅ All metrics exported correctly
- ✅ Metrics overhead <5%
- ✅ Metrics documented

#### **Day 2 (Tuesday): Observability - Tracing + Logging (6 hours)**

**Tasks:**
1. ☐ Add OpenTelemetry dependencies (30 min)
2. ☐ Implement tracing initialization (1 hour)
3. ☐ Add instrumentation to hot paths (2 hours)
4. ☐ Implement structured logging (1.5 hours)
5. ☐ Test with Jaeger (1 hour)

**Deliverables:**
- `src/tracing_init.rs` (~150 lines)
- Tracing spans on all operations
- Structured logging with context
- Jaeger integration tested

**Success Criteria:**
- ✅ Traces visible in Jaeger
- ✅ Tracing overhead <2%
- ✅ Logs are structured JSON

#### **Day 3 (Wednesday): Resilience (6 hours)**

**Tasks:**
1. ☐ Implement CircuitBreaker (2.5 hours)
2. ☐ Implement retry with backoff (1.5 hours)
3. ☐ Implement timeout enforcement (1 hour)
4. ☐ Implement graceful degradation (1 hour)

**Deliverables:**
- `src/circuit_breaker.rs` (~250 lines)
- `src/retry.rs` (~100 lines)
- Circuit breaker tests (6 tests)
- Retry tests

**Success Criteria:**
- ✅ Circuit breaker opens/closes correctly
- ✅ Retry works with backoff
- ✅ Timeouts enforced
- ✅ CPU fallback works

#### **Day 4 (Thursday): Health Checks + Integration Tests (6 hours)**

**Tasks:**
1. ☐ Implement health check endpoints (1.5 hours)
2. ☐ Write E2E tests (2 hours)
3. ☐ Write resilience tests (1.5 hours)
4. ☐ Write health check tests (1 hour)

**Deliverables:**
- `src/health.rs` (~150 lines)
- 8 E2E tests
- 6 resilience tests
- 3 health check tests
- 3 observability tests
- **Total: 20 new tests**

**Success Criteria:**
- ✅ All 20 tests passing
- ✅ Health checks work with K8s
- ✅ No flaky tests

#### **Day 5 (Friday): Chaos Testing + Documentation (6 hours)**

**Tasks:**
1. ☐ Implement 5 chaos tests (2.5 hours)
2. ☐ Run chaos test suite (1 hour)
3. ☐ Write operations runbook (1.5 hours)
4. ☐ Write Phase 3 completion report (1 hour)

**Deliverables:**
- 5 chaos tests (marked `#[ignore]`)
- `docs/OPERATIONS-RUNBOOK.md`
- Phase 3 completion report
- RC2 release tag

**Success Criteria:**
- ✅ All chaos tests pass
- ✅ 99.9% success rate under stress
- ✅ Operations runbook complete
- ✅ Phase 3 COMPLETE → RC2 🎉

### Phase 3 Milestones

- **M1 (Day 1 EOD):** Prometheus metrics exported
- **M2 (Day 2 EOD):** Tracing and logging integrated
- **M3 (Day 3 EOD):** Resilience patterns implemented
- **M4 (Day 4 EOD):** 20 integration tests passing
- **M5 (Day 5 EOD):** Chaos tests passing + RC2 READY 🎉

---

## Dependencies

### Internal Dependencies

**From Phase 1+2:**
- ✅ CandleEmbeddingProvider (working, optimized)
- ✅ Dynamic batching
- ✅ Model caching
- ✅ 36 tests passing
- ✅ 200+ QPS throughput

**Blockers:**
- ❌ None (Phase 2 complete)

### External Dependencies

**New Rust Crates:**
```toml
[dependencies]
# Existing
candle-core = "0.8"
# ... (Phase 1+2 deps)

# NEW for Phase 3
prometheus = "0.13"
lazy_static = "1.4"
scopeguard = "1.2"
opentelemetry = "0.21"
opentelemetry-otlp = "0.14"
tracing-opentelemetry = "0.22"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
thiserror = "1.0"

[dev-dependencies]
# NEW for chaos tests
num_cpus = "1.16"
```

**System Tools:**
```bash
# Observability stack (for local testing)
docker compose up -d prometheus grafana jaeger

# Or install individually
brew install prometheus  # macOS
apt install prometheus   # Linux
```

---

## Deliverables

### Code Deliverables

| File | Lines | Description |
|------|-------|-------------|
| `src/metrics.rs` | ~200 | Prometheus metrics |
| `src/tracing_init.rs` | ~150 | OpenTelemetry setup |
| `src/circuit_breaker.rs` | ~250 | Circuit breaker implementation |
| `src/retry.rs` | ~100 | Retry logic |
| `src/health.rs` | ~150 | Health check endpoints |
| `tests/e2e_tests.rs` | ~200 | End-to-end tests |
| `tests/resilience_tests.rs` | ~150 | Resilience tests |
| `tests/health_tests.rs` | ~80 | Health check tests |
| `tests/observability_tests.rs` | ~80 | Observability tests |
| `tests/chaos_tests.rs` | ~200 | Chaos engineering tests |
| **Total** | **~1,560 lines** | |

### Documentation Deliverables

1. **`docs/OPERATIONS-RUNBOOK.md`** - Operations guide
   - Deployment procedures
   - Troubleshooting guide
   - Metrics interpretation
   - Incident response playbook
   - Common failure modes

2. **Phase 3 Completion Report** - `automatosx/tmp/PHASE-3-COMPLETION-REPORT.md`
   - Summary of accomplishments
   - RC2 readiness assessment
   - Lessons learned
   - Next steps (Phase 4)

### Test Deliverables

- **20 new integration tests**
- **5 chaos tests**
- **Cumulative: 61 total tests** (Phase 1: 15 + Phase 2: 21 + Phase 3: 20 + 5 chaos)

### Production Deliverables

**RC2 Release:**
- ✅ Observability (metrics + tracing + logging)
- ✅ Resilience (circuit breaker + retry + timeout)
- ✅ Health checks (K8s-ready)
- ✅ Comprehensive tests (56 tests + 5 chaos)
- ✅ Operations runbook

---

## Appendix

### A. Prometheus Metrics Reference

```prometheus
# Request metrics
candle_requests_total{} 1234
candle_requests_in_flight{} 5
candle_request_duration_seconds{status="success",quantile="0.5"} 0.013
candle_request_duration_seconds{status="success",quantile="0.95"} 0.032

# Error metrics
candle_errors_total{} 10
candle_circuit_breaker_state{} 0  # 0=closed

# Batch metrics
candle_batch_size{quantile="0.5"} 8
candle_batch_wait_seconds{quantile="0.95"} 0.009

# Model metrics
candle_model_load_duration_seconds{quantile="0.95"} 0.8
candle_model_inference_duration_seconds{quantile="0.95"} 0.011

# Resource metrics
candle_gpu_utilization_percent{} 75
candle_memory_usage_bytes{} 536870912  # 512MB
```

### B. Tracing Example

```
Trace ID: 7f8a9b0c1d2e3f4a
Span: embedding_request (12.5ms)
├─ Span: validation (0.1ms)
├─ Span: batching (8.2ms)
│  └─ Event: batch_wait (8ms)
├─ Span: tokenize (1.2ms)
│  └─ Event: parallel_tokenize (1.1ms)
├─ Span: inference (2.8ms)
│  ├─ Event: gpu_transfer_in (0.3ms)
│  ├─ Event: forward_pass (2.0ms)
│  ├─ Event: mean_pooling (0.2ms)
│  ├─ Event: normalize (0.2ms)
│  └─ Event: gpu_transfer_out (0.1ms)
└─ Span: result_distribution (0.2ms)
```

### C. Health Check Decision Tree

```
/health/ready request
│
├─ Is model loaded?
│  ├─ No → UNHEALTHY (503)
│  └─ Yes → Continue
│
├─ Is GPU available? (if required)
│  ├─ No → UNHEALTHY (503)
│  └─ Yes → Continue
│
├─ Is circuit breaker open?
│  ├─ Yes → UNHEALTHY (503)
│  └─ No → Continue
│
├─ Is memory usage < 2GB?
│  ├─ No → UNHEALTHY (503)
│  └─ Yes → Continue
│
└─ HEALTHY (200)
```

### D. Circuit Breaker Tuning Guide

**Symptom:** Too many false positives (circuit opens unnecessarily)
- **Action:** Increase `failure_threshold` (5 → 10)
- **Action:** Decrease `failure_window` (10s → 30s)

**Symptom:** Not opening fast enough
- **Action:** Decrease `failure_threshold` (5 → 3)
- **Action:** Increase `failure_window` (10s → 5s)

**Symptom:** Circuit stays open too long
- **Action:** Decrease `recovery_timeout` (30s → 15s)

**Symptom:** Flapping (open/close repeatedly)
- **Action:** Increase `success_threshold` (2 → 5)
- **Action:** Increase `recovery_timeout` (30s → 60s)

---

## Sign-Off

**Phase 3 PRD Version:** 1.0
**Status:** ✅ Ready for Implementation
**Estimated Effort:** 30 development hours (5 days × 6 hours)
**Expected Completion:** End of Week 3 → **RC2 Release**

**Next Phase:** [Phase 4: Multi-Model Support](CANDLE-PHASE-4-MULTI-MODEL-PRD.md)

---

**Document End**
