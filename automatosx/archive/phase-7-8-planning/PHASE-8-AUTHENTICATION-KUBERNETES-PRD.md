# Phase 8: Authentication, TLS & Kubernetes Deployment - PRD

**Version:** 1.0
**Date:** 2025-11-08
**Status:** PLANNING
**Dependencies:** Phase 7 Complete ✅
**Target Milestone:** v2.0.0-GA

---

## Executive Summary

Phase 8 bridges the critical gap between Phase 7's production-hardened infrastructure and a GA-ready enterprise product by implementing **authentication, TLS encryption, rate limiting, and Kubernetes deployment**.

**Current State:** Phase 7 complete with circuit breaker, DLQ management, observability, and admin APIs. However, **cannot deploy to production** due to missing authentication and TLS.

**Phase 8 Deliverables:**
1. **API Key Authentication** - Secure API access with tenant-scoped keys
2. **JWT Token Support** - Session management for web applications
3. **TLS 1.3 Encryption** - Secure data transmission (REST + gRPC)
4. **Per-Tenant Rate Limiting** - DoS protection and quota enforcement
5. **Kubernetes Deployment** - Production-grade deployment with Helm charts
6. **Operational Polish** - Fix flaky tests, implement DLQ retry, runtime config

**Timeline:** 6 weeks (30 working days)
**Team:** 1 developer (AI-assisted)
**Risk:** Medium (security-critical features)

**Success Criteria:**
- ✅ Secure public internet deployment (authentication + TLS)
- ✅ Compliance-ready (SOC 2, HIPAA)
- ✅ Kubernetes production deployment (Helm charts)
- ✅ Load tested @ 1000 QPS
- ✅ Zero critical security vulnerabilities
- ✅ All tests passing (195+ tests, zero ignored)

---

## Problem Statement

### Current Gaps to GA

**BLOCKER Gaps (Cannot Deploy to Production):**

1. **No Authentication** ❌
   - Current: Open API (anyone can access)
   - Impact: Cannot deploy to public internet
   - Risk: Data breaches, unauthorized access
   - Compliance: Fails SOC 2, HIPAA, GDPR

2. **No TLS Encryption** ❌
   - Current: Plaintext HTTP/gRPC
   - Impact: Man-in-the-middle attacks possible
   - Risk: Data interception, credential theft
   - Compliance: Fails PCI-DSS, HIPAA

3. **No Rate Limiting** ❌
   - Current: Unlimited API requests
   - Impact: DoS attacks possible
   - Risk: Service degradation, abuse
   - Compliance: No quota enforcement

4. **No Kubernetes Deployment** ⚠️
   - Current: Docker Compose only
   - Impact: Manual deployment, no cloud-native support
   - Risk: Operational complexity

5. **Flaky Tests** ⚠️
   - Current: 3 tests ignored (timing-dependent)
   - Impact: Lower CI confidence
   - Risk: Bugs may slip through

### Target State (GA-Ready)

- ✅ API key authentication (tenant-scoped, revocable)
- ✅ JWT token support (session management)
- ✅ TLS 1.3 encryption (REST + gRPC)
- ✅ Per-tenant rate limiting (QPS quotas)
- ✅ Kubernetes Helm charts (1-command deployment)
- ✅ Health probes (liveness, readiness)
- ✅ All tests passing (zero ignored tests)
- ✅ Security audit complete (zero critical vulnerabilities)
- ✅ Load tested @ 1000 QPS
- ✅ Production deployment guide

---

## Goals & Non-Goals

### Goals (In Scope)

**Primary Goals:**

1. **Authentication & Authorization**
   - API key authentication (32-byte random tokens, SHA-256 hashed)
   - JWT token support (HS256 signing, 24-hour expiration)
   - Multi-tenant API key isolation
   - Admin API for key management (create, revoke, list)
   - Integration with existing RBAC roles
   - Audit logging for auth events

2. **TLS & Security**
   - TLS 1.3 support (REST + gRPC)
   - Certificate management (file-based, auto-reload)
   - Optional mTLS for client authentication
   - Security audit (OWASP Top 10, cargo-audit)
   - Vulnerability scanning
   - Penetration testing

3. **Rate Limiting**
   - Per-tenant QPS quotas (default: 100 QPS)
   - Token bucket algorithm
   - Rate limit headers (X-RateLimit-* RFC)
   - 429 Too Many Requests responses
   - Admin API for quota updates
   - Prometheus metrics

4. **Kubernetes Deployment**
   - Helm chart for AkiDB
   - Deployment, Service, ConfigMap, Secret manifests
   - Health probes (liveness, readiness, startup)
   - PersistentVolumeClaim for SQLite
   - Ingress configuration (TLS termination)
   - HorizontalPodAutoscaler support
   - Resource limits (CPU, memory)

5. **Operational Polish**
   - Fix 3 flaky tests (deterministic mocks)
   - Implement actual DLQ retry (background worker)
   - Runtime config updates (compaction, DLQ, rate limits)
   - Load testing @ 1000 QPS
   - Security hardening checklist
   - GA release preparation

**Secondary Goals:**

6. **Documentation**
   - API authentication guide
   - TLS setup guide
   - Kubernetes deployment guide
   - Security best practices
   - Migration guide (RC1 → GA)

### Non-Goals (Out of Scope)

**Explicitly NOT in Phase 8:**

1. **Cedar Policy Engine** → Deferred to v2.1
   - Rationale: Current RBAC is sufficient for GA
   - Optional ABAC upgrade post-GA

2. **Multi-Region Deployment** → Deferred to v2.2
   - Rationale: Single-node is sufficient for GA
   - Distributed features post-GA

3. **gRPC Streaming** → Deferred to v2.1
   - Rationale: Unary calls work fine
   - Nice-to-have, not blocker

4. **Vector Compression** → Deferred to v2.1
   - Rationale: Storage optimization post-GA
   - Not required for GA

5. **Read Replicas** → Deferred to v2.2
   - Rationale: Single-node HA sufficient
   - Distributed features post-GA

---

## User Stories

### Epic 1: Authentication (Week 1-2)

**US-801: API Key Authentication**

**As a** system administrator
**I want** to issue API keys for tenants
**So that** only authorized clients can access the API

**Acceptance Criteria:**
- [ ] API keys are 32-byte random tokens (hex-encoded, 64 chars)
- [ ] Keys stored hashed in SQLite (SHA-256)
- [ ] REST API: `Authorization: Bearer <api_key>` header
- [ ] gRPC API: `authorization` metadata
- [ ] Invalid key returns 401 Unauthorized
- [ ] Expired key returns 401 Unauthorized
- [ ] Admin API: create, revoke, list keys
- [ ] Metrics: `api_requests_total{authenticated=true|false}`

**US-802: JWT Token Support**

**As a** web application developer
**I want** to use JWT tokens for session management
**So that** users can authenticate once and reuse tokens

**Acceptance Criteria:**
- [ ] JWT tokens issued by `/auth/login` endpoint
- [ ] Tokens signed with HS256 (HMAC-SHA256)
- [ ] Default expiration: 24 hours (configurable)
- [ ] Token payload: tenant_id, user_id, role, exp
- [ ] Invalid signature returns 401 Unauthorized
- [ ] Expired token returns 401 Unauthorized

### Epic 2: TLS & Security (Week 3)

**US-804: TLS 1.3 Support**

**As a** security officer
**I want** TLS 1.3 encryption for all API traffic
**So that** data is encrypted in transit

**Acceptance Criteria:**
- [ ] REST API supports TLS 1.3 (Axum + rustls)
- [ ] gRPC API supports TLS 1.3 (Tonic + rustls)
- [ ] Certificate loaded from files (cert_path, key_path)
- [ ] Certificate auto-reload on SIGHUP
- [ ] Minimum TLS version enforced (1.3 only)
- [ ] HSTS header support

**US-806: Security Audit**

**As a** security officer
**I want** a security audit of the authentication layer
**So that** I'm confident there are no vulnerabilities

**Acceptance Criteria:**
- [ ] cargo-audit clean (zero critical/high vulnerabilities)
- [ ] OWASP Top 10 checklist complete
- [ ] Penetration testing complete
- [ ] SQL injection tested (prepared statements confirmed)
- [ ] Security documentation complete

### Epic 3: Rate Limiting (Week 4)

**US-807: Per-Tenant Rate Limiting**

**As a** system administrator
**I want** to enforce per-tenant QPS quotas
**So that** no single tenant can overwhelm the system

**Acceptance Criteria:**
- [ ] Token bucket algorithm per tenant
- [ ] Default: 100 QPS per tenant (configurable)
- [ ] Burst allowance: 2x rate limit
- [ ] Exceeded limit returns 429 Too Many Requests
- [ ] Response headers: `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset`
- [ ] Admin API to update quotas
- [ ] Metrics: `rate_limit_exceeded_total{tenant_id}`

### Epic 4: Kubernetes (Week 5)

**US-808: Helm Chart**

**As a** DevOps engineer
**I want** a Helm chart for AkiDB
**So that** I can deploy to Kubernetes easily

**Acceptance Criteria:**
- [ ] Helm chart structure: `helm/akidb/`
- [ ] values.yaml with sensible defaults
- [ ] Deployment manifest (replicas, resources)
- [ ] Service manifest (ClusterIP, ports)
- [ ] ConfigMap for configuration
- [ ] Secret for API keys and JWT secret
- [ ] PersistentVolumeClaim for SQLite
- [ ] Ingress for TLS termination
- [ ] HorizontalPodAutoscaler support
- [ ] README with installation instructions

**US-809: Health Probes**

**As a** Kubernetes operator
**I want** proper health probes
**So that** Kubernetes can manage pod lifecycle

**Acceptance Criteria:**
- [ ] Liveness probe: `GET /admin/health`
- [ ] Readiness probe: `GET /admin/health`
- [ ] Startup probe: `GET /admin/health`
- [ ] Health endpoint checks database, S3, memory
- [ ] Returns 200 OK if healthy, 503 if unhealthy

### Epic 5: Operational Polish (Week 6)

**US-810: Fix Flaky Tests**

**As a** developer
**I want** all tests to pass reliably
**So that** CI is trustworthy

**Acceptance Criteria:**
- [ ] Fix `test_auto_compaction_triggered` (use MemoryS3 policy)
- [ ] Fix `test_e2e_s3_retry_recovery` (deterministic mock)
- [ ] Fix `test_e2e_circuit_breaker_trip_and_recovery` (deterministic timing)
- [ ] All 3 tests no longer ignored
- [ ] All tests pass 10 consecutive times

**US-813: Load Testing**

**As a** performance engineer
**I want** load test results @ 1000 QPS
**So that** I'm confident in production capacity

**Acceptance Criteria:**
- [ ] 1000 QPS sustained for 10 minutes
- [ ] P50 <5ms, P95 <25ms, P99 <50ms
- [ ] Zero errors (100% success rate)
- [ ] CPU usage <80%
- [ ] Memory stable (no leaks)
- [ ] Load test report documented

---

## Technical Architecture

### Authentication Flow

```
Client Request (Authorization: Bearer <token>)
        ↓
  TLS Handshake
        ↓
Extract Bearer Token
        ↓
API Key? ────Yes───→ Validate API Key (SHA-256 hash lookup)
    ↓ No                    ↓
Validate JWT         Check Expiration
    ↓                       ↓
Load Tenant + User Context
        ↓
Check Rate Limit ────Exceeded───→ 429 Too Many Requests
    ↓ Within Limit
Check RBAC Permissions ────Denied───→ 403 Forbidden
    ↓ Allowed
Process Request
        ↓
   200 OK
```

### Database Schema

**api_keys Table:**
```sql
CREATE TABLE api_keys (
    key_id BLOB PRIMARY KEY,           -- UUID v7
    tenant_id BLOB NOT NULL REFERENCES tenants(tenant_id),
    key_hash TEXT NOT NULL UNIQUE,     -- SHA-256 hash
    name TEXT NOT NULL,
    permissions TEXT NOT NULL,         -- JSON array
    created_at TEXT NOT NULL,
    expires_at TEXT,
    last_used_at TEXT,
    created_by BLOB REFERENCES users(user_id)
) STRICT;
```

### Configuration

```toml
[server.tls]
enabled = true
cert_path = "/etc/akidb/tls/server.crt"
key_path = "/etc/akidb/tls/server.key"
min_version = "1.3"

[authentication]
jwt_secret = "${JWT_SECRET}"  # Environment variable
jwt_expiration_hours = 24

[rate_limiting]
enabled = true
default_qps = 100
default_burst = 200
```

---

## Week-by-Week Plan

### Week 1: API Key Authentication (Days 1-5)
- Day 1: Database schema + key generation
- Day 2: Validation middleware (REST + gRPC)
- Day 3: Admin endpoints (create, revoke, list)
- Day 4: JWT token support
- Day 5: Validation + docs

### Week 2: Authentication Polish (Days 6-10)
- Day 6: Permission mapping (API key → RBAC)
- Day 7: Metrics + observability
- Day 8: Multi-tenant isolation testing
- Day 9: gRPC authentication testing
- Day 10: Validation + docs

### Week 3: TLS & Security (Days 11-15)
- Day 11: TLS 1.3 for REST API
- Day 12: TLS 1.3 for gRPC API
- Day 13: mTLS client auth (optional)
- Day 14: Security audit (cargo-audit, OWASP Top 10)
- Day 15: Validation + docs

### Week 4: Rate Limiting (Days 16-20)
- Day 16: Token bucket implementation
- Day 17: Rate limiting middleware
- Day 18: Admin endpoints (quota management)
- Day 19: Metrics + observability
- Day 20: Validation + docs

### Week 5: Kubernetes (Days 21-25)
- Day 21: Helm chart structure
- Day 22: ConfigMap + Secrets
- Day 23: Health probes + PVC
- Day 24: Ingress + HPA
- Day 25: Validation + docs

### Week 6: Polish & GA (Days 26-30)
- Day 26: Fix flaky tests
- Day 27: DLQ retry logic
- Day 28: Runtime config updates
- Day 29: Load testing @ 1000 QPS
- Day 30: GA release preparation

---

## Success Metrics

### Code Quality
- **Target:** 195+ tests passing (175 Phase 7 + 20 Phase 8)
- **Target:** Zero compiler warnings
- **Target:** Zero ignored tests

### Security
- **Target:** Zero critical/high vulnerabilities (cargo-audit)
- **Target:** OWASP Top 10 compliance
- **Target:** Zero authentication bypasses

### Performance
- **Target:** Auth overhead <1ms
- **Target:** TLS overhead <2ms
- **Target:** Rate limiting overhead <0.5ms
- **Target:** Load test @ 1000 QPS: P95 <25ms

### Operational
- **Target:** 1-command Helm deployment
- **Target:** Health probes working
- **Target:** TLS certificate auto-reload
- **Target:** Rate limiting enforced

---

## Risk Assessment

### High Risk

**Risk 1: Authentication Bypass**
- **Probability:** Low (10%)
- **Impact:** CRITICAL (data breach)
- **Mitigation:** Security audit, penetration testing, default-deny middleware
- **Contingency:** Immediate patching, rollback to Phase 7

**Risk 2: JWT Secret Compromise**
- **Probability:** Low (5%)
- **Impact:** HIGH (session hijacking)
- **Mitigation:** Environment-only secrets, never logged, 256-bit minimum
- **Contingency:** Revoke all tokens, rotate secret

### Medium Risk

**Risk 3: Rate Limiting Bypass**
- **Probability:** Medium (20%)
- **Impact:** MEDIUM (DoS possible)
- **Mitigation:** Unit tests, load tests, circuit breaker backup
- **Contingency:** Disable rate limiting temporarily, fix, redeploy

**Risk 4: TLS Certificate Expiration**
- **Probability:** Medium (30%)
- **Impact:** MEDIUM (service downtime)
- **Mitigation:** Monitoring, alerts 30 days before expiration, auto-reload
- **Contingency:** Emergency renewal, manual reload

### Low Risk

**Risk 5: Kubernetes Deployment Complexity**
- **Probability:** Medium (25%)
- **Impact:** LOW (slower deployment)
- **Mitigation:** Local testing, comprehensive README, default values
- **Contingency:** Docker Compose fallback

---

## Testing Strategy

### Unit Tests (40 new tests)
- 15 authentication tests
- 10 rate limiting tests
- 5 TLS tests
- 5 DLQ retry tests
- 5 runtime config tests

### Integration Tests (20 new tests)
- 8 E2E authentication tests
- 4 E2E rate limiting tests
- 4 E2E TLS tests
- 4 E2E Kubernetes tests

### Load Tests (Manual)
- Baseline @ 1000 QPS
- With API key auth @ 1000 QPS
- With JWT auth @ 1000 QPS
- Rate limiting under load (2000 QPS)
- Kubernetes load test (30 min)

---

## Deployment Strategy

### Pre-Deployment Checklist
- [ ] cargo-audit clean
- [ ] 195+ tests passing
- [ ] Load tests passing @ 1000 QPS
- [ ] Security documentation complete
- [ ] Helm chart tested
- [ ] TLS certificates prepared

### Deployment Steps
1. Provision Kubernetes cluster
2. Install Prometheus + Grafana
3. Create TLS and JWT secrets
4. Helm install AkiDB
5. Verify deployment (health checks)
6. Smoke tests (API key, JWT)
7. Gradual rollout (10% → 50% → 100%)

### Rollback Plan
- **Trigger:** Auth bypass, TLS failures >5%, error rate >1%
- **Steps:** Scale down GA, deploy Phase 7, verify, monitor, investigate

---

## Dependencies

**Rust Crates:**
- `jsonwebtoken` - JWT token generation/validation
- `sha2` - SHA-256 hashing for API keys
- `rustls` - TLS 1.3 implementation
- `axum-server` - TLS support for Axum
- `tonic` - TLS support for gRPC
- `rand` - Cryptographically secure random key generation

**Infrastructure:**
- Kubernetes cluster (GKE/EKS/AKS or minikube/kind)
- cert-manager (optional, for TLS cert automation)
- Prometheus + Grafana (monitoring)

**Tools:**
- `vegeta` or `k6` - Load testing
- `cargo-audit` - Security scanning
- `helm` - Kubernetes deployment

---

## Conclusion

Phase 8 is the **critical path to GA**, implementing authentication, TLS, rate limiting, and Kubernetes deployment. Without Phase 8, AkiDB cannot be deployed to production.

**Timeline:** 6 weeks (30 working days)
**Risk:** Medium (security-critical, comprehensive testing required)
**Impact:** HIGH (enables production deployment)

**Next Milestone:** v2.0.0-GA (Production-Ready) 🚀

---

**Document Version:** 1.0
**Last Updated:** 2025-11-08
**Status:** PLANNING - Ready for Implementation
**Approval:** Pending stakeholder review
