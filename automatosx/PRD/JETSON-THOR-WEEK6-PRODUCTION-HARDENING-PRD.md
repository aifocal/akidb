# Jetson Thor Week 6: Production Hardening & Security PRD

**Status:** Ready to Execute
**Timeline:** 5 days (Week 6)
**Owner:** Backend Team + Security Engineering + DevOps
**Dependencies:** Week 1-5 (✅ Complete)
**Target Platform:** NVIDIA Jetson Thor (Blackwell GPU, 2,000 TOPS)

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Goals & Non-Goals](#goals--non-goals)
3. [Baseline Analysis](#baseline-analysis)
4. [Production Hardening Architecture](#production-hardening-architecture)
5. [Security Architecture](#security-architecture)
6. [Day-by-Day Implementation Plan](#day-by-day-implementation-plan)
7. [Resilience Patterns](#resilience-patterns)
8. [Security Implementation](#security-implementation)
9. [Chaos Engineering](#chaos-engineering)
10. [Risk Management](#risk-management)
11. [Success Criteria](#success-criteria)
12. [Appendix: Code Examples](#appendix-code-examples)

---

## Executive Summary

Week 6 focuses on **production hardening** and **security enhancements**, transforming the deployed API service (Week 5) into an enterprise-grade, resilient, and secure production system. We will implement circuit breakers, rate limiting, backpressure handling, mutual TLS (mTLS), RBAC, API authentication, and chaos engineering tests to validate system resilience under failure conditions.

### Key Objectives

1. **Resilience Patterns:** Circuit breakers, rate limiting, retry with exponential backoff
2. **Backpressure Handling:** Request queuing, load shedding, graceful degradation
3. **Security Layer:** Mutual TLS (mTLS), RBAC, API key authentication
4. **Chaos Engineering:** Fault injection tests (pod failures, network latency, GPU OOM)
5. **Advanced Monitoring:** SLI/SLO tracking, alerting, incident response playbooks
6. **Production Readiness:** Comprehensive runbooks, disaster recovery procedures

### Expected Outcomes

- ✅ Circuit breakers with 3-state FSM (Closed/Open/Half-Open)
- ✅ Rate limiting: 100 req/sec per client, 1000 req/sec global
- ✅ Backpressure: Request queue (size 1000), load shedding at 95% capacity
- ✅ mTLS: Mutual authentication for all internal communication
- ✅ RBAC: Role-based access control with 4 roles (admin, developer, viewer, operator)
- ✅ API Authentication: API key + JWT token validation
- ✅ Chaos tests: 6 scenarios (pod kill, network partition, latency injection, GPU OOM, disk pressure, DNS failure)
- ✅ SLI/SLO: 99.9% availability, P95 <30ms, error rate <0.1%
- ✅ Zero security vulnerabilities (CVE scan clean)

---

## Goals & Non-Goals

### Goals (Week 6)

**Primary Goals:**
1. ✅ **Circuit Breakers** - Prevent cascading failures with 3-state FSM
2. ✅ **Rate Limiting** - Token bucket algorithm, per-client and global limits
3. ✅ **Backpressure Handling** - Request queuing, load shedding, graceful degradation
4. ✅ **Mutual TLS (mTLS)** - Certificate-based mutual authentication
5. ✅ **RBAC** - Kubernetes RBAC + application-level role checks
6. ✅ **API Authentication** - API key + JWT tokens
7. ✅ **Chaos Engineering** - 6 fault injection scenarios
8. ✅ **SLI/SLO Tracking** - Prometheus alerts for SLO violations

**Secondary Goals:**
- 📊 Blue-Green deployment strategy
- 📊 Canary releases with traffic splitting
- 📊 Multi-region failover (active-passive)
- 📝 Comprehensive runbooks (20+ scenarios)
- 📝 Disaster recovery automation

### Non-Goals (Deferred to Week 7+)

**Not in Scope for Week 6:**
- ❌ Multi-region active-active deployment - Week 7
- ❌ CI/CD pipeline with GitOps - Week 7
- ❌ Advanced threat detection (IDS/IPS) - Week 8+
- ❌ Service mesh (Istio/Linkerd) - Week 8+
- ❌ Cost optimization and autoscaling tuning - Week 9+
- ❌ Compliance certifications (SOC2, HIPAA) - Week 10+

---

## Baseline Analysis

### Week 5 Deployment Status

**Infrastructure (Deployed):**
- REST API on `:8080` (Kubernetes service)
- gRPC API on `:9090` (Kubernetes service)
- Docker images: ~1.8GB compressed
- Kubernetes: 2 pods (1 REST, 1 gRPC), GPU scheduled
- Observability: Prometheus + Grafana (15+ metrics)

**Performance (Validated):**
- P95 latency: <30ms @ 50 QPS
- Throughput: >150 QPS concurrent (15 threads)
- GPU memory: ~3.5GB (stable)
- Zero errors in 5-minute stress test

**Current Limitations:**
- ❌ No circuit breakers (cascading failures possible)
- ❌ No rate limiting (vulnerable to DoS)
- ❌ No backpressure (OOM under extreme load)
- ❌ No mTLS (plaintext internal communication)
- ❌ No RBAC (all clients have full access)
- ❌ No API authentication (open endpoints)
- ❌ No chaos testing (unknown failure modes)

### Week 6 Target State

**Resilience (Hardened):**
- ✅ Circuit breakers on all external dependencies
- ✅ Rate limiting with 429 responses
- ✅ Backpressure with request queue (1000 capacity)
- ✅ Retry with exponential backoff (max 3 retries)
- ✅ Timeouts on all I/O operations

**Security (Locked Down):**
- ✅ mTLS for internal pod-to-pod communication
- ✅ RBAC with 4 roles + namespace isolation
- ✅ API key authentication (REST)
- ✅ JWT token validation (gRPC)
- ✅ Network policies (deny-all default)

**Validated Resilience:**
- ✅ Survives pod kill (zero downtime)
- ✅ Handles network partition gracefully
- ✅ Degrades gracefully under GPU OOM
- ✅ Recovers from disk pressure
- ✅ Tolerates DNS failures
- ✅ Maintains SLOs during chaos tests

---

## Production Hardening Architecture

### Resilience Layer Architecture

```
┌────────────────────────────────────────────────────────────────┐
│                        Client                                   │
└─────────────────────────┬──────────────────────────────────────┘
                          │
                          ▼
┌────────────────────────────────────────────────────────────────┐
│                   API Gateway Layer                             │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │ Rate Limiter (Token Bucket)                             │  │
│  │ - Per-client: 100 req/sec                               │  │
│  │ - Global: 1000 req/sec                                  │  │
│  │ - Burst: 20 requests                                    │  │
│  └─────────────────────────────────────────────────────────┘  │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │ Authentication Layer                                    │  │
│  │ - API Key validation (REST)                             │  │
│  │ - JWT token validation (gRPC)                           │  │
│  │ - RBAC enforcement                                      │  │
│  └─────────────────────────────────────────────────────────┘  │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │ Request Queue (Backpressure)                            │  │
│  │ - Capacity: 1000 requests                               │  │
│  │ - Load shedding at 95% (reject with 503)               │  │
│  │ - Priority queue (admin > normal)                       │  │
│  └─────────────────────────────────────────────────────────┘  │
└─────────────────────────┬──────────────────────────────────────┘
                          │
                          ▼
┌────────────────────────────────────────────────────────────────┐
│                   Service Layer                                 │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │ Circuit Breaker (EmbeddingManager)                      │  │
│  │ - Failure threshold: 50% in 10 requests                │  │
│  │ - Open timeout: 30 seconds                              │  │
│  │ - Half-open probe: 3 requests                           │  │
│  └─────────────────────────────────────────────────────────┘  │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │ Retry Logic (Exponential Backoff)                      │  │
│  │ - Max retries: 3                                        │  │
│  │ - Base delay: 100ms                                     │  │
│  │ - Max delay: 5s                                         │  │
│  │ - Jitter: ±25%                                          │  │
│  └─────────────────────────────────────────────────────────┘  │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │ Timeout Management                                      │  │
│  │ - Model load: 60s                                       │  │
│  │ - Inference: 5s                                         │  │
│  │ - Health check: 3s                                      │  │
│  └─────────────────────────────────────────────────────────┘  │
└─────────────────────────┬──────────────────────────────────────┘
                          │
                          ▼
┌────────────────────────────────────────────────────────────────┐
│               ONNX Runtime + TensorRT                           │
│               (GPU Inference)                                   │
└────────────────────────────────────────────────────────────────┘
```

### Circuit Breaker State Machine

```
                    ┌───────────────┐
                    │    CLOSED     │
                    │ (Normal Ops)  │
                    └───────┬───────┘
                            │
              Failure rate  │  Success rate
              > 50%         │  normal
              (10 requests) │
                            │
                            ▼
                    ┌───────────────┐
              ┌─────│     OPEN      │
              │     │ (Fail Fast)   │
              │     └───────┬───────┘
              │             │
              │             │ After 30s timeout
              │             │
              │             ▼
              │     ┌───────────────┐
              │     │  HALF-OPEN    │
              │     │ (Testing)     │
              │     └───────┬───────┘
              │             │
              │   Success   │   Failure
              │   (3 probes)│   (1 probe)
              │             │
              └─────────────┘
```

---

## Security Architecture

### Defense in Depth Strategy

```
┌────────────────────────────────────────────────────────────────┐
│ Layer 1: Network Security                                       │
│ - Network Policies (deny-all default)                          │
│ - Pod Security Standards (restricted)                          │
│ - Service mesh (optional, future)                              │
└────────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌────────────────────────────────────────────────────────────────┐
│ Layer 2: Transport Security                                     │
│ - mTLS (mutual TLS)                                             │
│ - Certificate rotation (30 days)                               │
│ - TLS 1.3 only                                                  │
└────────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌────────────────────────────────────────────────────────────────┐
│ Layer 3: Authentication                                         │
│ - API Key (REST): X-API-Key header                             │
│ - JWT Token (gRPC): metadata authorization                     │
│ - Token expiration: 1 hour                                     │
└────────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌────────────────────────────────────────────────────────────────┐
│ Layer 4: Authorization                                          │
│ - Kubernetes RBAC (ClusterRole, Role)                          │
│ - Application RBAC (4 roles)                                   │
│   - admin: Full access                                         │
│   - developer: Read + write embeddings                         │
│   - viewer: Read-only                                          │
│   - operator: Health checks + metrics                          │
└────────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌────────────────────────────────────────────────────────────────┐
│ Layer 5: Application Security                                   │
│ - Input validation (request size <10MB)                        │
│ - Rate limiting (per-client, per-role)                         │
│ - Audit logging (all API calls)                                │
└────────────────────────────────────────────────────────────────┘
```

### mTLS Certificate Chain

```
                   ┌─────────────────┐
                   │   Root CA       │
                   │ (self-signed)   │
                   └────────┬────────┘
                            │
              ┌─────────────┴─────────────┐
              │                           │
      ┌───────▼────────┐         ┌───────▼────────┐
      │ Server Cert    │         │ Client Cert    │
      │ (akidb-rest)   │         │ (clients)      │
      └────────────────┘         └────────────────┘
```

---

## Day-by-Day Implementation Plan

### Day 1: Circuit Breakers & Rate Limiting

**Objective:** Implement resilience patterns to prevent cascading failures

**Tasks:**

1. **Add Circuit Breaker Crate** (`crates/akidb-service/src/circuit_breaker.rs`, ~250 lines)

```rust
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CircuitState {
    Closed,   // Normal operation
    Open,     // Failing fast
    HalfOpen, // Testing recovery
}

pub struct CircuitBreakerConfig {
    pub failure_threshold: f64,        // 0.5 = 50% failure rate
    pub success_threshold: usize,      // 3 successful probes to close
    pub timeout: Duration,             // 30s before trying half-open
    pub window_size: usize,            // 10 requests for failure calculation
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 0.5,
            success_threshold: 3,
            timeout: Duration::from_secs(30),
            window_size: 10,
        }
    }
}

pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: Arc<RwLock<CircuitState>>,
    failure_count: Arc<RwLock<usize>>,
    success_count: Arc<RwLock<usize>>,
    last_failure_time: Arc<RwLock<Option<Instant>>>,
    recent_results: Arc<RwLock<Vec<bool>>>, // true = success, false = failure
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_count: Arc::new(RwLock::new(0)),
            success_count: Arc::new(RwLock::new(0)),
            last_failure_time: Arc::new(RwLock::new(None)),
            recent_results: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn call<F, T, E>(&self, f: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: FnOnce() -> Result<T, E> + Send,
        T: Send,
        E: Send,
    {
        // Check current state
        let state = *self.state.read().unwrap();

        match state {
            CircuitState::Open => {
                // Check if timeout has elapsed
                if let Some(last_failure) = *self.last_failure_time.read().unwrap() {
                    if last_failure.elapsed() >= self.config.timeout {
                        // Transition to half-open
                        *self.state.write().unwrap() = CircuitState::HalfOpen;
                        *self.success_count.write().unwrap() = 0;
                        tracing::info!("Circuit breaker: OPEN -> HALF-OPEN");
                    } else {
                        // Still open, fail fast
                        return Err(CircuitBreakerError::Open);
                    }
                }
            }
            CircuitState::Closed | CircuitState::HalfOpen => {}
        }

        // Execute the function
        match f() {
            Ok(result) => {
                self.on_success();
                Ok(result)
            }
            Err(e) => {
                self.on_failure();
                Err(CircuitBreakerError::Error(e))
            }
        }
    }

    fn on_success(&self) {
        let mut recent = self.recent_results.write().unwrap();
        recent.push(true);
        if recent.len() > self.config.window_size {
            recent.remove(0);
        }

        let state = *self.state.read().unwrap();
        if state == CircuitState::HalfOpen {
            let mut success_count = self.success_count.write().unwrap();
            *success_count += 1;

            if *success_count >= self.config.success_threshold {
                // Transition to closed
                *self.state.write().unwrap() = CircuitState::Closed;
                *self.failure_count.write().unwrap() = 0;
                tracing::info!("Circuit breaker: HALF-OPEN -> CLOSED");
            }
        }
    }

    fn on_failure(&self) {
        let mut recent = self.recent_results.write().unwrap();
        recent.push(false);
        if recent.len() > self.config.window_size {
            recent.remove(0);
        }

        // Calculate failure rate
        if recent.len() >= self.config.window_size {
            let failures = recent.iter().filter(|&&r| !r).count();
            let failure_rate = failures as f64 / recent.len() as f64;

            if failure_rate >= self.config.failure_threshold {
                // Transition to open
                let current_state = *self.state.read().unwrap();
                if current_state != CircuitState::Open {
                    *self.state.write().unwrap() = CircuitState::Open;
                    *self.last_failure_time.write().unwrap() = Some(Instant::now());
                    tracing::warn!(
                        "Circuit breaker: {:?} -> OPEN (failure rate: {:.2}%)",
                        current_state,
                        failure_rate * 100.0
                    );
                }
            }
        }
    }

    pub fn state(&self) -> CircuitState {
        *self.state.read().unwrap()
    }
}

#[derive(Debug)]
pub enum CircuitBreakerError<E> {
    Open,
    Error(E),
}

impl<E: std::fmt::Display> std::fmt::Display for CircuitBreakerError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "Circuit breaker is open"),
            Self::Error(e) => write!(f, "Operation failed: {}", e),
        }
    }
}

impl<E: std::error::Error> std::error::Error for CircuitBreakerError<E> {}
```

2. **Add Rate Limiter** (`crates/akidb-service/src/rate_limiter.rs`, ~200 lines)

```rust
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

pub struct RateLimiterConfig {
    pub requests_per_second: usize,
    pub burst_size: usize,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            requests_per_second: 100,
            burst_size: 20,
        }
    }
}

pub struct TokenBucket {
    capacity: usize,
    tokens: f64,
    refill_rate: f64, // tokens per second
    last_refill: Instant,
}

impl TokenBucket {
    pub fn new(requests_per_second: usize, burst_size: usize) -> Self {
        Self {
            capacity: burst_size,
            tokens: burst_size as f64,
            refill_rate: requests_per_second as f64,
            last_refill: Instant::now(),
        }
    }

    pub fn try_acquire(&mut self, tokens: usize) -> bool {
        self.refill();

        if self.tokens >= tokens as f64 {
            self.tokens -= tokens as f64;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        let new_tokens = elapsed * self.refill_rate;

        self.tokens = (self.tokens + new_tokens).min(self.capacity as f64);
        self.last_refill = now;
    }
}

pub struct RateLimiter {
    config: RateLimiterConfig,
    global_bucket: Arc<RwLock<TokenBucket>>,
    client_buckets: Arc<RwLock<HashMap<String, TokenBucket>>>,
}

impl RateLimiter {
    pub fn new(config: RateLimiterConfig) -> Self {
        Self {
            global_bucket: Arc::new(RwLock::new(TokenBucket::new(
                config.requests_per_second * 10, // 10x for global
                config.burst_size * 10,
            ))),
            client_buckets: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    pub fn check_rate_limit(&self, client_id: &str) -> Result<(), RateLimitError> {
        // Check global limit first
        {
            let mut global = self.global_bucket.write().unwrap();
            if !global.try_acquire(1) {
                return Err(RateLimitError::GlobalLimitExceeded);
            }
        }

        // Check per-client limit
        {
            let mut buckets = self.client_buckets.write().unwrap();
            let bucket = buckets
                .entry(client_id.to_string())
                .or_insert_with(|| {
                    TokenBucket::new(
                        self.config.requests_per_second,
                        self.config.burst_size,
                    )
                });

            if !bucket.try_acquire(1) {
                return Err(RateLimitError::ClientLimitExceeded);
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub enum RateLimitError {
    GlobalLimitExceeded,
    ClientLimitExceeded,
}

impl std::fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GlobalLimitExceeded => write!(f, "Global rate limit exceeded"),
            Self::ClientLimitExceeded => write!(f, "Client rate limit exceeded"),
        }
    }
}

impl std::error::Error for RateLimitError {}
```

3. **Integrate into REST Server** (`crates/akidb-rest/src/middleware/resilience.rs`)

```rust
use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use akidb_service::rate_limiter::RateLimiter;
use std::sync::Arc;

pub async fn rate_limit_middleware(
    State(limiter): State<Arc<RateLimiter>>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract client ID from header or IP
    let client_id = request
        .headers()
        .get("x-client-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("default");

    // Check rate limit
    match limiter.check_rate_limit(client_id) {
        Ok(_) => Ok(next.run(request).await),
        Err(e) => {
            tracing::warn!("Rate limit exceeded for client {}: {}", client_id, e);
            Err(StatusCode::TOO_MANY_REQUESTS)
        }
    }
}
```

4. **Testing**

```bash
# Unit tests for circuit breaker
cargo test -p akidb-service circuit_breaker

# Unit tests for rate limiter
cargo test -p akidb-service rate_limiter

# Integration test: Trigger circuit breaker
curl -X POST http://localhost:8080/api/v1/embed \
  -H "Content-Type: application/json" \
  -d '{"inputs": ["test"]}' \
  --max-time 1 \
  --retry 20 \
  --retry-delay 0

# Integration test: Trigger rate limiter
for i in {1..150}; do
  curl -X POST http://localhost:8080/api/v1/embed \
    -H "Content-Type: application/json" \
    -H "X-Client-ID: test-client" \
    -d '{"inputs": ["test '$i'"]}'
done
```

**Success Criteria:**
- [ ] Circuit breaker opens after 50% failure rate
- [ ] Circuit breaker transitions to half-open after 30s
- [ ] Circuit breaker closes after 3 successful probes
- [ ] Rate limiter returns 429 after 100 req/sec per client
- [ ] Global rate limiter enforces 1000 req/sec
- [ ] All unit tests pass

**Completion:** `automatosx/tmp/jetson-thor-week6-day1-completion.md`

---

### Day 2: Backpressure & Request Queue

**Objective:** Implement backpressure handling with request queuing and load shedding

**Tasks:**

1. **Add Request Queue** (`crates/akidb-service/src/request_queue.rs`, ~300 lines)

```rust
use tokio::sync::{mpsc, oneshot};
use std::sync::Arc;
use std::time::Duration;

pub struct RequestQueueConfig {
    pub capacity: usize,
    pub load_shedding_threshold: f64, // 0.95 = 95%
    pub timeout: Duration,
}

impl Default for RequestQueueConfig {
    fn default() -> Self {
        Self {
            capacity: 1000,
            load_shedding_threshold: 0.95,
            timeout: Duration::from_secs(30),
        }
    }
}

pub struct QueuedRequest<T, R> {
    pub request: T,
    pub priority: RequestPriority,
    pub response_tx: oneshot::Sender<Result<R, QueueError>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RequestPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Admin = 3,
}

pub struct RequestQueue<T, R> {
    config: RequestQueueConfig,
    queue_tx: mpsc::Sender<QueuedRequest<T, R>>,
    queue_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<QueuedRequest<T, R>>>>,
    current_size: Arc<std::sync::atomic::AtomicUsize>,
}

impl<T: Send + 'static, R: Send + 'static> RequestQueue<T, R> {
    pub fn new(config: RequestQueueConfig) -> Self {
        let (queue_tx, queue_rx) = mpsc::channel(config.capacity);

        Self {
            config,
            queue_tx,
            queue_rx: Arc::new(tokio::sync::Mutex::new(queue_rx)),
            current_size: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    pub async fn enqueue(
        &self,
        request: T,
        priority: RequestPriority,
    ) -> Result<oneshot::Receiver<Result<R, QueueError>>, QueueError> {
        // Check load shedding threshold
        let current = self.current_size.load(std::sync::atomic::Ordering::Relaxed);
        let threshold = (self.config.capacity as f64 * self.config.load_shedding_threshold) as usize;

        if current >= threshold && priority < RequestPriority::High {
            return Err(QueueError::LoadShedding);
        }

        let (response_tx, response_rx) = oneshot::channel();

        let queued = QueuedRequest {
            request,
            priority,
            response_tx,
        };

        // Try to send with timeout
        match tokio::time::timeout(
            Duration::from_millis(100),
            self.queue_tx.send(queued),
        )
        .await
        {
            Ok(Ok(_)) => {
                self.current_size.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Ok(response_rx)
            }
            Ok(Err(_)) => Err(QueueError::QueueFull),
            Err(_) => Err(QueueError::Timeout),
        }
    }

    pub async fn dequeue(&self) -> Option<QueuedRequest<T, R>> {
        let mut rx = self.queue_rx.lock().await;
        let request = rx.recv().await;
        if request.is_some() {
            self.current_size.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        }
        request
    }

    pub fn current_size(&self) -> usize {
        self.current_size.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn capacity(&self) -> usize {
        self.config.capacity
    }
}

#[derive(Debug)]
pub enum QueueError {
    QueueFull,
    LoadShedding,
    Timeout,
}

impl std::fmt::Display for QueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QueueFull => write!(f, "Request queue is full"),
            Self::LoadShedding => write!(f, "Load shedding active"),
            Self::Timeout => write!(f, "Queue enqueue timeout"),
        }
    }
}

impl std::error::Error for QueueError {}
```

2. **Add Queue Worker** (`crates/akidb-rest/src/queue_worker.rs`)

```rust
use akidb_service::request_queue::{RequestQueue, QueuedRequest};
use akidb_service::embedding_manager::EmbeddingManager;
use std::sync::Arc;

pub async fn start_queue_worker(
    queue: Arc<RequestQueue<EmbedRequest, EmbedResponse>>,
    manager: Arc<EmbeddingManager>,
) {
    tokio::spawn(async move {
        loop {
            if let Some(queued) = queue.dequeue().await {
                let result = process_request(queued.request, &manager).await;
                let _ = queued.response_tx.send(result);
            }
        }
    });
}

async fn process_request(
    request: EmbedRequest,
    manager: &EmbeddingManager,
) -> Result<EmbedResponse, QueueError> {
    // Process embedding request
    // ... (implementation details)
}
```

3. **Add Metrics**

```rust
// Add to metrics.rs
lazy_static! {
    pub static ref QUEUE_SIZE: IntGauge = IntGauge::new(
        "akidb_queue_size", "Current request queue size"
    ).unwrap();

    pub static ref QUEUE_CAPACITY: IntGauge = IntGauge::new(
        "akidb_queue_capacity", "Request queue capacity"
    ).unwrap();

    pub static ref LOAD_SHED_TOTAL: Counter = Counter::new(
        "akidb_load_shed_total", "Total requests shed due to load"
    ).unwrap();
}
```

4. **Testing**

```bash
# Stress test: Fill queue to capacity
for i in {1..1200}; do
  curl -X POST http://localhost:8080/api/v1/embed \
    -H "Content-Type: application/json" \
    -d '{"inputs": ["stress test '$i'"]}'&
done

# Check metrics
curl http://localhost:8080/metrics | grep akidb_queue_size
curl http://localhost:8080/metrics | grep akidb_load_shed_total
```

**Success Criteria:**
- [ ] Queue handles 1000 concurrent requests
- [ ] Load shedding triggers at 95% capacity
- [ ] High-priority requests bypass load shedding
- [ ] Queue metrics accurate
- [ ] Zero dropped admin requests
- [ ] Graceful degradation under extreme load

**Completion:** `automatosx/tmp/jetson-thor-week6-day2-completion.md`

---

### Day 3: Mutual TLS (mTLS) & Network Security

**Objective:** Implement mTLS for secure internal communication

**Tasks:**

1. **Generate Certificates**

```bash
# Create certificate generation script
cat > scripts/generate-mtls-certs.sh <<'EOF'
#!/bin/bash
set -e

CERTS_DIR="deploy/certs"
mkdir -p $CERTS_DIR

# Generate Root CA
openssl req -x509 -new -nodes -sha256 -days 365 \
  -newkey rsa:2048 \
  -keyout $CERTS_DIR/ca-key.pem \
  -out $CERTS_DIR/ca-cert.pem \
  -subj "/C=US/ST=CA/O=AkiDB/CN=AkiDB Root CA"

# Generate Server Certificate
openssl req -new -nodes -sha256 \
  -newkey rsa:2048 \
  -keyout $CERTS_DIR/server-key.pem \
  -out $CERTS_DIR/server-csr.pem \
  -subj "/C=US/ST=CA/O=AkiDB/CN=akidb-rest"

openssl x509 -req -sha256 -days 365 \
  -in $CERTS_DIR/server-csr.pem \
  -CA $CERTS_DIR/ca-cert.pem \
  -CAkey $CERTS_DIR/ca-key.pem \
  -CAcreateserial \
  -out $CERTS_DIR/server-cert.pem \
  -extfile <(echo "subjectAltName=DNS:akidb-rest,DNS:akidb-rest.akidb.svc.cluster.local")

# Generate Client Certificate
openssl req -new -nodes -sha256 \
  -newkey rsa:2048 \
  -keyout $CERTS_DIR/client-key.pem \
  -out $CERTS_DIR/client-csr.pem \
  -subj "/C=US/ST=CA/O=AkiDB/CN=akidb-client"

openssl x509 -req -sha256 -days 365 \
  -in $CERTS_DIR/client-csr.pem \
  -CA $CERTS_DIR/ca-cert.pem \
  -CAkey $CERTS_DIR/ca-key.pem \
  -CAcreateserial \
  -out $CERTS_DIR/client-cert.pem

echo "✅ mTLS certificates generated in $CERTS_DIR"
EOF

chmod +x scripts/generate-mtls-certs.sh
bash scripts/generate-mtls-certs.sh
```

2. **Create Kubernetes Secret**

```bash
# Create TLS secret
kubectl create secret tls akidb-tls \
  --cert=deploy/certs/server-cert.pem \
  --key=deploy/certs/server-key.pem \
  --namespace=akidb

# Create CA secret
kubectl create secret generic akidb-ca \
  --from-file=ca.crt=deploy/certs/ca-cert.pem \
  --namespace=akidb

# Create client secret
kubectl create secret tls akidb-client-tls \
  --cert=deploy/certs/client-cert.pem \
  --key=deploy/certs/client-key.pem \
  --namespace=akidb
```

3. **Configure REST Server for mTLS**

```rust
// crates/akidb-rest/src/tls.rs
use axum_server::tls_rustls::RustlsConfig;
use std::path::Path;

pub async fn create_tls_config(
    cert_path: &Path,
    key_path: &Path,
    ca_cert_path: &Path,
) -> Result<RustlsConfig, Box<dyn std::error::Error>> {
    let config = RustlsConfig::from_pem_file(cert_path, key_path)
        .await?
        .with_client_auth_required(ca_cert_path)
        .await?;

    Ok(config)
}
```

4. **Update Deployment**

```yaml
# deploy/helm/akidb-jetson/templates/deployment-rest.yaml
spec:
  containers:
  - name: akidb-rest
    volumeMounts:
    - name: tls-certs
      mountPath: /etc/tls
      readOnly: true
    env:
    - name: AKIDB_TLS_CERT
      value: /etc/tls/tls.crt
    - name: AKIDB_TLS_KEY
      value: /etc/tls/tls.key
    - name: AKIDB_TLS_CA
      value: /etc/tls/ca.crt
  volumes:
  - name: tls-certs
    projected:
      sources:
      - secret:
          name: akidb-tls
      - secret:
          name: akidb-ca
```

5. **Network Policies**

```yaml
# deploy/k8s/network-policy.yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: akidb-network-policy
  namespace: akidb
spec:
  podSelector:
    matchLabels:
      app: akidb-rest
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          name: akidb
    ports:
    - protocol: TCP
      port: 8080
  egress:
  - to:
    - namespaceSelector:
        matchLabels:
          name: akidb
    ports:
    - protocol: TCP
      port: 9090
  - to:
    - namespaceSelector:
        matchLabels:
          name: kube-system
    ports:
    - protocol: TCP
      port: 53 # DNS
```

6. **Testing**

```bash
# Test mTLS connection
curl --cacert deploy/certs/ca-cert.pem \
     --cert deploy/certs/client-cert.pem \
     --key deploy/certs/client-key.pem \
     https://localhost:8080/health

# Test without client cert (should fail)
curl --cacert deploy/certs/ca-cert.pem \
     https://localhost:8080/health
```

**Success Criteria:**
- [ ] mTLS certificates generated
- [ ] Server enforces client certificate validation
- [ ] Network policies deny unauthorized traffic
- [ ] TLS 1.3 enforced
- [ ] Certificate rotation documented
- [ ] Zero plaintext communication

**Completion:** `automatosx/tmp/jetson-thor-week6-day3-completion.md`

---

### Day 4: RBAC & API Authentication

**Objective:** Implement role-based access control and API authentication

**Tasks:**

1. **Define RBAC Roles** (`crates/akidb-core/src/rbac.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Viewer = 0,   // Read-only (health, metrics)
    Operator = 1, // Viewer + operational tasks
    Developer = 2,// Operator + embedding generation
    Admin = 3,    // Full access
}

pub struct Permission {
    pub resource: Resource,
    pub action: Action,
}

pub enum Resource {
    Health,
    Metrics,
    Embeddings,
    Models,
    Configuration,
}

pub enum Action {
    Read,
    Write,
    Execute,
}

impl Role {
    pub fn can_perform(&self, permission: &Permission) -> bool {
        match (self, &permission.resource, &permission.action) {
            // Viewer: Read health and metrics
            (Role::Viewer, Resource::Health, Action::Read) => true,
            (Role::Viewer, Resource::Metrics, Action::Read) => true,

            // Operator: Viewer + operational tasks
            (Role::Operator, Resource::Health, _) => true,
            (Role::Operator, Resource::Metrics, _) => true,
            (Role::Operator, Resource::Models, Action::Read) => true,

            // Developer: Operator + embeddings
            (Role::Developer, _, Action::Read) => true,
            (Role::Developer, Resource::Embeddings, _) => true,

            // Admin: Full access
            (Role::Admin, _, _) => true,

            _ => false,
        }
    }
}
```

2. **API Key Authentication** (`crates/akidb-service/src/auth.rs`)

```rust
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::sync::Arc;

pub struct ApiKeyStore {
    keys: Arc<std::sync::RwLock<HashMap<String, ApiKey>>>,
}

pub struct ApiKey {
    pub key_hash: String,
    pub client_id: String,
    pub role: Role,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl ApiKeyStore {
    pub fn new() -> Self {
        Self {
            keys: Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }

    pub fn add_key(&self, key: &str, client_id: &str, role: Role) {
        let key_hash = hash_api_key(key);
        let api_key = ApiKey {
            key_hash: key_hash.clone(),
            client_id: client_id.to_string(),
            role,
            created_at: chrono::Utc::now(),
            expires_at: None,
        };

        self.keys.write().unwrap().insert(key_hash, api_key);
    }

    pub fn validate(&self, key: &str) -> Option<ApiKey> {
        let key_hash = hash_api_key(key);
        self.keys.read().unwrap().get(&key_hash).cloned()
    }
}

fn hash_api_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    format!("{:x}", hasher.finalize())
}
```

3. **Auth Middleware** (`crates/akidb-rest/src/middleware/auth.rs`)

```rust
use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use akidb_service::auth::ApiKeyStore;

pub async fn api_key_middleware(
    State(store): State<Arc<ApiKeyStore>>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract API key from header
    let api_key = request
        .headers()
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate API key
    let validated = store
        .validate(api_key)
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Add client info to request extensions
    request.extensions_mut().insert(validated);

    Ok(next.run(request).await)
}
```

4. **Kubernetes RBAC**

```yaml
# deploy/k8s/rbac.yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: akidb-viewer
rules:
- apiGroups: [""]
  resources: ["pods", "services"]
  verbs: ["get", "list"]

---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: akidb-admin
rules:
- apiGroups: [""]
  resources: ["*"]
  verbs: ["*"]
- apiGroups: ["apps"]
  resources: ["*"]
  verbs: ["*"]

---
apiVersion: v1
kind: ServiceAccount
metadata:
  name: akidb-operator
  namespace: akidb

---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRoleBinding
metadata:
  name: akidb-operator-binding
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: ClusterRole
  name: akidb-viewer
subjects:
- kind: ServiceAccount
  name: akidb-operator
  namespace: akidb
```

5. **Testing**

```bash
# Generate API keys
ADMIN_KEY=$(openssl rand -hex 32)
DEVELOPER_KEY=$(openssl rand -hex 32)
VIEWER_KEY=$(openssl rand -hex 32)

echo "Admin Key: $ADMIN_KEY"
echo "Developer Key: $DEVELOPER_KEY"
echo "Viewer Key: $VIEWER_KEY"

# Test admin access
curl -X POST http://localhost:8080/api/v1/embed \
  -H "X-API-Key: $ADMIN_KEY" \
  -H "Content-Type: application/json" \
  -d '{"inputs": ["test"]}'

# Test developer access (should succeed)
curl -X POST http://localhost:8080/api/v1/embed \
  -H "X-API-Key: $DEVELOPER_KEY" \
  -H "Content-Type: application/json" \
  -d '{"inputs": ["test"]}'

# Test viewer access (should fail)
curl -X POST http://localhost:8080/api/v1/embed \
  -H "X-API-Key: $VIEWER_KEY" \
  -H "Content-Type: application/json" \
  -d '{"inputs": ["test"]}'

# Test unauthorized (no API key)
curl -X POST http://localhost:8080/api/v1/embed \
  -H "Content-Type: application/json" \
  -d '{"inputs": ["test"]}'
```

**Success Criteria:**
- [ ] 4 roles implemented (viewer, operator, developer, admin)
- [ ] API key authentication enforced
- [ ] RBAC permissions enforced
- [ ] Kubernetes RBAC configured
- [ ] Unauthorized requests return 401
- [ ] Forbidden requests return 403

**Completion:** `automatosx/tmp/jetson-thor-week6-day4-completion.md`

---

### Day 5: Chaos Engineering & SLI/SLO Tracking

**Objective:** Validate system resilience with chaos tests and implement SLI/SLO tracking

**Tasks:**

1. **Install Chaos Mesh**

```bash
# Install Chaos Mesh
kubectl apply -f https://mirrors.chaos-mesh.org/v2.7.0/crd.yaml
kubectl apply -f https://mirrors.chaos-mesh.org/v2.7.0/chaos-mesh.yaml

# Verify installation
kubectl get pods -n chaos-mesh
```

2. **Chaos Test Scenarios**

**Scenario 1: Pod Kill**
```yaml
# deploy/chaos/pod-kill.yaml
apiVersion: chaos-mesh.org/v1alpha1
kind: PodChaos
metadata:
  name: pod-kill-test
  namespace: akidb
spec:
  action: pod-kill
  mode: one
  selector:
    namespaces:
      - akidb
    labelSelectors:
      app: akidb-rest
  scheduler:
    cron: '@every 5m'
```

**Scenario 2: Network Latency**
```yaml
# deploy/chaos/network-latency.yaml
apiVersion: chaos-mesh.org/v1alpha1
kind: NetworkChaos
metadata:
  name: network-latency-test
  namespace: akidb
spec:
  action: delay
  mode: one
  selector:
    namespaces:
      - akidb
    labelSelectors:
      app: akidb-rest
  delay:
    latency: "100ms"
    correlation: "25"
    jitter: "50ms"
  duration: "2m"
```

**Scenario 3: GPU Memory Stress**
```yaml
# deploy/chaos/stress-gpu.yaml
apiVersion: chaos-mesh.org/v1alpha1
kind: StressChaos
metadata:
  name: gpu-stress-test
  namespace: akidb
spec:
  mode: one
  selector:
    namespaces:
      - akidb
    labelSelectors:
      app: akidb-rest
  stressors:
    memory:
      workers: 4
      size: "2GB"
  duration: "2m"
```

3. **Chaos Test Runner**

```bash
# Create chaos test script
cat > scripts/run-chaos-tests.sh <<'EOF'
#!/bin/bash
set -e

echo "Running Chaos Engineering Tests"
echo "================================"

# Scenario 1: Pod Kill
echo "Test 1: Pod Kill"
kubectl apply -f deploy/chaos/pod-kill.yaml
sleep 60
kubectl delete podchaos pod-kill-test -n akidb

# Scenario 2: Network Latency
echo "Test 2: Network Latency"
kubectl apply -f deploy/chaos/network-latency.yaml
sleep 120
kubectl delete networkchaos network-latency-test -n akidb

# Scenario 3: GPU Stress
echo "Test 3: GPU Memory Stress"
kubectl apply -f deploy/chaos/stress-gpu.yaml
sleep 120
kubectl delete stresschaos gpu-stress-test -n akidb

echo "✅ All chaos tests complete"
EOF

chmod +x scripts/run-chaos-tests.sh
```

4. **SLI/SLO Definitions**

```yaml
# deploy/k8s/slo-alerts.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: prometheus-slo-rules
  namespace: akidb
data:
  slo-rules.yml: |
    groups:
    - name: slo-alerts
      interval: 30s
      rules:
      # SLI: Availability (target: 99.9%)
      - alert: AvailabilitySLOViolation
        expr: |
          (
            sum(rate(akidb_embed_requests_total{status="success"}[5m]))
            /
            sum(rate(akidb_embed_requests_total[5m]))
          ) < 0.999
        for: 5m
        labels:
          severity: critical
          slo: availability
        annotations:
          summary: "Availability SLO violation"
          description: "Success rate is {{ $value | humanizePercentage }}, below 99.9% target"

      # SLI: Latency (target: P95 < 30ms)
      - alert: LatencySLOViolation
        expr: |
          histogram_quantile(0.95, akidb_embed_latency_seconds) > 0.030
        for: 5m
        labels:
          severity: warning
          slo: latency
        annotations:
          summary: "Latency SLO violation"
          description: "P95 latency is {{ $value }}s, above 30ms target"

      # SLI: Error Rate (target: < 0.1%)
      - alert: ErrorRateSLOViolation
        expr: |
          (
            sum(rate(akidb_embed_requests_total{status="error"}[5m]))
            /
            sum(rate(akidb_embed_requests_total[5m]))
          ) > 0.001
        for: 2m
        labels:
          severity: critical
          slo: error_rate
        annotations:
          summary: "Error rate SLO violation"
          description: "Error rate is {{ $value | humanizePercentage }}, above 0.1% target"
```

5. **Run Chaos Tests with Metrics**

```bash
# Start chaos tests
bash scripts/run-chaos-tests.sh &

# Generate load during chaos
while true; do
  curl -X POST http://localhost:8080/api/v1/embed \
    -H "X-API-Key: $ADMIN_KEY" \
    -H "Content-Type: application/json" \
    -d '{"inputs": ["chaos test"]}'
  sleep 0.1
done &

LOAD_PID=$!

# Wait for tests to complete
wait

# Stop load generation
kill $LOAD_PID

# Check SLO compliance
curl http://localhost:9090/api/v1/query?query='sum(rate(akidb_embed_requests_total{status="success"}[5m])) / sum(rate(akidb_embed_requests_total[5m]))'
```

6. **Generate Chaos Report**

```bash
# Create chaos report script
cat > scripts/generate-chaos-report.sh <<'EOF'
#!/bin/bash

REPORT_FILE="automatosx/tmp/jetson-thor-week6-chaos-report.md"

cat > $REPORT_FILE <<'REPORT'
# Jetson Thor Week 6: Chaos Engineering Report

**Date:** $(date +"%Y-%m-%d")
**Duration:** 6 chaos scenarios, 10 minutes each

## Test Results

### Scenario 1: Pod Kill
- **Objective:** Validate zero-downtime pod restarts
- **Result:** [PASS/FAIL]
- **Availability during test:** [XX.XX%]
- **Max latency spike:** [XXms]
- **Recovery time:** [Xs]

### Scenario 2: Network Latency (100ms ±50ms)
- **Objective:** Validate timeout handling
- **Result:** [PASS/FAIL]
- **P95 latency during test:** [XXms]
- **Timeout errors:** [X]
- **Circuit breaker triggered:** [YES/NO]

### Scenario 3: GPU Memory Stress (2GB)
- **Objective:** Validate OOM handling
- **Result:** [PASS/FAIL]
- **GPU memory peak:** [X.XGB]
- **OOM kills:** [X]
- **Graceful degradation:** [YES/NO]

### Scenario 4: Network Partition
- **Objective:** Validate split-brain prevention
- **Result:** [PASS/FAIL]
- **Requests failed:** [X]
- **Recovery time:** [Xs]

### Scenario 5: Disk Pressure
- **Objective:** Validate disk full handling
- **Result:** [PASS/FAIL]
- **TensorRT cache evictions:** [X]
- **Errors logged:** [X]

### Scenario 6: DNS Failure
- **Objective:** Validate service discovery resilience
- **Result:** [PASS/FAIL]
- **Failed lookups:** [X]
- **Fallback used:** [YES/NO]

## SLO Compliance

| SLI | Target | Actual | Status |
|-----|--------|--------|--------|
| Availability | 99.9% | [XX.XX%] | [✅/❌] |
| P95 Latency | <30ms | [XXms] | [✅/❌] |
| Error Rate | <0.1% | [X.XX%] | [✅/❌] |

## Recommendations

1. [Recommendation based on test results]
2. [Recommendation based on test results]
3. [Recommendation based on test results]

---

**Next Steps:** Week 7 - Multi-Region Deployment & CI/CD
REPORT

echo "✅ Chaos report generated: $REPORT_FILE"
EOF

chmod +x scripts/generate-chaos-report.sh
bash scripts/generate-chaos-report.sh
```

**Success Criteria:**
- [ ] All 6 chaos scenarios executed
- [ ] Availability maintained >99% during chaos
- [ ] P95 latency <50ms during chaos
- [ ] Zero pod crashes
- [ ] Circuit breakers triggered appropriately
- [ ] SLO alerts configured and tested

**Completion:** `automatosx/tmp/jetson-thor-week6-completion-report.md`

---

## Resilience Patterns

### Circuit Breaker Pattern

**States:**
1. **Closed**: Normal operation, requests pass through
2. **Open**: Failure threshold exceeded, fail fast for timeout period
3. **Half-Open**: Testing recovery with limited probes

**Configuration:**
- Failure threshold: 50% in rolling window of 10 requests
- Open timeout: 30 seconds
- Half-open probes: 3 successful requests to close

### Rate Limiting Pattern

**Algorithm:** Token bucket with per-client and global limits

**Configuration:**
- Per-client limit: 100 req/sec, burst 20
- Global limit: 1000 req/sec, burst 200
- Response: HTTP 429 Too Many Requests

### Retry Pattern

**Strategy:** Exponential backoff with jitter

**Configuration:**
- Max retries: 3
- Base delay: 100ms
- Max delay: 5s
- Jitter: ±25%

**Retry on:**
- Network errors
- Timeout errors
- HTTP 5xx errors

**Do not retry on:**
- HTTP 4xx errors (except 429)
- Circuit breaker open

### Timeout Pattern

**Configuration:**
- Model load: 60s
- Inference: 5s
- Health check: 3s
- Database query: 1s

---

## Security Implementation

### mTLS Best Practices

1. **Certificate Rotation:**
   - Automated rotation every 30 days
   - Use cert-manager for Kubernetes
   - Zero-downtime rotation with dual certificate support

2. **Certificate Validation:**
   - Enforce TLS 1.3 only
   - Verify certificate chain
   - Check certificate revocation (OCSP)

3. **Private Key Protection:**
   - Store in Kubernetes secrets
   - Never log or expose keys
   - Use hardware security modules (HSM) for production

### API Authentication Best Practices

1. **API Key Management:**
   - Use SHA-256 hashed keys
   - Never store plaintext keys
   - Rotate keys every 90 days
   - Implement key expiration

2. **JWT Tokens:**
   - Use RS256 algorithm
   - Short expiration (1 hour)
   - Implement refresh tokens
   - Validate issuer and audience

3. **Rate Limiting:**
   - Different limits per role
   - Track by client ID, not IP
   - Implement sliding window

---

## Chaos Engineering

### Testing Strategy

**Blast Radius Control:**
- Start with non-production environments
- Use percentage-based chaos (10% of pods)
- Implement automatic rollback on SLO violations
- Schedule chaos during low-traffic periods

### Common Failure Scenarios

1. **Infrastructure Failures:**
   - Pod/node crashes
   - Disk full
   - Network partitions
   - DNS failures

2. **Resource Exhaustion:**
   - GPU OOM
   - CPU saturation
   - Memory leaks
   - Connection pool exhaustion

3. **Latency Issues:**
   - Network latency
   - Slow dependencies
   - Database slow queries
   - Model load delays

4. **Application Bugs:**
   - Null pointer exceptions
   - Race conditions
   - Deadlocks
   - Memory corruption

---

## Risk Management

### Production Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| **Circuit breaker false positives** | Medium | Medium | Tune failure threshold, add anomaly detection |
| **Rate limiting too aggressive** | High | Low | Monitor 429 rate, adjust limits based on traffic |
| **mTLS cert expiration** | Critical | Low | Automated rotation, 30-day expiry warning |
| **API key leakage** | Critical | Low | Rotate immediately, audit logs, detection system |
| **Chaos tests impact production** | High | Low | Blast radius control, SLO guards, manual approval |
| **Backpressure queue OOM** | High | Medium | Set memory limits, load shedding at 95% |

### Security Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| **Man-in-the-middle attack** | Critical | Low | Enforce mTLS, certificate pinning |
| **API key brute force** | High | Medium | Rate limiting, exponential backoff, CAPTCHA |
| **Privilege escalation** | Critical | Low | RBAC least privilege, audit all role changes |
| **DoS attack** | High | Medium | Rate limiting, backpressure, DDoS protection |
| **Container escape** | Critical | Very Low | Pod Security Standards, SELinux/AppArmor |

---

## Success Criteria

### Week 6 Completion Criteria

| Criterion | Target | Measurement | Priority |
|-----------|--------|-------------|----------|
| **Circuit Breakers** | 3-state FSM | Unit tests + integration tests | P0 |
| **Rate Limiting** | 100/1000 req/sec | Load test verification | P0 |
| **Backpressure** | Queue 1000, shed at 95% | Stress test | P0 |
| **mTLS** | All internal traffic | Certificate validation | P0 |
| **RBAC** | 4 roles | Permission tests | P0 |
| **API Authentication** | API key + JWT | Auth tests | P0 |
| **Chaos Tests** | 6 scenarios | All pass | P0 |
| **SLI/SLO** | 99.9% availability | Prometheus metrics | P0 |
| **Availability (chaos)** | >99% | During all chaos tests | P1 |
| **P95 Latency (chaos)** | <50ms | During all chaos tests | P1 |
| **Security Audit** | Zero CVEs | Trivy/Snyk scan | P1 |
| **Runbooks** | 10+ scenarios | Documentation | P2 |

**Overall Success:** All P0 criteria + 80% of P1 criteria + 50% of P2 criteria

---

## Appendix: Code Examples

### Example 1: Complete Circuit Breaker Usage

```rust
use akidb_service::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let circuit_breaker = CircuitBreaker::new(CircuitBreakerConfig::default());

    // Wrap risky operation
    let result = circuit_breaker
        .call(|| {
            // Potentially failing operation
            generate_embedding("test input")
        })
        .await;

    match result {
        Ok(embedding) => println!("Success: {:?}", embedding),
        Err(CircuitBreakerError::Open) => {
            println!("Circuit breaker is open, failing fast");
        }
        Err(CircuitBreakerError::Error(e)) => {
            println!("Operation failed: {}", e);
        }
    }

    Ok(())
}
```

### Example 2: Rate Limiter with RBAC

```rust
use akidb_service::rate_limiter::{RateLimiter, RateLimiterConfig};
use akidb_core::rbac::Role;

let config = match role {
    Role::Admin => RateLimiterConfig {
        requests_per_second: 1000, // 10x for admin
        burst_size: 200,
    },
    Role::Developer => RateLimiterConfig {
        requests_per_second: 100,
        burst_size: 20,
    },
    Role::Viewer => RateLimiterConfig {
        requests_per_second: 10,
        burst_size: 2,
    },
    _ => RateLimiterConfig::default(),
};

let limiter = RateLimiter::new(config);

match limiter.check_rate_limit(&client_id) {
    Ok(_) => {
        // Process request
    }
    Err(RateLimitError::ClientLimitExceeded) => {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    Err(RateLimitError::GlobalLimitExceeded) => {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
}
```

---

**End of Week 6 PRD**

**Next Steps:** Week 7 - Multi-Region Deployment & CI/CD Pipeline
