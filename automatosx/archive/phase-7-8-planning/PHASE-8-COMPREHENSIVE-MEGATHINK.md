# Phase 8: Production Readiness & Authentication - COMPREHENSIVE MEGATHINK

**Date:** 2025-11-08
**Status:** PLANNING
**Dependencies:** Phase 7 Complete ✅
**Target:** v2.0.0-rc2 → v2.0.0-GA

---

## Executive Summary

Phase 8 transforms AkiDB 2.0 from "Phase 7 production-hardened" to "GA production-ready" by implementing critical missing features for enterprise deployment: **authentication, authorization, Kubernetes deployment, and operational polish**.

**Strategic Decision Point:**
- **Phase 7 completed:** Circuit breaker, DLQ, observability, admin APIs
- **Current state:** Production-hardened infrastructure but missing auth & deployment features
- **Gap to GA:** Authentication, TLS, Kubernetes, flaky test fixes, operational polish

**Phase 8 Focus:** Bridge the gap from RC1 (infrastructure) to GA (enterprise-ready).

**Timeline:** 6 weeks (30 working days)
**Team Size:** 1 developer (AI-assisted)
**Risk Level:** Medium (security-critical features)

---

## Table of Contents

1. [Current State Analysis](#current-state-analysis)
2. [Phase 8 Objectives](#phase-8-objectives)
3. [Strategic Options & Decision](#strategic-options--decision)
4. [Detailed Requirements](#detailed-requirements)
5. [Week-by-Week Action Plan](#week-by-week-action-plan)
6. [Technical Architecture](#technical-architecture)
7. [Risk Assessment](#risk-assessment)
8. [Success Metrics](#success-metrics)
9. [Testing Strategy](#testing-strategy)
10. [Deployment Strategy](#deployment-strategy)

---

## Current State Analysis

### Phase 1-7 Completion Summary

**✅ Completed Infrastructure (Phases 1-4):**
- SQLite metadata layer with ACID guarantees
- Multi-tenant architecture (Tenant → Database → Collection)
- User management with Argon2id password hashing
- Role-based access control (Admin/Developer/Viewer/Auditor)
- Audit logging (17 action types)
- Vector indexing (BruteForce + InstantDistance HNSW)
- >95% recall guarantee

**✅ Completed Server Layer (Phase 5 - RC1):**
- Dual API support (gRPC + REST)
- Collection persistence with auto-initialization
- 147 tests passing
- E2E integration tests
- Performance: P95 <25ms search @ 100k vectors

**✅ Completed Storage Layer (Phase 6):**
- S3/MinIO tiered storage (3 policies: Memory, MemoryS3, S3Only)
- Write-Ahead Log (WAL) for durability
- Parquet snapshots
- Background workers (S3 upload, compaction, retry)
- Crash recovery
- 95+ tests passing

**✅ Completed Production Hardening (Phase 7):**
- Circuit breaker pattern (Closed/Open/HalfOpen states)
- DLQ management (size limits, TTL, persistence)
- Batch S3 uploads (10 ops/batch)
- Parallel S3 uploads (5 concurrent)
- Optional compression (gzip)
- Prometheus metrics exporter
- Grafana dashboards (3 dashboards)
- OpenTelemetry distributed tracing
- Admin REST endpoints (3 working: health, DLQ retry, circuit breaker reset)
- 142+ tests passing

### Gaps to GA (Production-Ready)

**❌ CRITICAL Gaps (Blocks Production Use):**

1. **No Authentication** (BLOCKER)
   - Current: No API authentication (deploy behind firewall only)
   - Required: API key authentication + JWT support
   - Impact: Cannot deploy to public internet
   - Risk: HIGH - data breaches, unauthorized access

2. **No TLS Support** (BLOCKER)
   - Current: Plaintext HTTP/gRPC (use TLS termination proxy)
   - Required: Native TLS/mTLS support
   - Impact: Cannot meet compliance requirements (SOC 2, HIPAA)
   - Risk: HIGH - man-in-the-middle attacks

3. **No Rate Limiting** (BLOCKER)
   - Current: Unlimited API requests
   - Required: Per-tenant rate limits (QPS quotas)
   - Impact: Cannot prevent abuse, DoS attacks
   - Risk: MEDIUM - service degradation

**⚠️ HIGH-PRIORITY Gaps (Reduces Production Confidence):**

4. **No Kubernetes Deployment** (HIGH)
   - Current: Docker Compose only
   - Required: Helm charts, manifests, health probes
   - Impact: Cannot deploy to cloud-native platforms
   - Risk: MEDIUM - operational complexity

5. **Flaky Tests** (HIGH)
   - Current: 3 tests ignored due to timing issues
   - Required: All tests passing reliably
   - Impact: Lower CI confidence
   - Risk: LOW - tests are valid but unreliable

6. **Incomplete Admin Features** (HIGH)
   - Current: DLQ "retry" is actually "clear" (doesn't re-upload)
   - Current: No runtime config updates (immutable StorageConfig)
   - Required: Actual DLQ retry + runtime config
   - Impact: Operational inflexibility
   - Risk: LOW - workarounds exist

**🟡 MEDIUM-PRIORITY Gaps (Nice-to-Have):**

7. **No gRPC Streaming** (MEDIUM)
   - Current: Unary gRPC calls only
   - Required: Streaming for bulk insert/query
   - Impact: Lower throughput for batch operations
   - Risk: LOW - unary calls work fine

8. **No Load Testing Results** (MEDIUM)
   - Current: Benchmarks @ 100 QPS max
   - Required: Load tests @ 1000+ QPS
   - Impact: Unknown performance ceiling
   - Risk: LOW - targets already met

**🟢 LOW-PRIORITY (Post-GA):**

9. **Cedar Policy Engine** (Optional ABAC upgrade)
   - Current: Hard-coded RBAC roles
   - Optional: Cedar policy-based ABAC
   - Impact: More flexible authorization
   - Risk: NONE - current RBAC is sufficient

10. **Multi-Region Deployment** (v2.1+)
    - Current: Single-node deployment
    - Future: Distributed, multi-region
    - Impact: Higher availability
    - Risk: NONE - not required for GA

---

## Phase 8 Objectives

### Primary Goal

**Transform AkiDB 2.0 from "production-hardened infrastructure" to "enterprise-ready GA"** by implementing authentication, TLS, Kubernetes deployment, and operational polish.

### Specific Objectives

**Week 1-2: Authentication & Authorization (10 days)**
- API key authentication for REST/gRPC
- JWT token support for session management
- Multi-tenant API key isolation
- Admin API for key management (create, revoke, list)
- Secure key storage (hashed in SQLite)
- Integration with existing RBAC

**Week 3: TLS & Security Hardening (5 days)**
- TLS 1.3 support for REST API (Axum)
- TLS 1.3 support for gRPC API (Tonic)
- Certificate management (file-based, auto-reload)
- Optional mTLS for client authentication
- Security audit of authentication layer
- Vulnerability scanning (cargo-audit)

**Week 4: Rate Limiting & Quotas (5 days)**
- Per-tenant rate limiting (QPS quotas)
- Token bucket algorithm implementation
- Rate limit headers (X-RateLimit-* RFC)
- 429 Too Many Requests responses
- Admin API for quota updates
- Prometheus metrics for rate limiting

**Week 5: Kubernetes & Deployment (5 days)**
- Helm chart for AkiDB deployment
- Kubernetes manifests (Deployment, Service, ConfigMap, Secret)
- Health probes (liveness, readiness, startup)
- Resource limits (CPU, memory)
- PersistentVolumeClaim for SQLite
- Ingress configuration (TLS termination)
- Horizontal Pod Autoscaler (HPA) support

**Week 6: Operational Polish & GA Preparation (5 days)**
- Fix 3 flaky tests (deterministic async mocks)
- Implement actual DLQ retry logic (background worker)
- Runtime config updates (compaction, DLQ settings)
- Load testing @ 1000+ QPS
- Security hardening checklist
- GA release preparation

### Success Criteria

**MUST-HAVE for GA:**
- ✅ API key authentication working (REST + gRPC)
- ✅ JWT token support working
- ✅ TLS 1.3 enabled (REST + gRPC)
- ✅ Rate limiting enforced (per-tenant quotas)
- ✅ Helm chart deployable to Kubernetes
- ✅ All tests passing (0 ignored tests)
- ✅ Security audit complete (zero critical vulnerabilities)
- ✅ Load tests @ 1000 QPS passing

**NICE-TO-HAVE for GA:**
- 🟡 mTLS client authentication
- 🟡 gRPC streaming operations
- 🟡 Runtime config updates
- 🟡 Advanced monitoring dashboards

**DEFERRED to v2.1:**
- ⏸️ Cedar policy engine
- ⏸️ Multi-region deployment
- ⏸️ Read replicas
- ⏸️ Distributed coordination

---

## Strategic Options & Decision

### Option 1: Focus on Cedar Policy Engine (4 weeks)

**Pros:**
- More flexible ABAC authorization
- ADR-002 already exists with detailed plan
- Security team can author policies without code
- Compliance-friendly (audit trail)

**Cons:**
- ❌ Doesn't solve authentication gap (still can't deploy to internet)
- ❌ Doesn't solve TLS gap (still can't meet compliance)
- ❌ Doesn't solve Kubernetes gap (still manual deployment)
- ❌ Current RBAC is sufficient for GA
- ⚠️ Week 0 validation required (performance benchmark)

**Verdict:** REJECT for Phase 8. Defer to v2.1 (post-GA).

### Option 2: Focus on Multi-Region Deployment (6 weeks)

**Pros:**
- Higher availability
- Lower latency for global users
- Distributed storage

**Cons:**
- ❌ Requires authentication first (can't deploy insecure system)
- ❌ Requires TLS first (can't transmit data plaintext)
- ❌ Requires Kubernetes first (distributed = cloud-native)
- ❌ Too complex for GA (v2.2 feature)

**Verdict:** REJECT for Phase 8. Defer to v2.2.

### Option 3: Authentication + TLS + Kubernetes + Polish (6 weeks) ✅ RECOMMENDED

**Pros:**
- ✅ Solves all CRITICAL gaps (auth, TLS, rate limiting)
- ✅ Solves HIGH-PRIORITY gaps (Kubernetes, flaky tests)
- ✅ Enables public internet deployment
- ✅ Meets compliance requirements (SOC 2, HIPAA)
- ✅ Production-ready for GA
- ✅ Clear 6-week timeline

**Cons:**
- ⚠️ Defers Cedar policies (acceptable - RBAC sufficient)
- ⚠️ Defers multi-region (acceptable - v2.2 feature)
- ⚠️ Security-critical features (higher risk)

**Verdict:** ✅ ACCEPT. This is Phase 8.

### Option 4: Quick Fixes Only (2 weeks)

**Pros:**
- Fast path to RC2
- Fix flaky tests
- Implement DLQ retry

**Cons:**
- ❌ Still can't deploy to production (no auth)
- ❌ Still doesn't meet compliance (no TLS)
- ❌ Still manual deployment (no Kubernetes)

**Verdict:** REJECT. Insufficient for GA.

---

## Strategic Decision: Phase 8 = Authentication + TLS + Kubernetes + Polish

**Rationale:**
- Bridges all critical gaps from RC1 to GA
- Enables secure public deployment
- Meets enterprise compliance requirements
- Clear 6-week deliverable
- Cedar policies deferred to v2.1 (optional ABAC upgrade)

**Trade-offs Accepted:**
- Cedar policies → v2.1 (current RBAC is sufficient)
- Multi-region → v2.2 (single-node is sufficient for GA)
- gRPC streaming → v2.1 (unary calls work fine)

---

## Detailed Requirements

### Epic 1: API Authentication (Week 1-2)

#### US-801: API Key Authentication

**As a** system administrator
**I want** to issue API keys for tenants
**So that** only authorized clients can access the API

**Acceptance Criteria:**
- [ ] API keys are 32-byte random tokens (hex-encoded, 64 chars)
- [ ] Keys stored hashed in SQLite (SHA-256)
- [ ] Keys include metadata: tenant_id, name, permissions, created_at, expires_at
- [ ] REST API: `Authorization: Bearer <api_key>` header
- [ ] gRPC API: `authorization` metadata with `Bearer <api_key>`
- [ ] Middleware validates key before request processing
- [ ] Invalid key returns 401 Unauthorized
- [ ] Expired key returns 401 Unauthorized
- [ ] Key permissions checked against RBAC roles
- [ ] Metrics: `api_requests_total{authenticated=true|false}`

**Database Schema:**
```sql
CREATE TABLE api_keys (
    key_id BLOB PRIMARY KEY,           -- UUID v7
    tenant_id BLOB NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    key_hash TEXT NOT NULL UNIQUE,     -- SHA-256 hash of API key
    name TEXT NOT NULL,                -- Human-readable key name
    permissions TEXT NOT NULL,         -- JSON array of permissions
    created_at TEXT NOT NULL,
    expires_at TEXT,                   -- NULL = never expires
    last_used_at TEXT,
    created_by BLOB REFERENCES users(user_id)
) STRICT;

CREATE INDEX ix_api_keys_tenant ON api_keys(tenant_id);
CREATE INDEX ix_api_keys_hash ON api_keys(key_hash);
```

**API Endpoints:**
```
POST   /admin/api-keys           - Create API key
GET    /admin/api-keys           - List API keys (for tenant)
DELETE /admin/api-keys/{id}      - Revoke API key
GET    /admin/api-keys/{id}      - Get API key details
```

**Example Usage:**
```bash
# Create API key
curl -X POST http://localhost:8080/admin/api-keys \
  -H "Authorization: Bearer <admin-key>" \
  -d '{"name": "production-service", "permissions": ["collection::read", "collection::write"], "expires_at": "2026-01-01T00:00:00Z"}'

# Response
{
  "key_id": "01JC1234...",
  "api_key": "ak_1234567890abcdef...",  // Only returned once!
  "name": "production-service",
  "permissions": ["collection::read", "collection::write"],
  "expires_at": "2026-01-01T00:00:00Z"
}

# Use API key
curl http://localhost:8080/api/v1/collections \
  -H "Authorization: Bearer ak_1234567890abcdef..."
```

#### US-802: JWT Token Authentication

**As a** web application developer
**I want** to use JWT tokens for session management
**So that** users can authenticate once and reuse tokens

**Acceptance Criteria:**
- [ ] JWT tokens issued by `/auth/login` endpoint
- [ ] Tokens signed with HS256 (HMAC-SHA256)
- [ ] Secret key configurable (environment variable or config file)
- [ ] Token payload includes: tenant_id, user_id, role, exp (expiration)
- [ ] Default expiration: 24 hours (configurable)
- [ ] Refresh token support (optional, extend to 7 days)
- [ ] Middleware validates JWT signature and expiration
- [ ] Invalid signature returns 401 Unauthorized
- [ ] Expired token returns 401 Unauthorized
- [ ] Token permissions mapped to RBAC roles

**Login Flow:**
```bash
# Login with email/password
curl -X POST http://localhost:8080/auth/login \
  -d '{"email": "user@example.com", "password": "secret"}'

# Response
{
  "access_token": "eyJhbGciOiJIUzI1NiIs...",
  "token_type": "Bearer",
  "expires_in": 86400,
  "refresh_token": "rt_1234567890..."
}

# Use JWT token
curl http://localhost:8080/api/v1/collections \
  -H "Authorization: Bearer eyJhbGciOiJIUzI1NiIs..."
```

#### US-803: Authentication Middleware

**As a** developer
**I want** unified authentication middleware
**So that** all API endpoints are protected

**Acceptance Criteria:**
- [ ] Middleware checks `Authorization` header (REST) or metadata (gRPC)
- [ ] Supports both API keys (`Bearer ak_...`) and JWT (`Bearer eyJ...`)
- [ ] Extracts tenant_id, user_id, role from token
- [ ] Injects authentication context into request
- [ ] Public endpoints bypass authentication (health check)
- [ ] Admin endpoints require admin role
- [ ] Failed authentication logged for audit
- [ ] Prometheus metrics: `auth_failures_total{method=api_key|jwt, reason=invalid|expired}`

### Epic 2: TLS & Security (Week 3)

#### US-804: TLS 1.3 Support

**As a** security officer
**I want** TLS 1.3 encryption for all API traffic
**So that** data is encrypted in transit

**Acceptance Criteria:**
- [ ] REST API supports TLS 1.3 (Axum with rustls)
- [ ] gRPC API supports TLS 1.3 (Tonic with rustls)
- [ ] Certificate and private key loaded from files
- [ ] Certificate auto-reload on SIGHUP
- [ ] Optional certificate chain (intermediate CAs)
- [ ] Configuration: `tls.enabled`, `tls.cert_path`, `tls.key_path`
- [ ] HTTP redirects to HTTPS (optional)
- [ ] HSTS header support (Strict-Transport-Security)
- [ ] Minimum TLS version enforced (1.3 only)
- [ ] Cipher suite configuration (secure defaults)

**Configuration Example:**
```toml
[server]
rest_port = 8080
grpc_port = 9000

[server.tls]
enabled = true
cert_path = "/etc/akidb/tls/server.crt"
key_path = "/etc/akidb/tls/server.key"
min_version = "1.3"
```

**Testing:**
```bash
# Generate self-signed cert for testing
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes

# Test TLS connection
curl --cacert cert.pem https://localhost:8080/health
```

#### US-805: mTLS Client Authentication (Optional)

**As a** security officer
**I want** mutual TLS for client authentication
**So that** only trusted clients can connect

**Acceptance Criteria:**
- [ ] Optional mTLS mode (disabled by default)
- [ ] Client certificate validation
- [ ] Trusted CA certificate bundle
- [ ] Client DN (Distinguished Name) extraction
- [ ] Map client certificate to tenant/user
- [ ] Configuration: `tls.require_client_cert`, `tls.client_ca_path`
- [ ] Client cert revocation checking (optional)

#### US-806: Security Audit

**As a** security officer
**I want** a security audit of the authentication layer
**So that** I'm confident there are no vulnerabilities

**Acceptance Criteria:**
- [ ] cargo-audit scan (zero critical vulnerabilities)
- [ ] OWASP Top 10 checklist reviewed
- [ ] Password hashing reviewed (Argon2id confirmed)
- [ ] API key generation reviewed (CSPRNG confirmed)
- [ ] JWT secret key strength validated (>256 bits)
- [ ] TLS configuration reviewed (secure ciphers only)
- [ ] Rate limiting tested (DoS prevention)
- [ ] SQL injection tested (prepared statements confirmed)
- [ ] XSS tested (no user input reflected in responses)
- [ ] CSRF tested (stateless API, not vulnerable)
- [ ] Security checklist documented

### Epic 3: Rate Limiting (Week 4)

#### US-807: Per-Tenant Rate Limiting

**As a** system administrator
**I want** to enforce per-tenant QPS quotas
**So that** no single tenant can overwhelm the system

**Acceptance Criteria:**
- [ ] Token bucket algorithm per tenant
- [ ] Configurable rate limits in tenant metadata
- [ ] Default: 100 QPS per tenant (configurable)
- [ ] Burst allowance: 2x rate limit
- [ ] Rate limit checked in authentication middleware
- [ ] Exceeded limit returns 429 Too Many Requests
- [ ] Response headers: `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset`
- [ ] Prometheus metrics: `rate_limit_exceeded_total{tenant_id}`
- [ ] Admin API to update quotas

**Algorithm:**
```rust
pub struct TokenBucket {
    capacity: u64,          // Maximum tokens (burst)
    tokens: f64,            // Current tokens
    refill_rate: f64,       // Tokens per second
    last_refill: Instant,   // Last refill time
}

impl TokenBucket {
    pub fn allow(&mut self, cost: u64) -> bool {
        self.refill();
        if self.tokens >= cost as f64 {
            self.tokens -= cost as f64;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity as f64);
        self.last_refill = now;
    }
}
```

**Configuration:**
```toml
[rate_limiting]
enabled = true
default_qps = 100
default_burst = 200
```

**Response Example:**
```bash
# Request within quota
curl -i https://localhost:8080/api/v1/collections
HTTP/1.1 200 OK
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1699564800

# Request exceeds quota
curl -i https://localhost:8080/api/v1/collections
HTTP/1.1 429 Too Many Requests
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 0
X-RateLimit-Reset: 1699564800
Retry-After: 10

{"error": "Rate limit exceeded. Try again in 10 seconds."}
```

### Epic 4: Kubernetes Deployment (Week 5)

#### US-808: Helm Chart

**As a** DevOps engineer
**I want** a Helm chart for AkiDB
**So that** I can deploy to Kubernetes easily

**Acceptance Criteria:**
- [ ] Helm chart structure: `helm/akidb/`
- [ ] Chart.yaml with version and description
- [ ] values.yaml with sensible defaults
- [ ] Deployment manifest (replicas, resources)
- [ ] Service manifest (ClusterIP, ports)
- [ ] ConfigMap for configuration
- [ ] Secret for API keys and JWT secret
- [ ] PersistentVolumeClaim for SQLite database
- [ ] Health probes (liveness, readiness, startup)
- [ ] Resource limits (CPU: 1000m, memory: 2Gi)
- [ ] Ingress for TLS termination
- [ ] HorizontalPodAutoscaler support
- [ ] README with installation instructions

**Helm Chart Structure:**
```
helm/akidb/
├── Chart.yaml
├── values.yaml
├── templates/
│   ├── deployment.yaml
│   ├── service.yaml
│   ├── configmap.yaml
│   ├── secret.yaml
│   ├── pvc.yaml
│   ├── ingress.yaml
│   ├── hpa.yaml
│   └── _helpers.tpl
└── README.md
```

**Installation:**
```bash
# Install AkiDB with Helm
helm install akidb ./helm/akidb \
  --set image.tag=2.0.0-rc2 \
  --set persistence.size=10Gi \
  --set tls.enabled=true \
  --set tls.secretName=akidb-tls

# Verify deployment
kubectl get pods -l app=akidb
kubectl logs -f deploy/akidb
```

#### US-809: Health Probes

**As a** Kubernetes operator
**I want** proper health probes
**So that** Kubernetes can manage pod lifecycle

**Acceptance Criteria:**
- [ ] Liveness probe: `GET /admin/health`
- [ ] Readiness probe: `GET /admin/health`
- [ ] Startup probe: `GET /admin/health` (slower interval)
- [ ] Liveness: initialDelaySeconds=30, periodSeconds=10, failureThreshold=3
- [ ] Readiness: initialDelaySeconds=5, periodSeconds=5, failureThreshold=1
- [ ] Startup: initialDelaySeconds=0, periodSeconds=10, failureThreshold=30
- [ ] Health endpoint checks database, S3, memory
- [ ] Returns 200 OK if healthy, 503 if unhealthy

**Deployment Manifest:**
```yaml
apiVersion: apps/v1
kind: Deployment
spec:
  template:
    spec:
      containers:
      - name: akidb
        livenessProbe:
          httpGet:
            path: /admin/health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
          failureThreshold: 3
        readinessProbe:
          httpGet:
            path: /admin/health
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
          failureThreshold: 1
        startupProbe:
          httpGet:
            path: /admin/health
            port: 8080
          initialDelaySeconds: 0
          periodSeconds: 10
          failureThreshold: 30
```

### Epic 5: Operational Polish (Week 6)

#### US-810: Fix Flaky Tests

**As a** developer
**I want** all tests to pass reliably
**So that** CI is trustworthy

**Acceptance Criteria:**
- [ ] Fix `test_auto_compaction_triggered` (use MemoryS3 policy instead of Memory)
- [ ] Fix `test_e2e_s3_retry_recovery` (deterministic async mock)
- [ ] Fix `test_e2e_circuit_breaker_trip_and_recovery` (deterministic timing)
- [ ] All 3 tests no longer ignored
- [ ] All tests pass 10 consecutive times (no flakes)
- [ ] CI runs all tests (no ignored tests)

#### US-811: Actual DLQ Retry Logic

**As an** operator
**I want** DLQ retry to actually re-upload failed vectors
**So that** I don't lose data after failures

**Acceptance Criteria:**
- [ ] DLQ retry spawns background worker
- [ ] Worker re-uploads each DLQ entry to S3
- [ ] Successful uploads removed from DLQ
- [ ] Failed uploads remain in DLQ (increment retry count)
- [ ] Max 3 retry attempts per entry
- [ ] After 3 failures, entry marked as permanent failure
- [ ] Metrics: `dlq_retry_success`, `dlq_retry_failure`, `dlq_permanent_failures`
- [ ] Admin endpoint shows retry progress

#### US-812: Runtime Config Updates

**As an** operator
**I want** to update configuration at runtime
**So that** I don't need to restart for tuning

**Acceptance Criteria:**
- [ ] `POST /admin/config/compaction` - Update compaction thresholds
- [ ] `POST /admin/config/dlq` - Update DLQ max size and TTL
- [ ] `POST /admin/config/rate-limit` - Update tenant rate limits
- [ ] Configuration changes persisted to disk
- [ ] Configuration changes applied immediately (no restart)
- [ ] Configuration changes logged for audit

#### US-813: Load Testing

**As a** performance engineer
**I want** load test results @ 1000 QPS
**So that** I'm confident in production capacity

**Acceptance Criteria:**
- [ ] Load test with vegeta or k6
- [ ] 1000 QPS sustained for 10 minutes
- [ ] P50 latency <5ms
- [ ] P95 latency <25ms
- [ ] P99 latency <50ms
- [ ] Zero errors (100% success rate)
- [ ] CPU usage <80%
- [ ] Memory stable (no leaks)
- [ ] Load test report documented

#### US-814: Security Hardening Checklist

**As a** security officer
**I want** a completed security checklist
**So that** I'm confident AkiDB is production-ready

**Acceptance Criteria:**
- [ ] OWASP Top 10 reviewed (no vulnerabilities)
- [ ] cargo-audit clean (zero critical/high)
- [ ] Dependency audit (no unmaintained deps)
- [ ] Secret management reviewed (no hardcoded secrets)
- [ ] Error messages sanitized (no stack traces exposed)
- [ ] Input validation comprehensive (all user inputs)
- [ ] SQL injection impossible (prepared statements only)
- [ ] Authentication tested (no bypasses)
- [ ] Authorization tested (RBAC enforced)
- [ ] Rate limiting tested (DoS prevention)
- [ ] TLS configuration reviewed (secure ciphers)
- [ ] Security documentation complete

---

## Week-by-Week Action Plan

### Week 1: API Key Authentication (Days 1-5)

**Day 1: Database Schema + API Key Generation**
- Create `api_keys` table migration
- Implement `ApiKey` domain model
- Implement `ApiKeyRepository` with SQLite
- Add key generation function (32-byte random, SHA-256 hash)
- Unit tests (10 tests)
- **Deliverable:** API key storage working

**Day 2: API Key Validation Middleware**
- Implement REST authentication middleware (Axum)
- Implement gRPC authentication interceptor (Tonic)
- Extract API key from `Authorization: Bearer` header
- Validate key against database (hash lookup)
- Check expiration
- Integration tests (5 tests)
- **Deliverable:** API key validation working

**Day 3: API Key Admin Endpoints**
- `POST /admin/api-keys` - Create key
- `GET /admin/api-keys` - List keys
- `DELETE /admin/api-keys/{id}` - Revoke key
- `GET /admin/api-keys/{id}` - Get key details
- OpenAPI spec update
- E2E tests (4 tests)
- **Deliverable:** API key management complete

**Day 4: JWT Token Support**
- Implement `POST /auth/login` endpoint
- Email/password validation (Argon2id)
- JWT token generation (jsonwebtoken crate)
- JWT validation middleware
- Refresh token support (optional)
- Integration tests (8 tests)
- **Deliverable:** JWT authentication working

**Day 5: Week 1 Validation + Docs**
- Run all tests (expect 160+ passing)
- Performance test (auth overhead <1ms)
- Update API-TUTORIAL.md (authentication examples)
- Create SECURITY.md (authentication guide)
- Week 1 completion report
- **Deliverable:** Week 1 complete ✅

### Week 2: Authentication Polish + Integration (Days 6-10)

**Day 6: Permission Mapping**
- Map API key permissions to RBAC roles
- Implement permission checking in middleware
- Admin keys bypass rate limits
- Audit logging for authentication events
- Tests (6 tests)
- **Deliverable:** Permission system integrated

**Day 7: Authentication Metrics + Observability**
- Prometheus metrics: `auth_requests_total`, `auth_failures_total`
- Grafana dashboard: Authentication panel
- OpenTelemetry tracing for auth requests
- Alert rules for auth failures
- **Deliverable:** Auth observability complete

**Day 8: Multi-Tenant API Key Isolation**
- Ensure API keys scoped to tenant
- Cross-tenant access prevention
- Test tenant isolation (10 tests)
- Security audit of tenant isolation
- **Deliverable:** Tenant isolation verified

**Day 9: gRPC Authentication Testing**
- gRPC metadata authentication tests
- grpcurl examples with Bearer token
- Python gRPC client example with auth
- Rust gRPC client example with auth
- **Deliverable:** gRPC auth working

**Day 10: Week 2 Validation + Docs**
- Run all tests (expect 175+ passing)
- Security review of authentication layer
- Update DEPLOYMENT-GUIDE.md (API key setup)
- Week 2 completion report
- **Deliverable:** Authentication complete ✅

### Week 3: TLS & Security Hardening (Days 11-15)

**Day 11: TLS 1.3 for REST API**
- Integrate rustls with Axum
- Load certificate and private key from files
- Configuration: `tls.enabled`, `tls.cert_path`, `tls.key_path`
- HTTP → HTTPS redirect (optional)
- Tests with self-signed cert
- **Deliverable:** REST API TLS working

**Day 12: TLS 1.3 for gRPC API**
- Integrate rustls with Tonic
- Server-side TLS configuration
- Certificate chain support
- Tests with self-signed cert
- **Deliverable:** gRPC API TLS working

**Day 13: mTLS Client Authentication (Optional)**
- Client certificate validation
- Trusted CA bundle
- Client DN extraction
- Map client cert to tenant
- Tests (5 tests)
- **Deliverable:** mTLS working (optional)

**Day 14: Security Audit**
- cargo-audit scan (fix vulnerabilities)
- OWASP Top 10 checklist review
- Dependency audit (update outdated deps)
- Secret management review
- Input validation review
- **Deliverable:** Security audit complete

**Day 15: Week 3 Validation + Docs**
- Run all tests with TLS enabled
- Performance test (TLS overhead <2ms)
- Update DEPLOYMENT-GUIDE.md (TLS setup)
- Create SECURITY.md (TLS best practices)
- Week 3 completion report
- **Deliverable:** TLS complete ✅

### Week 4: Rate Limiting & Quotas (Days 16-20)

**Day 16: Token Bucket Implementation**
- Implement `TokenBucket` algorithm
- Per-tenant bucket storage (in-memory)
- Bucket persistence (optional)
- Unit tests (8 tests)
- **Deliverable:** Token bucket working

**Day 17: Rate Limiting Middleware**
- Integrate token bucket with auth middleware
- Check rate limit before request processing
- Return 429 Too Many Requests if exceeded
- Rate limit headers (X-RateLimit-*)
- Tests (6 tests)
- **Deliverable:** Rate limiting enforced

**Day 18: Rate Limit Admin Endpoints**
- `POST /admin/tenants/{id}/quota` - Update tenant quota
- `GET /admin/tenants/{id}/quota` - Get quota usage
- Default quota: 100 QPS
- Configuration: `rate_limiting.default_qps`
- Tests (4 tests)
- **Deliverable:** Quota management complete

**Day 19: Rate Limit Metrics + Observability**
- Prometheus metrics: `rate_limit_exceeded_total`
- Grafana dashboard: Rate limiting panel
- Alert rules for quota exhaustion
- **Deliverable:** Rate limit observability complete

**Day 20: Week 4 Validation + Docs**
- Run all tests (expect 195+ passing)
- Load test with rate limiting (verify 429 responses)
- Update API-TUTORIAL.md (rate limiting examples)
- Week 4 completion report
- **Deliverable:** Rate limiting complete ✅

### Week 5: Kubernetes Deployment (Days 21-25)

**Day 21: Helm Chart Structure**
- Create helm/akidb/ directory
- Chart.yaml and values.yaml
- Deployment manifest (replicas, resources)
- Service manifest (ClusterIP, ports)
- **Deliverable:** Basic Helm chart

**Day 22: ConfigMap + Secrets**
- ConfigMap for akidb.toml configuration
- Secret for API keys, JWT secret, TLS certs
- Environment variable injection
- **Deliverable:** Config management complete

**Day 23: Health Probes + PVC**
- Health probes (liveness, readiness, startup)
- PersistentVolumeClaim for SQLite database
- Resource limits (CPU, memory)
- Tests with minikube/kind
- **Deliverable:** Pod lifecycle managed

**Day 24: Ingress + HPA**
- Ingress for TLS termination
- HorizontalPodAutoscaler (target: 70% CPU)
- README with installation instructions
- **Deliverable:** Production Helm chart

**Day 25: Week 5 Validation + Docs**
- Deploy to minikube/kind cluster
- Verify health probes working
- Verify Ingress routing
- Update DEPLOYMENT-GUIDE.md (Kubernetes section)
- Week 5 completion report
- **Deliverable:** Kubernetes deployment complete ✅

### Week 6: Operational Polish & GA Prep (Days 26-30)

**Day 26: Fix Flaky Tests**
- Fix `test_auto_compaction_triggered` (use MemoryS3 policy)
- Fix `test_e2e_s3_retry_recovery` (deterministic mock)
- Fix `test_e2e_circuit_breaker_trip_and_recovery` (deterministic timing)
- All tests passing (no ignored)
- **Deliverable:** Zero flaky tests ✅

**Day 27: Actual DLQ Retry Logic**
- Implement background retry worker
- Re-upload DLQ entries to S3
- Retry count tracking (max 3 attempts)
- Metrics for retry success/failure
- Tests (5 tests)
- **Deliverable:** DLQ retry working

**Day 28: Runtime Config Updates**
- `POST /admin/config/compaction` endpoint
- `POST /admin/config/dlq` endpoint
- `POST /admin/config/rate-limit` endpoint
- Config persistence
- Tests (6 tests)
- **Deliverable:** Runtime config complete

**Day 29: Load Testing @ 1000 QPS**
- vegeta load test (1000 QPS, 10 minutes)
- P50/P95/P99 latency measurement
- CPU/memory profiling
- Load test report
- **Deliverable:** Load test passing ✅

**Day 30: GA Release Preparation**
- Security hardening checklist (complete)
- Update CHANGELOG.md (v2.0.0-GA)
- Update all documentation
- Tag release: `v2.0.0-ga`
- Phase 8 completion report
- **Deliverable:** Phase 8 COMPLETE ✅

---

## Technical Architecture

### Authentication Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    AkiDB 2.0 - Phase 8                      │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌───────────────────────────────────────────────────┐    │
│  │         REST API (Axum + TLS)                     │    │
│  │  ┌─────────────────────────────────────────────┐  │    │
│  │  │  Authentication Middleware                  │  │    │
│  │  │  ├─ Extract Bearer token                    │  │    │
│  │  │  ├─ Validate API key OR JWT                 │  │    │
│  │  │  ├─ Check expiration                        │  │    │
│  │  │  ├─ Load tenant + user context             │  │    │
│  │  │  └─ Check rate limit                        │  │    │
│  │  └─────────────────────────────────────────────┘  │    │
│  └───────────────────────────────────────────────────┘    │
│                                                             │
│  ┌───────────────────────────────────────────────────┐    │
│  │         gRPC API (Tonic + TLS)                    │    │
│  │  ┌─────────────────────────────────────────────┐  │    │
│  │  │  Authentication Interceptor                 │  │    │
│  │  │  ├─ Extract authorization metadata          │  │    │
│  │  │  ├─ Validate API key OR JWT                 │  │    │
│  │  │  ├─ Check expiration                        │  │    │
│  │  │  ├─ Load tenant + user context             │  │    │
│  │  │  └─ Check rate limit                        │  │    │
│  │  └─────────────────────────────────────────────┘  │    │
│  └───────────────────────────────────────────────────┘    │
│                                                             │
│  ┌───────────────────────────────────────────────────┐    │
│  │         Authentication Services                   │    │
│  │  ├─ ApiKeyService (validate, create, revoke)    │    │
│  │  ├─ JwtService (issue, validate, refresh)       │    │
│  │  ├─ RateLimiter (token bucket per tenant)       │    │
│  │  └─ PermissionChecker (RBAC integration)        │    │
│  └───────────────────────────────────────────────────┘    │
│                                                             │
│  ┌───────────────────────────────────────────────────┐    │
│  │         SQLite Metadata                           │    │
│  │  ├─ api_keys (key_hash, tenant_id, permissions) │    │
│  │  ├─ users (email, password_hash, role)          │    │
│  │  └─ audit_logs (auth events)                    │    │
│  └───────────────────────────────────────────────────┘    │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Request Flow with Authentication

```
┌──────────────────────────────────────────────────────────┐
│            Authenticated Request Flow                     │
└──────────────────────────────────────────────────────────┘

  Client Request
  (Authorization: Bearer <token>)
        │
        ▼
  ┌──────────────────┐
  │  TLS Handshake   │
  │  (verify cert)   │
  └────┬─────────────┘
       │
       ▼
  ┌──────────────────┐      Yes    ┌─────────────┐
  │ Extract Bearer   ├─────────────►│ API Key?    │
  │ Token            │              └──┬──────┬───┘
  └──────────────────┘                 │      │
                                   No  │      │ Yes
                                       │      │
                                       ▼      ▼
                              ┌─────────┐  ┌──────────┐
                              │ JWT     │  │ API Key  │
                              │ Validate│  │ Validate │
                              └────┬────┘  └────┬─────┘
                                   │            │
                                   └──────┬─────┘
                                          │
                                          ▼
                                  ┌───────────────┐
                                  │ Load Tenant + │
                                  │ User Context  │
                                  └───────┬───────┘
                                          │
                                          ▼
                                  ┌───────────────┐     Exceeded
                                  │ Check Rate    ├──────────────► 429 Too Many Requests
                                  │ Limit         │
                                  └───────┬───────┘
                                          │ Within Limit
                                          ▼
                                  ┌───────────────┐     Denied
                                  │ Check RBAC    ├──────────────► 403 Forbidden
                                  │ Permissions   │
                                  └───────┬───────┘
                                          │ Allowed
                                          ▼
                                  ┌───────────────┐
                                  │ Process       │
                                  │ Request       │
                                  └───────┬───────┘
                                          │
                                          ▼
                                    200 OK + Response
```

### Rate Limiting Architecture

```
┌──────────────────────────────────────────────────────────┐
│              Token Bucket Rate Limiting                   │
└──────────────────────────────────────────────────────────┘

  Per-Tenant Token Buckets (in-memory)
  ┌────────────────────────────────────┐
  │ Tenant A: TokenBucket              │
  │   capacity: 200 (burst)            │
  │   tokens: 150.5 (current)          │
  │   refill_rate: 100/sec             │
  │   last_refill: Instant::now()      │
  └────────────────────────────────────┘

  Request arrives → refill() → deduct tokens → allow/reject

  Refill Logic:
    elapsed = now - last_refill
    new_tokens = tokens + (elapsed * refill_rate)
    tokens = min(new_tokens, capacity)

  Deduct Logic:
    if tokens >= request_cost {
      tokens -= request_cost
      return ALLOW
    } else {
      return REJECT (429)
    }
```

---

## Risk Assessment

### High Risk (Requires Mitigation)

**Risk 1: Authentication Bypass Vulnerability**
- **Probability:** Low (10%)
- **Impact:** CRITICAL (data breach)
- **Scenario:** Bug in middleware allows unauthenticated requests
- **Mitigation:**
  - Comprehensive security audit
  - Penetration testing
  - Default-deny middleware (explicit allow list)
  - Unit tests for auth bypass attempts
  - E2E tests with unauthenticated requests
- **Contingency:** Immediate patching, security advisory, rollback to Phase 7

**Risk 2: JWT Secret Key Compromise**
- **Probability:** Low (5%)
- **Impact:** HIGH (session hijacking)
- **Scenario:** JWT secret leaked in logs or config
- **Mitigation:**
  - Secret key loaded from environment variable only
  - Never logged or printed
  - Minimum 256-bit random secret
  - Key rotation support (future)
- **Contingency:** Revoke all JWT tokens, rotate secret, force re-authentication

**Risk 3: Rate Limiting Bypass**
- **Probability:** Medium (20%)
- **Impact:** MEDIUM (DoS attacks possible)
- **Scenario:** Bug in token bucket allows unlimited requests
- **Mitigation:**
  - Unit tests for rate limit enforcement
  - Load tests with rate limiting
  - Prometheus alerts for quota exhaustion
  - Circuit breaker as backup (already implemented)
- **Contingency:** Disable rate limiting temporarily, fix bug, redeploy

### Medium Risk (Monitor)

**Risk 4: TLS Certificate Expiration**
- **Probability:** Medium (30%)
- **Impact:** MEDIUM (service downtime)
- **Scenario:** Certificate expires, services unavailable
- **Mitigation:**
  - Certificate expiration monitoring (Prometheus)
  - Alert 30 days before expiration
  - Auto-reload certificates on SIGHUP
  - Documentation for certificate renewal
- **Contingency:** Emergency certificate renewal, manual reload

**Risk 5: Kubernetes Deployment Complexity**
- **Probability:** Medium (25%)
- **Impact:** LOW (slower deployment)
- **Scenario:** Helm chart bugs, misconfiguration
- **Mitigation:**
  - Test with minikube/kind locally
  - Comprehensive README with examples
  - Default values for all configurations
  - Validation of required values
- **Contingency:** Docker Compose fallback (already working)

### Low Risk (Accept)

**Risk 6: Performance Degradation from Authentication**
- **Probability:** Low (15%)
- **Impact:** LOW (slightly slower requests)
- **Scenario:** Auth middleware adds >5ms latency
- **Mitigation:**
  - Performance benchmarks before/after
  - In-memory API key cache (avoid DB lookup every request)
  - Efficient JWT validation (jsonwebtoken crate)
- **Contingency:** Optimize middleware, add caching

**Risk 7: Flaky Test Fixes Break Tests**
- **Probability:** Low (10%)
- **Impact:** LOW (CI delays)
- **Scenario:** Test fixes introduce new bugs
- **Mitigation:**
  - Run tests 10+ times to verify stability
  - Deterministic async mocks
  - Proper test isolation
- **Contingency:** Revert to ignored tests temporarily

---

## Success Metrics

### Code Quality Metrics

**Target:**
- 195+ tests passing (175 Phase 7 + 20 Phase 8)
- Zero compiler errors
- Zero critical warnings
- All clippy checks passing
- Zero ignored tests (fix flaky tests)

**Measurement:**
```bash
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

### Security Metrics

**Target:**
- Zero critical/high vulnerabilities (cargo-audit)
- OWASP Top 10 compliance (checklist complete)
- Zero authentication bypasses (penetration testing)
- Zero hardcoded secrets (secret scanning)

**Measurement:**
```bash
cargo audit
# Manual penetration testing
# Secret scanning (gitleaks or similar)
```

### Performance Metrics

**Target:**
- Authentication overhead: <1ms per request
- TLS overhead: <2ms per request
- Rate limiting overhead: <0.5ms per request
- Total overhead: <3.5ms (maintains P95 <25ms target)
- Load test @ 1000 QPS: P95 <25ms, P99 <50ms

**Measurement:**
```bash
# Load test with vegeta
echo "GET https://localhost:8080/api/v1/collections" | \
  vegeta attack -rate=1000 -duration=600s -header="Authorization: Bearer <token>" | \
  vegeta report
```

### Operational Metrics

**Target:**
- Helm deployment success (1-command install)
- Health probes working (Kubernetes restarts unhealthy pods)
- TLS certificate auto-reload (zero downtime renewal)
- Rate limiting enforced (429 responses when exceeded)

**Measurement:**
- Manual Kubernetes deployment testing
- Certificate expiration simulation
- Rate limit load testing

---

## Testing Strategy

### Unit Tests (Target: 40 new tests)

**Authentication Tests (15 tests):**
- `test_api_key_generation` - Random 32-byte keys
- `test_api_key_hashing` - SHA-256 hashing
- `test_api_key_validation_valid` - Valid key accepted
- `test_api_key_validation_invalid` - Invalid key rejected
- `test_api_key_validation_expired` - Expired key rejected
- `test_jwt_token_issue` - JWT generation
- `test_jwt_token_validation_valid` - Valid JWT accepted
- `test_jwt_token_validation_expired` - Expired JWT rejected
- `test_jwt_token_validation_invalid_signature` - Bad signature rejected
- `test_permission_mapping` - API key → RBAC roles
- `test_multi_tenant_isolation` - Tenant A cannot access Tenant B keys
- `test_api_key_admin_create` - Admin can create keys
- `test_api_key_admin_revoke` - Admin can revoke keys
- `test_api_key_admin_list` - Admin can list keys
- `test_api_key_last_used_tracking` - Last used timestamp updated

**Rate Limiting Tests (10 tests):**
- `test_token_bucket_allow` - Within quota allowed
- `test_token_bucket_deny` - Exceeded quota denied
- `test_token_bucket_refill` - Tokens refill over time
- `test_token_bucket_burst` - Burst allowance working
- `test_rate_limit_per_tenant` - Separate buckets per tenant
- `test_rate_limit_headers` - X-RateLimit-* headers correct
- `test_rate_limit_429_response` - 429 status code on limit
- `test_rate_limit_admin_update` - Admin can update quotas
- `test_rate_limit_metrics` - Prometheus metrics updated
- `test_rate_limit_zero_quota` - Zero quota blocks all requests

**TLS Tests (5 tests):**
- `test_tls_certificate_loading` - Cert loaded from file
- `test_tls_handshake_success` - TLS connection established
- `test_tls_min_version_enforced` - TLS 1.2 rejected
- `test_tls_client_cert_validation` - mTLS client validation
- `test_tls_certificate_reload` - SIGHUP reloads cert

**DLQ Retry Tests (5 tests):**
- `test_dlq_retry_success` - Successful retry removes entry
- `test_dlq_retry_failure` - Failed retry increments count
- `test_dlq_retry_max_attempts` - Max 3 attempts
- `test_dlq_retry_permanent_failure` - After 3 attempts, marked permanent
- `test_dlq_retry_metrics` - Metrics updated

**Runtime Config Tests (5 tests):**
- `test_config_update_compaction` - Compaction threshold updated
- `test_config_update_dlq` - DLQ max size updated
- `test_config_update_rate_limit` - Rate limit quota updated
- `test_config_persistence` - Config saved to disk
- `test_config_reload` - Config reloaded on startup

### Integration Tests (Target: 20 new tests)

**E2E Authentication Tests (8 tests):**
- `test_e2e_api_key_authentication_rest` - REST with API key
- `test_e2e_api_key_authentication_grpc` - gRPC with API key
- `test_e2e_jwt_authentication_rest` - REST with JWT
- `test_e2e_jwt_authentication_grpc` - gRPC with JWT
- `test_e2e_unauthenticated_request_rejected` - 401 Unauthorized
- `test_e2e_expired_token_rejected` - 401 Unauthorized
- `test_e2e_invalid_token_rejected` - 401 Unauthorized
- `test_e2e_permission_denied` - 403 Forbidden

**E2E Rate Limiting Tests (4 tests):**
- `test_e2e_rate_limit_enforced` - 429 after quota exceeded
- `test_e2e_rate_limit_headers` - X-RateLimit-* headers
- `test_e2e_rate_limit_recovery` - Tokens refill over time
- `test_e2e_rate_limit_per_tenant_isolation` - Tenant A limit doesn't affect Tenant B

**E2E TLS Tests (4 tests):**
- `test_e2e_tls_rest_connection` - HTTPS connection
- `test_e2e_tls_grpc_connection` - gRPC TLS connection
- `test_e2e_tls_mtls_client_auth` - mTLS client authentication
- `test_e2e_tls_certificate_validation` - Invalid cert rejected

**E2E Kubernetes Tests (4 tests):**
- `test_e2e_helm_install` - Helm chart deploys successfully
- `test_e2e_health_probes` - Kubernetes health checks working
- `test_e2e_ingress_routing` - Ingress routes to pods
- `test_e2e_hpa_scaling` - HPA scales based on CPU

### Load Tests (Manual)

**Scenario 1: Baseline Performance (No Auth)**
- 1000 QPS for 10 minutes
- Measure P50/P95/P99 latency
- Establish baseline

**Scenario 2: Performance with API Key Auth**
- 1000 QPS for 10 minutes with API key
- Measure P50/P95/P99 latency
- Compare to baseline (overhead <1ms)

**Scenario 3: Performance with JWT Auth**
- 1000 QPS for 10 minutes with JWT
- Measure P50/P95/P99 latency
- Compare to baseline (overhead <1ms)

**Scenario 4: Rate Limiting Under Load**
- 2000 QPS (2x quota) for 5 minutes
- Verify 50% requests get 429 responses
- Verify rate limit headers correct

**Scenario 5: Kubernetes Load Test**
- Deploy to Kubernetes cluster
- 1000 QPS for 30 minutes
- Verify HPA scales up
- Verify no pod restarts

---

## Deployment Strategy

### Pre-Deployment Checklist

**Security:**
- [ ] cargo-audit clean (zero critical/high)
- [ ] OWASP Top 10 checklist complete
- [ ] Penetration testing complete
- [ ] Secret scanning complete
- [ ] Security documentation complete

**Testing:**
- [ ] 195+ tests passing
- [ ] Zero ignored tests
- [ ] Load tests passing @ 1000 QPS
- [ ] E2E tests with authentication passing
- [ ] Kubernetes deployment tests passing

**Documentation:**
- [ ] API-TUTORIAL.md updated (authentication examples)
- [ ] DEPLOYMENT-GUIDE.md updated (Kubernetes, TLS)
- [ ] SECURITY.md created (best practices)
- [ ] CHANGELOG.md updated (v2.0.0-GA)
- [ ] Migration guide (RC1 → GA)

**Operational:**
- [ ] Helm chart tested with minikube/kind
- [ ] Health probes verified
- [ ] TLS certificates prepared
- [ ] API keys generated for testing
- [ ] Monitoring dashboards updated

### Deployment Steps (v2.0.0-GA)

**Step 1: Infrastructure Preparation**
- Provision Kubernetes cluster (GKE/EKS/AKS)
- Install Prometheus + Grafana
- Install cert-manager for TLS certificates
- Create namespace: `kubectl create namespace akidb`

**Step 2: Secret Management**
- Create TLS certificate secret:
  ```bash
  kubectl create secret tls akidb-tls \
    --cert=server.crt \
    --key=server.key \
    -n akidb
  ```
- Create JWT secret:
  ```bash
  kubectl create secret generic akidb-jwt \
    --from-literal=secret=$(openssl rand -hex 32) \
    -n akidb
  ```

**Step 3: Helm Installation**
```bash
helm install akidb ./helm/akidb \
  --namespace akidb \
  --set image.tag=2.0.0-ga \
  --set persistence.size=10Gi \
  --set tls.enabled=true \
  --set tls.secretName=akidb-tls \
  --set jwt.secretName=akidb-jwt \
  --set rateLimiting.enabled=true \
  --set rateLimiting.defaultQps=100
```

**Step 4: Verify Deployment**
```bash
# Check pods
kubectl get pods -n akidb

# Check logs
kubectl logs -f deploy/akidb -n akidb

# Check health
kubectl exec -it deploy/akidb -n akidb -- \
  curl -k https://localhost:8080/admin/health

# Check TLS
kubectl port-forward svc/akidb 8080:8080 -n akidb
curl --cacert ca.crt https://localhost:8080/health
```

**Step 5: Smoke Tests**
```bash
# Create API key
kubectl exec -it deploy/akidb -n akidb -- \
  curl -X POST https://localhost:8080/admin/api-keys \
  -d '{"name":"test-key","permissions":["collection::read"]}'

# Use API key
curl -H "Authorization: Bearer <api-key>" \
  https://akidb.example.com/api/v1/collections
```

**Step 6: Gradual Rollout**
- Deploy to staging cluster (1 week testing)
- Deploy to production (10% traffic)
- Monitor for 48 hours
- Scale to 50% traffic
- Monitor for 48 hours
- Scale to 100% traffic

### Rollback Plan

**Trigger Conditions:**
- Authentication bypass detected
- TLS handshake failures >5%
- Error rate >1%
- P95 latency >50ms
- Security vulnerability discovered

**Rollback Steps:**
1. Scale down GA deployment: `kubectl scale deploy/akidb --replicas=0`
2. Deploy Phase 7 version: `helm upgrade akidb --set image.tag=2.0.0-rc1`
3. Verify health check: `200 OK`
4. Run smoke tests
5. Monitor for 1 hour
6. Investigate root cause
7. Fix and re-deploy GA

**Data Compatibility:**
- GA uses same database schema as RC1
- New tables: `api_keys`
- Backward compatible: RC1 can read GA database (ignores api_keys table)
- Forward compatible: GA can read RC1 database (no API keys = auth disabled)

---

## Comparison: Phase 8 Options

| Aspect | Option 1: Cedar | Option 2: Multi-Region | Option 3: Auth+TLS+K8s (CHOSEN) | Option 4: Quick Fixes |
|--------|-----------------|------------------------|----------------------------------|----------------------|
| **Timeline** | 4 weeks | 6 weeks | 6 weeks | 2 weeks |
| **Blocks GA** | No | No | Yes | No |
| **Production Ready** | No (still needs auth) | No (still needs auth) | ✅ Yes | No (still needs auth) |
| **Security** | 🟡 RBAC only | 🟡 RBAC only | ✅ API key + JWT + TLS | 🟡 RBAC only |
| **Deployment** | Manual | Distributed (complex) | ✅ Kubernetes | Manual |
| **Compliance** | 🟡 Partial | 🟡 Partial | ✅ SOC 2, HIPAA ready | 🟡 Partial |
| **Complexity** | Medium | Very High | Medium | Low |
| **Risk** | Low | High | Medium | Low |
| **Value** | Optional ABAC | Future HA | ✅ GA-ready | Minimal |

**Verdict:** Option 3 (Auth+TLS+K8s) is the only path to GA.

---

## Appendix A: API Examples

### API Key Authentication

**Create API Key:**
```bash
curl -X POST https://akidb.example.com/admin/api-keys \
  -H "Authorization: Bearer <admin-api-key>" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "production-service",
    "permissions": ["collection::read", "collection::write"],
    "expires_at": "2026-12-31T23:59:59Z"
  }'

# Response
{
  "key_id": "01JC1234...",
  "api_key": "ak_f8d7c6b5a4e3d2c1b0a9f8e7d6c5b4a3f2e1d0c9b8a7f6e5d4c3b2a1f0e9d8c7",
  "name": "production-service",
  "permissions": ["collection::read", "collection::write"],
  "expires_at": "2026-12-31T23:59:59Z",
  "created_at": "2025-11-08T12:00:00Z"
}
```

**Use API Key:**
```bash
curl https://akidb.example.com/api/v1/collections \
  -H "Authorization: Bearer ak_f8d7c6b5a4e3d2c1b0a9f8e7d6c5b4a3f2e1d0c9b8a7f6e5d4c3b2a1f0e9d8c7"
```

### JWT Authentication

**Login:**
```bash
curl -X POST https://akidb.example.com/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "email": "user@example.com",
    "password": "secure-password"
  }'

# Response
{
  "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "token_type": "Bearer",
  "expires_in": 86400,
  "refresh_token": "rt_a1b2c3d4e5f6..."
}
```

**Use JWT:**
```bash
curl https://akidb.example.com/api/v1/collections \
  -H "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
```

### Rate Limiting

**Within Quota:**
```bash
curl -i https://akidb.example.com/api/v1/collections \
  -H "Authorization: Bearer <api-key>"

HTTP/1.1 200 OK
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 99
X-RateLimit-Reset: 1699564800
```

**Exceeded Quota:**
```bash
curl -i https://akidb.example.com/api/v1/collections \
  -H "Authorization: Bearer <api-key>"

HTTP/1.1 429 Too Many Requests
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 0
X-RateLimit-Reset: 1699564810
Retry-After: 10

{"error":"Rate limit exceeded. Try again in 10 seconds."}
```

---

## Appendix B: Kubernetes Examples

### Helm Installation

```bash
# Install AkiDB with all features
helm install akidb ./helm/akidb \
  --namespace akidb \
  --create-namespace \
  --set image.repository=akidb/akidb \
  --set image.tag=2.0.0-ga \
  --set replicaCount=3 \
  --set persistence.size=20Gi \
  --set tls.enabled=true \
  --set tls.secretName=akidb-tls \
  --set jwt.secretName=akidb-jwt \
  --set rateLimiting.enabled=true \
  --set rateLimiting.defaultQps=1000 \
  --set resources.limits.cpu=2000m \
  --set resources.limits.memory=4Gi \
  --set ingress.enabled=true \
  --set ingress.hosts[0].host=akidb.example.com \
  --set autoscaling.enabled=true \
  --set autoscaling.minReplicas=3 \
  --set autoscaling.maxReplicas=10 \
  --set autoscaling.targetCPUUtilizationPercentage=70
```

### Health Check

```bash
# Port forward
kubectl port-forward svc/akidb 8080:8080 -n akidb

# Check health
curl -k https://localhost:8080/admin/health

# Response
{
  "status": "healthy",
  "version": "2.0.0",
  "uptime_seconds": 3600,
  "components": {
    "database": {"status": "healthy"},
    "storage": {"status": "healthy"},
    "memory": {"status": "healthy"}
  }
}
```

---

## Conclusion

Phase 8 transforms AkiDB 2.0 from Phase 7 production-hardened infrastructure to **enterprise-ready GA** by implementing:

**Critical Features:**
- ✅ API key + JWT authentication
- ✅ TLS 1.3 encryption
- ✅ Per-tenant rate limiting
- ✅ Kubernetes deployment (Helm charts)
- ✅ Operational polish (fix flaky tests, DLQ retry, runtime config)

**Timeline:** 6 weeks (30 working days)
**Risk:** Medium (security-critical features, comprehensive testing required)
**Impact:** HIGH (enables production deployment for enterprises)

**Next Milestone:** v2.0.0-GA (Production-Ready) 🚀

---

**Document Version:** 1.0
**Last Updated:** 2025-11-08
**Status:** PLANNING - Ready for Review ✅
**Next Step:** Stakeholder approval → Begin Week 1 implementation
