# Phase 8 Week 2: Authentication Polish & Integration - COMPREHENSIVE MEGATHINK

**Date:** 2025-11-08
**Status:** PLANNING
**Timeline:** 5 working days (Days 6-10)
**Dependencies:** Phase 8 Week 1 Complete ✅
**Target:** Production-grade authentication with observability and hardening

---

## Executive Summary

Week 2 transforms Week 1's **functional authentication** into a **production-grade, enterprise-ready authentication system** by adding permission mapping, comprehensive observability, security hardening, and operational polish.

**Week 1 Achievements:**
- ✅ API key authentication (create, validate, revoke)
- ✅ JWT token support (login, validate)
- ✅ Authentication middleware (REST + gRPC)
- ✅ Admin endpoints (manage API keys)
- ✅ 28 tests passing (unit + integration + E2E)

**Week 2 Focus:**
1. **Permission Mapping** - API key permissions → RBAC roles
2. **Observability** - Metrics, tracing, dashboards
3. **Security Hardening** - Audit logging, rate limiting prep, multi-tenant isolation
4. **gRPC Testing** - Comprehensive gRPC auth testing
5. **Operational Polish** - Caching, performance optimization, documentation

**Week 2 Deliverables:**
1. **Permission System** - Granular permission checking (17 action types)
2. **Auth Metrics** - Prometheus metrics for all auth events
3. **Grafana Dashboard** - Authentication monitoring panel
4. **OpenTelemetry Tracing** - Distributed traces for auth requests
5. **Audit Logging** - All auth events logged for compliance
6. **Multi-Tenant Isolation Tests** - 10+ tests verifying tenant boundaries
7. **gRPC Auth Examples** - Python, Rust, grpcurl examples
8. **Performance Optimization** - In-memory API key cache (5-min TTL)
9. **20+ Tests** - Additional coverage for edge cases

**Success Criteria:**
- ✅ All 17 RBAC action types supported
- ✅ Auth metrics exposed to Prometheus
- ✅ Grafana dashboard showing auth failures, latency
- ✅ Audit log for every auth event (success + failure)
- ✅ Multi-tenant isolation verified (10+ tests)
- ✅ gRPC authentication working (Python + Rust clients)
- ✅ Auth overhead <2ms (with caching)
- ✅ 195+ tests passing (175 Week 1 + 20 Week 2)

---

## Table of Contents

1. [Permission Mapping System](#permission-mapping-system)
2. [Observability Infrastructure](#observability-infrastructure)
3. [Security Hardening](#security-hardening)
4. [Multi-Tenant Isolation](#multi-tenant-isolation)
5. [gRPC Authentication](#grpc-authentication)
6. [Performance Optimization](#performance-optimization)
7. [Day-by-Day Action Plan](#day-by-day-action-plan)
8. [Testing Strategy](#testing-strategy)
9. [Metrics & Monitoring](#metrics--monitoring)
10. [Risk Assessment](#risk-assessment)

---

## Permission Mapping System

### RBAC Permissions (17 Action Types)

From Phase 3, we have 17 granular action types across 4 resource categories:

**User Actions (4):**
- `user::create` - Create new users
- `user::read` - View user information
- `user::update` - Update user information
- `user::delete` - Delete users

**Collection Actions (6):**
- `collection::create` - Create collections
- `collection::read` - View collection metadata
- `collection::update` - Update collection settings
- `collection::delete` - Delete collections
- `collection::insert` - Insert vectors
- `collection::query` - Query vectors

**Database Actions (4):**
- `database::create` - Create databases
- `database::read` - View database information
- `database::update` - Update database settings
- `database::delete` - Delete databases

**Audit Actions (3):**
- `audit::read` - View audit logs
- `audit::export` - Export audit logs
- `audit::manage` - Manage audit settings

### Permission Mapping Logic

```rust
// File: crates/akidb-core/src/permissions.rs (NEW FILE)

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    // User actions
    UserCreate,
    UserRead,
    UserUpdate,
    UserDelete,

    // Collection actions
    CollectionCreate,
    CollectionRead,
    CollectionUpdate,
    CollectionDelete,
    CollectionInsert,
    CollectionQuery,

    // Database actions
    DatabaseCreate,
    DatabaseRead,
    DatabaseUpdate,
    DatabaseDelete,

    // Audit actions
    AuditRead,
    AuditExport,
    AuditManage,
}

impl Action {
    pub fn as_str(&self) -> &'static str {
        match self {
            Action::UserCreate => "user::create",
            Action::UserRead => "user::read",
            Action::UserUpdate => "user::update",
            Action::UserDelete => "user::delete",
            Action::CollectionCreate => "collection::create",
            Action::CollectionRead => "collection::read",
            Action::CollectionUpdate => "collection::update",
            Action::CollectionDelete => "collection::delete",
            Action::CollectionInsert => "collection::insert",
            Action::CollectionQuery => "collection::query",
            Action::DatabaseCreate => "database::create",
            Action::DatabaseRead => "database::read",
            Action::DatabaseUpdate => "database::update",
            Action::DatabaseDelete => "database::delete",
            Action::AuditRead => "audit::read",
            Action::AuditExport => "audit::export",
            Action::AuditManage => "audit::manage",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "user::create" => Some(Action::UserCreate),
            "user::read" => Some(Action::UserRead),
            "user::update" => Some(Action::UserUpdate),
            "user::delete" => Some(Action::UserDelete),
            "collection::create" => Some(Action::CollectionCreate),
            "collection::read" => Some(Action::CollectionRead),
            "collection::update" => Some(Action::CollectionUpdate),
            "collection::delete" => Some(Action::CollectionDelete),
            "collection::insert" => Some(Action::CollectionInsert),
            "collection::query" => Some(Action::CollectionQuery),
            "database::create" => Some(Action::DatabaseCreate),
            "database::read" => Some(Action::DatabaseRead),
            "database::update" => Some(Action::DatabaseUpdate),
            "database::delete" => Some(Action::DatabaseDelete),
            "audit::read" => Some(Action::AuditRead),
            "audit::export" => Some(Action::AuditExport),
            "audit::manage" => Some(Action::AuditManage),
            _ => None,
        }
    }

    pub fn all() -> Vec<Action> {
        vec![
            Action::UserCreate, Action::UserRead, Action::UserUpdate, Action::UserDelete,
            Action::CollectionCreate, Action::CollectionRead, Action::CollectionUpdate,
            Action::CollectionDelete, Action::CollectionInsert, Action::CollectionQuery,
            Action::DatabaseCreate, Action::DatabaseRead, Action::DatabaseUpdate,
            Action::DatabaseDelete, Action::AuditRead, Action::AuditExport, Action::AuditManage,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Admin,      // Full access
    Developer,  // Collection + Database read/write
    Viewer,     // Read-only access
    Auditor,    // Audit log access only
}

impl Role {
    pub fn permissions(&self) -> Vec<Action> {
        match self {
            Role::Admin => Action::all(),
            Role::Developer => vec![
                Action::CollectionCreate,
                Action::CollectionRead,
                Action::CollectionUpdate,
                Action::CollectionDelete,
                Action::CollectionInsert,
                Action::CollectionQuery,
                Action::DatabaseCreate,
                Action::DatabaseRead,
                Action::DatabaseUpdate,
                Action::DatabaseDelete,
            ],
            Role::Viewer => vec![
                Action::CollectionRead,
                Action::CollectionQuery,
                Action::DatabaseRead,
            ],
            Role::Auditor => vec![
                Action::AuditRead,
                Action::AuditExport,
            ],
        }
    }

    pub fn has_permission(&self, action: &Action) -> bool {
        self.permissions().contains(action)
    }
}

pub struct PermissionChecker;

impl PermissionChecker {
    pub fn check(auth_context: &AuthContext, required_action: &Action) -> CoreResult<()> {
        // Check if user has required permission
        let has_permission = auth_context.permissions.iter()
            .any(|p| Action::from_str(p) == Some(required_action.clone()));

        if has_permission {
            Ok(())
        } else {
            Err(CoreError::forbidden(
                format!("Missing required permission: {}", required_action.as_str())
            ))
        }
    }

    pub fn check_any(auth_context: &AuthContext, required_actions: &[Action]) -> CoreResult<()> {
        // Check if user has ANY of the required permissions
        let has_any = required_actions.iter().any(|action| {
            auth_context.permissions.iter()
                .any(|p| Action::from_str(p) == Some(action.clone()))
        });

        if has_any {
            Ok(())
        } else {
            Err(CoreError::forbidden(
                format!("Missing any of required permissions: {:?}", required_actions)
            ))
        }
    }

    pub fn check_all(auth_context: &AuthContext, required_actions: &[Action]) -> CoreResult<()> {
        // Check if user has ALL required permissions
        let missing: Vec<_> = required_actions.iter()
            .filter(|action| {
                !auth_context.permissions.iter()
                    .any(|p| Action::from_str(p) == Some((*action).clone()))
            })
            .collect();

        if missing.is_empty() {
            Ok(())
        } else {
            Err(CoreError::forbidden(
                format!("Missing required permissions: {:?}", missing)
            ))
        }
    }
}
```

### Integration with Handlers

```rust
// Example: Collection creation endpoint

pub async fn create_collection(
    State(service): State<Arc<CollectionService>>,
    auth: Extension<AuthContext>,
    Json(req): Json<CreateCollectionRequest>,
) -> Result<Json<CollectionResponse>, (StatusCode, String)> {
    // Check permission BEFORE processing request
    PermissionChecker::check(&auth, &Action::CollectionCreate)
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;

    // Permission granted, proceed with request
    let collection = service.create_collection(
        auth.tenant_id,
        req.name,
        req.dimension,
        req.metric,
    ).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(collection.into()))
}
```

### Wildcard Permissions

For API keys, support wildcard permissions for convenience:

```rust
// API key with wildcard permissions
{
  "name": "admin-key",
  "permissions": ["*"]  // All permissions
}

// API key with category wildcard
{
  "name": "collection-admin",
  "permissions": ["collection::*"]  // All collection actions
}

impl PermissionChecker {
    pub fn check_with_wildcards(auth_context: &AuthContext, required_action: &Action) -> CoreResult<()> {
        // Check exact match
        if auth_context.permissions.contains(&required_action.as_str().to_string()) {
            return Ok(());
        }

        // Check wildcard: "*"
        if auth_context.permissions.contains(&"*".to_string()) {
            return Ok(());
        }

        // Check category wildcard: "collection::*"
        let category = required_action.as_str().split("::").next().unwrap();
        let wildcard = format!("{}::*", category);
        if auth_context.permissions.contains(&wildcard) {
            return Ok(());
        }

        Err(CoreError::forbidden(
            format!("Missing required permission: {}", required_action.as_str())
        ))
    }
}
```

---

## Observability Infrastructure

### Prometheus Metrics

**Authentication Metrics (10 metrics):**

```rust
// File: crates/akidb-rest/src/metrics/auth.rs (NEW FILE)

use prometheus::{
    Counter, CounterVec, Histogram, HistogramVec, IntGauge, Registry,
    Opts, HistogramOpts,
};
use lazy_static::lazy_static;

lazy_static! {
    // Total authentication requests
    pub static ref AUTH_REQUESTS_TOTAL: CounterVec = CounterVec::new(
        Opts::new("auth_requests_total", "Total authentication requests"),
        &["method", "result"]  // method: api_key|jwt, result: success|failure
    ).unwrap();

    // Authentication failures by reason
    pub static ref AUTH_FAILURES_TOTAL: CounterVec = CounterVec::new(
        Opts::new("auth_failures_total", "Total authentication failures"),
        &["method", "reason"]  // reason: invalid|expired|revoked|missing
    ).unwrap();

    // Authentication latency
    pub static ref AUTH_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new("auth_duration_seconds", "Authentication latency")
            .buckets(vec![0.0001, 0.0005, 0.001, 0.002, 0.005, 0.01, 0.05, 0.1]),
        &["method"]  // method: api_key|jwt
    ).unwrap();

    // Active API keys
    pub static ref ACTIVE_API_KEYS: IntGauge = IntGauge::new(
        "active_api_keys_total", "Total active (non-revoked) API keys"
    ).unwrap();

    // API key creations
    pub static ref API_KEY_CREATIONS_TOTAL: Counter = Counter::new(
        "api_key_creations_total", "Total API keys created"
    ).unwrap();

    // API key revocations
    pub static ref API_KEY_REVOCATIONS_TOTAL: Counter = Counter::new(
        "api_key_revocations_total", "Total API keys revoked"
    ).unwrap();

    // JWT token issuances
    pub static ref JWT_TOKENS_ISSUED_TOTAL: Counter = Counter::new(
        "jwt_tokens_issued_total", "Total JWT tokens issued"
    ).unwrap();

    // Permission check failures
    pub static ref PERMISSION_DENIED_TOTAL: CounterVec = CounterVec::new(
        Opts::new("permission_denied_total", "Total permission check failures"),
        &["action", "role"]
    ).unwrap();

    // Login attempts
    pub static ref LOGIN_ATTEMPTS_TOTAL: CounterVec = CounterVec::new(
        Opts::new("login_attempts_total", "Total login attempts"),
        &["result"]  // result: success|failure
    ).unwrap();

    // API key cache hit rate
    pub static ref API_KEY_CACHE_HITS: Counter = Counter::new(
        "api_key_cache_hits_total", "Total API key cache hits"
    ).unwrap();

    pub static ref API_KEY_CACHE_MISSES: Counter = Counter::new(
        "api_key_cache_misses_total", "Total API key cache misses"
    ).unwrap();
}

pub fn register_metrics(registry: &Registry) {
    registry.register(Box::new(AUTH_REQUESTS_TOTAL.clone())).unwrap();
    registry.register(Box::new(AUTH_FAILURES_TOTAL.clone())).unwrap();
    registry.register(Box::new(AUTH_DURATION_SECONDS.clone())).unwrap();
    registry.register(Box::new(ACTIVE_API_KEYS.clone())).unwrap();
    registry.register(Box::new(API_KEY_CREATIONS_TOTAL.clone())).unwrap();
    registry.register(Box::new(API_KEY_REVOCATIONS_TOTAL.clone())).unwrap();
    registry.register(Box::new(JWT_TOKENS_ISSUED_TOTAL.clone())).unwrap();
    registry.register(Box::new(PERMISSION_DENIED_TOTAL.clone())).unwrap();
    registry.register(Box::new(LOGIN_ATTEMPTS_TOTAL.clone())).unwrap();
    registry.register(Box::new(API_KEY_CACHE_HITS.clone())).unwrap();
    registry.register(Box::new(API_KEY_CACHE_MISSES.clone())).unwrap();
}
```

**Instrumentation in Middleware:**

```rust
// In auth_middleware:

pub async fn auth_middleware(
    State(service): State<Arc<CollectionService>>,
    mut request: Request,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    let start = Instant::now();

    // Skip authentication for public endpoints
    let path = request.uri().path();
    if is_public_endpoint(path) {
        return Ok(next.run(request).await);
    }

    // Extract Authorization header
    let auth_header = request.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok());

    if auth_header.is_none() {
        AUTH_FAILURES_TOTAL.with_label_values(&["unknown", "missing"]).inc();
        return Err((StatusCode::UNAUTHORIZED, "Missing Authorization header".to_string()));
    }

    let auth_header = auth_header.unwrap();
    if !auth_header.starts_with("Bearer ") {
        AUTH_FAILURES_TOTAL.with_label_values(&["unknown", "invalid_format"]).inc();
        return Err((StatusCode::UNAUTHORIZED, "Invalid Authorization header format".to_string()));
    }

    let token = &auth_header[7..];

    // Determine auth method
    let method = if token.starts_with("ak_") { "api_key" } else { "jwt" };

    // Validate token
    let auth_result = if token.starts_with("ak_") {
        service.validate_api_key(token).await
    } else {
        service.validate_jwt(token).await
    };

    // Record metrics
    let duration = start.elapsed();
    AUTH_DURATION_SECONDS.with_label_values(&[method]).observe(duration.as_secs_f64());

    match auth_result {
        Ok(auth_context) => {
            AUTH_REQUESTS_TOTAL.with_label_values(&[method, "success"]).inc();
            request.extensions_mut().insert(auth_context);
            Ok(next.run(request).await)
        }
        Err(e) => {
            let reason = classify_error(&e);
            AUTH_REQUESTS_TOTAL.with_label_values(&[method, "failure"]).inc();
            AUTH_FAILURES_TOTAL.with_label_values(&[method, reason]).inc();
            Err((StatusCode::UNAUTHORIZED, e.to_string()))
        }
    }
}

fn classify_error(e: &CoreError) -> &'static str {
    // Classify error for metrics
    match e {
        CoreError::Unauthorized { message } if message.contains("expired") => "expired",
        CoreError::Unauthorized { message } if message.contains("revoked") => "revoked",
        CoreError::Unauthorized { message } if message.contains("invalid") => "invalid",
        _ => "other",
    }
}
```

### OpenTelemetry Distributed Tracing

```rust
// File: crates/akidb-rest/src/tracing/auth.rs (NEW FILE)

use tracing::{info, warn, Span, instrument};
use opentelemetry::trace::{TraceContextExt, Tracer};

#[instrument(
    name = "authenticate_request",
    skip(service, token),
    fields(
        auth.method = %if token.starts_with("ak_") { "api_key" } else { "jwt" },
        auth.tenant_id = tracing::field::Empty,
        auth.user_id = tracing::field::Empty,
        auth.result = tracing::field::Empty,
    )
)]
pub async fn authenticate_with_tracing(
    service: &CollectionService,
    token: &str,
) -> CoreResult<AuthContext> {
    let span = Span::current();

    let result = if token.starts_with("ak_") {
        span.record("auth.method", "api_key");
        service.validate_api_key(token).await
    } else {
        span.record("auth.method", "jwt");
        service.validate_jwt(token).await
    };

    match &result {
        Ok(auth_context) => {
            span.record("auth.tenant_id", &auth_context.tenant_id.to_string());
            if let Some(user_id) = &auth_context.user_id {
                span.record("auth.user_id", &user_id.to_string());
            }
            span.record("auth.result", "success");
            info!("Authentication successful");
        }
        Err(e) => {
            span.record("auth.result", "failure");
            warn!("Authentication failed: {}", e);
        }
    }

    result
}
```

### Grafana Dashboard

**Authentication Dashboard JSON:**

```json
{
  "dashboard": {
    "title": "AkiDB Authentication",
    "panels": [
      {
        "title": "Authentication Requests (Rate)",
        "targets": [
          {
            "expr": "rate(auth_requests_total[5m])",
            "legendFormat": "{{method}} - {{result}}"
          }
        ],
        "type": "graph"
      },
      {
        "title": "Authentication Failures by Reason",
        "targets": [
          {
            "expr": "sum by(reason) (rate(auth_failures_total[5m]))",
            "legendFormat": "{{reason}}"
          }
        ],
        "type": "graph"
      },
      {
        "title": "Authentication Latency (P50/P95/P99)",
        "targets": [
          {
            "expr": "histogram_quantile(0.50, rate(auth_duration_seconds_bucket[5m]))",
            "legendFormat": "P50"
          },
          {
            "expr": "histogram_quantile(0.95, rate(auth_duration_seconds_bucket[5m]))",
            "legendFormat": "P95"
          },
          {
            "expr": "histogram_quantile(0.99, rate(auth_duration_seconds_bucket[5m]))",
            "legendFormat": "P99"
          }
        ],
        "type": "graph"
      },
      {
        "title": "Active API Keys",
        "targets": [
          {
            "expr": "active_api_keys_total"
          }
        ],
        "type": "singlestat"
      },
      {
        "title": "Permission Denials by Action",
        "targets": [
          {
            "expr": "sum by(action) (rate(permission_denied_total[5m]))",
            "legendFormat": "{{action}}"
          }
        ],
        "type": "graph"
      },
      {
        "title": "Login Success Rate",
        "targets": [
          {
            "expr": "rate(login_attempts_total{result=\"success\"}[5m]) / rate(login_attempts_total[5m])",
            "legendFormat": "Success Rate"
          }
        ],
        "type": "graph"
      },
      {
        "title": "API Key Cache Hit Rate",
        "targets": [
          {
            "expr": "rate(api_key_cache_hits_total[5m]) / (rate(api_key_cache_hits_total[5m]) + rate(api_key_cache_misses_total[5m]))",
            "legendFormat": "Cache Hit Rate"
          }
        ],
        "type": "graph"
      }
    ]
  }
}
```

---

## Security Hardening

### Audit Logging

**Log Every Auth Event:**

```rust
// File: crates/akidb-service/src/audit.rs

pub async fn log_auth_event(
    audit_repo: &impl AuditRepository,
    event: AuthEvent,
) -> CoreResult<()> {
    let audit_log = AuditLogEntry {
        audit_log_id: Uuid::now_v7(),
        tenant_id: event.tenant_id,
        user_id: event.user_id,
        action: event.action,
        resource_type: "authentication".to_string(),
        resource_id: event.resource_id,
        result: event.result,
        reason: event.reason,
        metadata: serde_json::to_value(&event.metadata).ok(),
        ip_address: event.ip_address,
        user_agent: event.user_agent,
        created_at: Utc::now(),
    };

    audit_repo.create(&audit_log).await
}

pub struct AuthEvent {
    pub tenant_id: TenantId,
    pub user_id: Option<UserId>,
    pub action: String,  // "api_key::create", "jwt::issue", "auth::validate"
    pub resource_id: String,
    pub result: AuditResult,  // Allowed | Denied
    pub reason: Option<String>,
    pub metadata: serde_json::Value,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}
```

**Integration with Middleware:**

```rust
// In auth_middleware:

match auth_result {
    Ok(auth_context) => {
        // Log successful authentication
        log_auth_event(&service.audit_repo, AuthEvent {
            tenant_id: auth_context.tenant_id,
            user_id: auth_context.user_id,
            action: "auth::validate".to_string(),
            resource_id: "authentication".to_string(),
            result: AuditResult::Allowed,
            reason: None,
            metadata: json!({
                "method": method,
                "ip": extract_ip(&request),
            }),
            ip_address: extract_ip(&request),
            user_agent: extract_user_agent(&request),
        }).await.ok();  // Don't block request on audit failure

        Ok(auth_context)
    }
    Err(e) => {
        // Log failed authentication
        log_auth_event(&service.audit_repo, AuthEvent {
            tenant_id: TenantId::nil(),  // Unknown tenant
            user_id: None,
            action: "auth::validate".to_string(),
            resource_id: "authentication".to_string(),
            result: AuditResult::Denied,
            reason: Some(e.to_string()),
            metadata: json!({
                "method": method,
                "error": e.to_string(),
            }),
            ip_address: extract_ip(&request),
            user_agent: extract_user_agent(&request),
        }).await.ok();

        Err(e)
    }
}
```

### IP Address & User Agent Extraction

```rust
// File: crates/akidb-rest/src/utils/request.rs (NEW FILE)

pub fn extract_ip(request: &Request) -> Option<String> {
    // Try X-Forwarded-For header (behind proxy)
    if let Some(forwarded) = request.headers().get("X-Forwarded-For") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            // First IP in X-Forwarded-For chain
            return Some(forwarded_str.split(',').next().unwrap().trim().to_string());
        }
    }

    // Try X-Real-IP header (Nginx)
    if let Some(real_ip) = request.headers().get("X-Real-IP") {
        if let Ok(ip_str) = real_ip.to_str() {
            return Some(ip_str.to_string());
        }
    }

    // Fallback: connection remote address
    // (Not available in Axum middleware, would need tower layer)
    None
}

pub fn extract_user_agent(request: &Request) -> Option<String> {
    request.headers()
        .get("User-Agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}
```

---

## Multi-Tenant Isolation

### Tenant Boundary Tests

**Test Categories:**

1. **API Key Isolation** - Tenant A cannot use Tenant B's API key
2. **JWT Isolation** - Tenant A user cannot access Tenant B resources
3. **Admin Isolation** - Tenant A admin cannot manage Tenant B's keys
4. **Cross-Tenant Queries** - Tenant A cannot query Tenant B's collections
5. **Data Leakage** - No information leakage across tenant boundaries

**Test Implementation:**

```rust
// File: crates/akidb-rest/tests/multi_tenant_isolation_test.rs (NEW FILE)

#[tokio::test]
async fn test_api_key_tenant_isolation() {
    let service = setup_test_service().await;

    // Create Tenant A and Tenant B
    let tenant_a = create_test_tenant(&service, "tenant-a").await;
    let tenant_b = create_test_tenant(&service, "tenant-b").await;

    // Create API key for Tenant A
    let api_key_a = create_test_api_key(&service, tenant_a.tenant_id, "key-a", vec!["collection::read"]).await;

    // Attempt to use Tenant A's API key to access Tenant B's resources
    let result = service.list_collections_with_auth(tenant_b.tenant_id, &api_key_a.plaintext_key).await;

    // Should fail: API key is scoped to Tenant A
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "Forbidden: Tenant mismatch");
}

#[tokio::test]
async fn test_jwt_tenant_isolation() {
    let service = setup_test_service().await;

    let tenant_a = create_test_tenant(&service, "tenant-a").await;
    let tenant_b = create_test_tenant(&service, "tenant-b").await;

    // Create user in Tenant A
    let user_a = create_test_user(&service, tenant_a.tenant_id, "user-a@example.com").await;

    // Login as Tenant A user → JWT token
    let jwt_a = service.login(&user_a.email, "password").await.unwrap();

    // Attempt to access Tenant B's resources with Tenant A's JWT
    let result = service.list_collections_with_jwt(tenant_b.tenant_id, &jwt_a).await;

    // Should fail: JWT is scoped to Tenant A
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "Forbidden: Tenant mismatch");
}

#[tokio::test]
async fn test_admin_key_management_isolation() {
    let service = setup_test_service().await;

    let tenant_a = create_test_tenant(&service, "tenant-a").await;
    let tenant_b = create_test_tenant(&service, "tenant-b").await;

    // Create admin user in Tenant A
    let admin_a = create_test_admin(&service, tenant_a.tenant_id).await;
    let admin_jwt_a = service.login(&admin_a.email, "password").await.unwrap();

    // Create API key for Tenant B
    let api_key_b = create_test_api_key(&service, tenant_b.tenant_id, "key-b", vec!["collection::read"]).await;

    // Attempt to revoke Tenant B's API key using Tenant A admin JWT
    let result = service.revoke_api_key_with_jwt(api_key_b.key_id, &admin_jwt_a).await;

    // Should fail: Admin can only manage their own tenant's keys
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "Forbidden: Cannot manage other tenant's API keys");
}

#[tokio::test]
async fn test_collection_query_tenant_isolation() {
    let service = setup_test_service().await;

    let tenant_a = create_test_tenant(&service, "tenant-a").await;
    let tenant_b = create_test_tenant(&service, "tenant-b").await;

    // Create collection in Tenant A
    let collection_a = create_test_collection(&service, tenant_a.tenant_id, "collection-a").await;

    // Create API key for Tenant B
    let api_key_b = create_test_api_key(&service, tenant_b.tenant_id, "key-b", vec!["collection::query"]).await;

    // Attempt to query Tenant A's collection using Tenant B's API key
    let query_vector = vec![0.1; 128];
    let result = service.query_collection_with_auth(collection_a.collection_id, &query_vector, 10, &api_key_b.plaintext_key).await;

    // Should fail: Cannot query other tenant's collections
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "Forbidden: Tenant mismatch");
}

#[tokio::test]
async fn test_list_api_keys_tenant_isolation() {
    let service = setup_test_service().await;

    let tenant_a = create_test_tenant(&service, "tenant-a").await;
    let tenant_b = create_test_tenant(&service, "tenant-b").await;

    // Create 3 API keys for Tenant A
    create_test_api_key(&service, tenant_a.tenant_id, "key-a-1", vec!["collection::read"]).await;
    create_test_api_key(&service, tenant_a.tenant_id, "key-a-2", vec!["collection::write"]).await;
    create_test_api_key(&service, tenant_a.tenant_id, "key-a-3", vec!["collection::delete"]).await;

    // Create 2 API keys for Tenant B
    create_test_api_key(&service, tenant_b.tenant_id, "key-b-1", vec!["collection::read"]).await;
    create_test_api_key(&service, tenant_b.tenant_id, "key-b-2", vec!["collection::write"]).await;

    // Login as Tenant A admin
    let admin_a = create_test_admin(&service, tenant_a.tenant_id).await;
    let admin_jwt_a = service.login(&admin_a.email, "password").await.unwrap();

    // List API keys with Tenant A admin JWT
    let keys = service.list_api_keys_with_jwt(&admin_jwt_a).await.unwrap();

    // Should only see Tenant A's keys (3 keys)
    assert_eq!(keys.len(), 3);
    assert!(keys.iter().all(|k| k.tenant_id == tenant_a.tenant_id));
}

#[tokio::test]
async fn test_error_messages_no_tenant_leakage() {
    let service = setup_test_service().await;

    let tenant_a = create_test_tenant(&service, "tenant-a").await;
    let tenant_b = create_test_tenant(&service, "tenant-b").await;

    // Create collection in Tenant B
    let collection_b = create_test_collection(&service, tenant_b.tenant_id, "secret-collection").await;

    // Create API key for Tenant A
    let api_key_a = create_test_api_key(&service, tenant_a.tenant_id, "key-a", vec!["collection::read"]).await;

    // Attempt to access Tenant B's collection
    let result = service.get_collection_with_auth(collection_b.collection_id, &api_key_a.plaintext_key).await;

    // Should fail with generic error (no collection name/details leaked)
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(!error_msg.contains("secret-collection"));  // No collection name leaked
    assert!(!error_msg.contains(&tenant_b.tenant_id.to_string()));  // No tenant ID leaked
    assert_eq!(error_msg, "Forbidden: Access denied");  // Generic message
}

#[tokio::test]
async fn test_timing_attack_resistance() {
    let service = setup_test_service().await;

    let tenant_a = create_test_tenant(&service, "tenant-a").await;

    // Create API key
    let api_key = create_test_api_key(&service, tenant_a.tenant_id, "key-a", vec!["collection::read"]).await;

    // Measure time for valid key
    let start = Instant::now();
    let _ = service.validate_api_key(&api_key.plaintext_key).await;
    let valid_duration = start.elapsed();

    // Measure time for invalid key (wrong hash)
    let invalid_key = "ak_0000000000000000000000000000000000000000000000000000000000000000";
    let start = Instant::now();
    let _ = service.validate_api_key(invalid_key).await;
    let invalid_duration = start.elapsed();

    // Timing should be similar (constant-time comparison)
    // Allow 10% variance for normal timing jitter
    let ratio = valid_duration.as_micros() as f64 / invalid_duration.as_micros() as f64;
    assert!((0.9..=1.1).contains(&ratio), "Timing attack vulnerability detected: ratio={}", ratio);
}

#[tokio::test]
async fn test_permission_escalation_prevention() {
    let service = setup_test_service().await;

    let tenant_a = create_test_tenant(&service, "tenant-a").await;

    // Create API key with read-only permissions
    let readonly_key = create_test_api_key(&service, tenant_a.tenant_id, "readonly", vec!["collection::read"]).await;

    // Attempt to create API key with admin permissions using readonly key
    // (This should fail at permission check, not just at database level)
    let result = service.create_api_key_with_auth(
        &readonly_key.plaintext_key,
        "new-admin-key",
        vec!["*"],
    ).await;

    // Should fail: Readonly key cannot create API keys
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "Forbidden: Missing required permission: admin");
}

#[tokio::test]
async fn test_revoked_key_immediate_effect() {
    let service = setup_test_service().await;

    let tenant_a = create_test_tenant(&service, "tenant-a").await;

    // Create API key
    let api_key = create_test_api_key(&service, tenant_a.tenant_id, "key-a", vec!["collection::read"]).await;

    // Use key (should work)
    let result1 = service.validate_api_key(&api_key.plaintext_key).await;
    assert!(result1.is_ok());

    // Revoke key
    service.revoke_api_key(api_key.key_id).await.unwrap();

    // Use key again (should fail immediately, no caching)
    let result2 = service.validate_api_key(&api_key.plaintext_key).await;
    assert!(result2.is_err());
    assert_eq!(result2.unwrap_err().to_string(), "Unauthorized: API key has been revoked");
}
```

---

## gRPC Authentication

### gRPC Client Examples

**Python gRPC Client:**

```python
# File: examples/python/grpc_auth_example.py

import grpc
from akidb.collection.v1 import collection_pb2, collection_pb2_grpc

def main():
    # API Key authentication
    api_key = "ak_f8d7c6b5a4e3d2c1b0a9f8e7d6c5b4a3f2e1d0c9b8a7f6e5d4c3b2a1f0e9d8c7"

    # Create gRPC channel with TLS
    credentials = grpc.ssl_channel_credentials()
    channel = grpc.secure_channel('akidb.example.com:9000', credentials)

    # Create stub
    stub = collection_pb2_grpc.CollectionServiceStub(channel)

    # Create metadata with Bearer token
    metadata = [('authorization', f'Bearer {api_key}')]

    # Make authenticated request
    try:
        response = stub.ListCollections(
            collection_pb2.ListCollectionsRequest(),
            metadata=metadata
        )

        print(f"Collections: {response.collections}")
    except grpc.RpcError as e:
        if e.code() == grpc.StatusCode.UNAUTHENTICATED:
            print(f"Authentication failed: {e.details()}")
        else:
            print(f"RPC failed: {e}")

if __name__ == '__main__':
    main()
```

**Rust gRPC Client:**

```rust
// File: examples/rust/grpc_auth_example.rs

use tonic::transport::{Channel, ClientTlsConfig};
use tonic::metadata::MetadataValue;
use tonic::Request;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = "ak_f8d7c6b5a4e3d2c1b0a9f8e7d6c5b4a3f2e1d0c9b8a7f6e5d4c3b2a1f0e9d8c7";

    // Create TLS config
    let tls = ClientTlsConfig::new()
        .domain_name("akidb.example.com");

    // Create channel
    let channel = Channel::from_static("https://akidb.example.com:9000")
        .tls_config(tls)?
        .connect()
        .await?;

    // Create client
    let mut client = CollectionServiceClient::new(channel);

    // Create request with auth metadata
    let mut request = Request::new(ListCollectionsRequest {});

    let bearer_token = format!("Bearer {}", api_key);
    let token_value = MetadataValue::from_str(&bearer_token)?;
    request.metadata_mut().insert("authorization", token_value);

    // Make authenticated request
    let response = client.list_collections(request).await?;

    println!("Collections: {:?}", response.into_inner().collections);

    Ok(())
}
```

**grpcurl Example:**

```bash
# API Key authentication
grpcurl \
  -H "authorization: Bearer ak_f8d7c6b5a4e3d2c1b0a9f8e7d6c5b4a3f2e1d0c9b8a7f6e5d4c3b2a1f0e9d8c7" \
  akidb.example.com:9000 \
  akidb.collection.v1.CollectionService/ListCollections

# JWT authentication
grpcurl \
  -H "authorization: Bearer eyJhbGciOiJIUzI1NiIs..." \
  akidb.example.com:9000 \
  akidb.collection.v1.CollectionService/ListCollections
```

---

## Performance Optimization

### In-Memory API Key Cache

**LRU Cache Implementation:**

```rust
// File: crates/akidb-service/src/cache/api_key_cache.rs (NEW FILE)

use lru::LruCache;
use std::sync::Arc;
use parking_lot::RwLock;
use std::time::{Duration, Instant};

pub struct ApiKeyCache {
    cache: Arc<RwLock<LruCache<String, CachedApiKey>>>,
    ttl: Duration,
}

struct CachedApiKey {
    api_key: ApiKey,
    cached_at: Instant,
}

impl ApiKeyCache {
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        Self {
            cache: Arc::new(RwLock::new(LruCache::new(capacity))),
            ttl,
        }
    }

    pub fn get(&self, key_hash: &str) -> Option<ApiKey> {
        let mut cache = self.cache.write();

        if let Some(cached) = cache.get(key_hash) {
            // Check TTL
            if cached.cached_at.elapsed() < self.ttl {
                // Cache hit
                return Some(cached.api_key.clone());
            } else {
                // Expired, remove
                cache.pop(key_hash);
            }
        }

        None
    }

    pub fn put(&self, key_hash: String, api_key: ApiKey) {
        let mut cache = self.cache.write();
        cache.put(key_hash, CachedApiKey {
            api_key,
            cached_at: Instant::now(),
        });
    }

    pub fn invalidate(&self, key_hash: &str) {
        let mut cache = self.cache.write();
        cache.pop(key_hash);
    }

    pub fn clear(&self) {
        let mut cache = self.cache.write();
        cache.clear();
    }
}
```

**Integration with Validation:**

```rust
impl CollectionService {
    pub async fn validate_api_key(&self, plaintext_key: &str) -> CoreResult<AuthContext> {
        // Hash the key
        let mut hasher = Sha256::new();
        hasher.update(plaintext_key.as_bytes());
        let key_hash = format!("{:x}", hasher.finalize());

        // Check cache first
        if let Some(api_key) = self.api_key_cache.get(&key_hash) {
            API_KEY_CACHE_HITS.inc();

            // Still need to check expiration and revocation
            if api_key.is_valid() {
                return Ok(api_key.to_auth_context());
            } else {
                // Invalid, invalidate cache
                self.api_key_cache.invalidate(&key_hash);
            }
        }

        // Cache miss, lookup from database
        API_KEY_CACHE_MISSES.inc();

        let api_key = self.api_key_repository.find_by_hash(&key_hash).await?;

        // Validate
        if api_key.is_revoked {
            return Err(CoreError::unauthorized("API key has been revoked"));
        }

        if api_key.is_expired() {
            return Err(CoreError::unauthorized("API key has expired"));
        }

        // Cache for future requests
        self.api_key_cache.put(key_hash, api_key.clone());

        Ok(api_key.to_auth_context())
    }
}
```

**Cache Invalidation:**

```rust
impl CollectionService {
    pub async fn revoke_api_key(&self, key_id: Uuid) -> CoreResult<()> {
        // Revoke in database
        let api_key = self.api_key_repository.revoke(key_id).await?;

        // Invalidate cache immediately
        let mut hasher = Sha256::new();
        hasher.update(api_key.plaintext_key.as_bytes());  // We don't have plaintext!
        // Problem: We don't have plaintext key to compute hash

        // Solution: Clear entire cache (simple but effective)
        self.api_key_cache.clear();

        // Or: Store key_hash in ApiKey model for cache invalidation
        // self.api_key_cache.invalidate(&api_key.key_hash);

        Ok(())
    }
}
```

**Configuration:**

```toml
[authentication]
api_key_cache_size = 1000      # Max cached API keys
api_key_cache_ttl_seconds = 300  # 5 minutes
```

---

## Day-by-Day Action Plan

### Day 6: Permission Mapping (8 hours)

**Morning (4 hours): Permission System**

**Task 6.1: Implement Action Enum (1.5 hours)**
```rust
// File: crates/akidb-core/src/permissions.rs (NEW FILE)
// Implement Action enum with all 17 action types
// Implement Role enum with permission mapping
// Unit tests (5 tests)
```

**Task 6.2: Implement PermissionChecker (1.5 hours)**
```rust
// Implement check(), check_any(), check_all()
// Implement wildcard support ("*", "collection::*")
// Unit tests (6 tests)
```

**Task 6.3: Integrate with Handlers (1 hour)**
```rust
// Update all handler functions to use PermissionChecker
// Add permission checks before processing requests
```

**Afternoon (4 hours): Testing + Metrics**

**Task 6.4: Permission Tests (2 hours)**
```rust
// Test each action type
// Test role permissions
// Test wildcards
// Test permission denials
// Total: 10 tests
```

**Task 6.5: Permission Denied Metrics (1 hour)**
```rust
// Add PERMISSION_DENIED_TOTAL metric
// Instrument PermissionChecker
// Test metrics recording
```

**Task 6.6: Documentation (1 hour)**
```markdown
# Update API-TUTORIAL.md
# Document all 17 action types
# Document wildcard permissions
# Example: API key with specific permissions
```

**Day 6 Deliverables:**
- ✅ Permission system complete (17 actions)
- ✅ PermissionChecker with wildcard support
- ✅ All handlers use permission checks
- ✅ 21 tests (11 unit + 10 integration)
- ✅ Permission metrics

---

### Day 7: Observability (8 hours)

**Morning (4 hours): Prometheus Metrics**

**Task 7.1: Define Auth Metrics (1 hour)**
```rust
// File: crates/akidb-rest/src/metrics/auth.rs (NEW FILE)
// Define 11 Prometheus metrics
// Register metrics with global registry
```

**Task 7.2: Instrument Middleware (1.5 hours)**
```rust
// Add metrics to auth_middleware
// Track request duration
// Track success/failure by method
// Track failure reasons
```

**Task 7.3: Instrument Endpoints (1.5 hours)**
```rust
// Add metrics to create_api_key
// Add metrics to login
// Add metrics to permission checks
```

**Afternoon (4 hours): Tracing + Dashboard**

**Task 7.4: OpenTelemetry Tracing (1.5 hours)**
```rust
// File: crates/akidb-rest/src/tracing/auth.rs (NEW FILE)
// Add spans for authentication
// Record auth method, tenant, user
// Integration tests
```

**Task 7.5: Grafana Dashboard (2 hours)**
```json
// Create monitoring/grafana/dashboards/auth.json
// 7 panels: requests, failures, latency, active keys, denials, login rate, cache hit rate
// Test dashboard with Prometheus data
```

**Task 7.6: Alert Rules (30 min)**
```yaml
// Create monitoring/prometheus/auth-alerts.yaml
// Alerts: HighAuthFailureRate, SlowAuth, HighPermissionDenials
```

**Day 7 Deliverables:**
- ✅ 11 Prometheus metrics
- ✅ OpenTelemetry tracing
- ✅ Grafana auth dashboard
- ✅ 3 Prometheus alert rules
- ✅ Metrics tested

---

### Day 8: Security Hardening (8 hours)

**Morning (4 hours): Audit Logging**

**Task 8.1: Audit Event Structure (1 hour)**
```rust
// Define AuthEvent struct
// Implement log_auth_event()
// Integration with AuditRepository
```

**Task 8.2: Audit All Auth Events (2 hours)**
```rust
// Log API key validation (success + failure)
// Log JWT validation (success + failure)
// Log API key creation
// Log API key revocation
// Log login attempts
// Log permission denials
```

**Task 8.3: IP & User Agent Extraction (1 hour)**
```rust
// File: crates/akidb-rest/src/utils/request.rs (NEW FILE)
// extract_ip() with X-Forwarded-For support
// extract_user_agent()
// Tests
```

**Afternoon (4 hours): Multi-Tenant Isolation Tests**

**Task 8.4: Tenant Isolation Tests (3 hours)**
```rust
// File: crates/akidb-rest/tests/multi_tenant_isolation_test.rs (NEW FILE)
// 10 tests (see "Multi-Tenant Isolation" section)
// All tests passing
```

**Task 8.5: Security Review (1 hour)**
```bash
# cargo-audit scan
# Review timing attack prevention
# Review error message leakage
# Document security considerations
```

**Day 8 Deliverables:**
- ✅ Audit logging for all auth events
- ✅ IP & user agent extraction
- ✅ 10 multi-tenant isolation tests
- ✅ Security review complete

---

### Day 9: gRPC Authentication (8 hours)

**Morning (4 hours): gRPC Client Examples**

**Task 9.1: Python gRPC Client (1.5 hours)**
```python
# File: examples/python/grpc_auth_example.py
# API key authentication example
# JWT authentication example
# Error handling
# README with setup instructions
```

**Task 9.2: Rust gRPC Client (1.5 hours)**
```rust
// File: examples/rust/grpc_auth_example.rs
// API key authentication example
// JWT authentication example
// Error handling
// Cargo.toml with dependencies
```

**Task 9.3: grpcurl Examples (1 hour)**
```markdown
# File: docs/GRPC-EXAMPLES.md
# grpcurl authentication examples
# Health check
# List collections
# Query vectors
# Error handling
```

**Afternoon (4 hours): gRPC Testing + Performance**

**Task 9.4: gRPC E2E Tests (2 hours)**
```rust
// File: crates/akidb-grpc/tests/auth_test.rs (NEW FILE)
// test_grpc_api_key_auth
// test_grpc_jwt_auth
// test_grpc_unauthenticated
// test_grpc_permission_denied
// 4 tests
```

**Task 9.5: Performance Optimization (1 hour)**
```rust
// Benchmark auth overhead
// Optimize middleware
// Add caching
// Measure improvement
```

**Task 9.6: Documentation (1 hour)**
```markdown
# Update DEPLOYMENT-GUIDE.md (gRPC auth setup)
# Update API-TUTORIAL.md (gRPC examples)
# Create GRPC-EXAMPLES.md
```

**Day 9 Deliverables:**
- ✅ Python gRPC client example
- ✅ Rust gRPC client example
- ✅ grpcurl examples
- ✅ 4 gRPC E2E tests
- ✅ Performance optimized
- ✅ Documentation complete

---

### Day 10: Performance + Validation (8 hours)

**Morning (4 hours): Performance Optimization**

**Task 10.1: Implement API Key Cache (2 hours)**
```rust
// File: crates/akidb-service/src/cache/api_key_cache.rs (NEW FILE)
// LRU cache with TTL
// Integration with validate_api_key()
// Cache invalidation on revocation
```

**Task 10.2: Performance Testing (1 hour)**
```bash
# Benchmark auth with/without cache
# Measure P50/P95/P99 latency
# Verify <2ms overhead target
# Document results
```

**Task 10.3: Cache Metrics (1 hour)**
```rust
# Add cache hit/miss metrics
# Test metrics
# Update Grafana dashboard
```

**Afternoon (4 hours): Final Validation + Docs**

**Task 10.4: Run All Tests (1 hour)**
```bash
cargo test --workspace
# Expected: 195+ tests (175 Week 1 + 20 Week 2)
```

**Task 10.5: Fix Any Issues (1 hour)**
```bash
# Debug test failures
# Fix compilation errors
# Address clippy warnings
```

**Task 10.6: Security Audit (1 hour)**
```bash
cargo audit
# Check for vulnerabilities
# Update dependencies if needed
```

**Task 10.7: Documentation (1 hour)**
```markdown
# Update CHANGELOG.md
# Create Week 2 completion report
# Update API-TUTORIAL.md
# Update SECURITY.md
```

**Day 10 Deliverables:**
- ✅ API key cache implemented
- ✅ Performance benchmarks (auth <2ms)
- ✅ All tests passing (195+)
- ✅ Security audit complete
- ✅ Week 2 completion report

---

## Testing Strategy

### Unit Tests (Target: 22 tests)

**Permission Tests (11 tests):**
- test_action_enum_all_variants
- test_role_admin_permissions
- test_role_developer_permissions
- test_role_viewer_permissions
- test_role_auditor_permissions
- test_permission_checker_exact_match
- test_permission_checker_wildcard_all
- test_permission_checker_wildcard_category
- test_permission_checker_denied
- test_permission_checker_any
- test_permission_checker_all

**Cache Tests (5 tests):**
- test_cache_hit
- test_cache_miss
- test_cache_ttl_expiration
- test_cache_invalidation
- test_cache_capacity_limit

**Audit Tests (3 tests):**
- test_log_auth_success
- test_log_auth_failure
- test_extract_ip_from_headers

**Metrics Tests (3 tests):**
- test_metrics_registered
- test_auth_request_metric
- test_permission_denied_metric

### Integration Tests (Target: 15 tests)

**Multi-Tenant Isolation (10 tests):**
- test_api_key_tenant_isolation
- test_jwt_tenant_isolation
- test_admin_key_management_isolation
- test_collection_query_tenant_isolation
- test_list_api_keys_tenant_isolation
- test_error_messages_no_tenant_leakage
- test_timing_attack_resistance
- test_permission_escalation_prevention
- test_revoked_key_immediate_effect
- test_cross_tenant_admin_blocked

**gRPC Authentication (4 tests):**
- test_grpc_api_key_auth
- test_grpc_jwt_auth
- test_grpc_unauthenticated
- test_grpc_permission_denied

**Performance (1 test):**
- test_auth_performance_with_cache

### E2E Tests (Target: 5 tests)

**End-to-End Flows (5 tests):**
- test_e2e_full_auth_flow_api_key
- test_e2e_full_auth_flow_jwt
- test_e2e_permission_denied_workflow
- test_e2e_api_key_lifecycle
- test_e2e_audit_log_complete

**Total: 42 new tests (Week 1: 28, Week 2: 42, Total: 70)**

---

## Metrics & Monitoring

### Prometheus Metrics Summary

| Metric | Type | Labels | Purpose |
|--------|------|--------|---------|
| auth_requests_total | Counter | method, result | Total auth requests |
| auth_failures_total | Counter | method, reason | Auth failures by reason |
| auth_duration_seconds | Histogram | method | Auth latency |
| active_api_keys_total | Gauge | - | Active API keys count |
| api_key_creations_total | Counter | - | API keys created |
| api_key_revocations_total | Counter | - | API keys revoked |
| jwt_tokens_issued_total | Counter | - | JWT tokens issued |
| permission_denied_total | Counter | action, role | Permission denials |
| login_attempts_total | Counter | result | Login attempts |
| api_key_cache_hits_total | Counter | - | Cache hits |
| api_key_cache_misses_total | Counter | - | Cache misses |

### Alert Rules

**auth-alerts.yaml:**

```yaml
groups:
  - name: authentication
    rules:
      - alert: HighAuthFailureRate
        expr: rate(auth_failures_total[5m]) > 10
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High authentication failure rate"
          description: "Auth failure rate is {{ $value }} per second"

      - alert: SlowAuthentication
        expr: histogram_quantile(0.95, rate(auth_duration_seconds_bucket[5m])) > 0.005
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Slow authentication requests"
          description: "P95 auth latency is {{ $value }}s (target: 2ms)"

      - alert: HighPermissionDenials
        expr: rate(permission_denied_total[5m]) > 5
        for: 5m
        labels:
          severity: info
        annotations:
          summary: "High permission denial rate"
          description: "Permission denial rate is {{ $value }} per second"
```

---

## Risk Assessment

### High Risk

**Risk 1: Cache Invalidation Bugs**
- **Probability:** Medium (25%)
- **Impact:** HIGH (stale API keys used after revocation)
- **Scenario:** API key revoked but still in cache, continues to work
- **Mitigation:**
  - Clear entire cache on revocation (simple, effective)
  - Store key_hash in ApiKey model for targeted invalidation
  - Short TTL (5 minutes max)
- **Contingency:** Disable cache, fix bug, re-enable

**Risk 2: Permission Escalation via Wildcards**
- **Probability:** Low (10%)
- **Impact:** CRITICAL (unauthorized access)
- **Scenario:** Developer creates API key with "*" permission without authorization
- **Mitigation:**
  - Only admins can create API keys
  - Validate wildcard permissions in create endpoint
  - Audit log all API key creations
- **Contingency:** Revoke compromised keys, audit all keys

### Medium Risk

**Risk 3: Audit Log Performance Impact**
- **Probability:** Medium (30%)
- **Impact:** MEDIUM (slower requests)
- **Scenario:** Synchronous audit logging adds latency
- **Mitigation:**
  - Async audit logging (tokio::spawn)
  - Don't block request on audit failure
  - Batch audit logs (future optimization)
- **Contingency:** Disable audit logging temporarily

**Risk 4: Multi-Tenant Test False Positives**
- **Probability:** Medium (20%)
- **Impact:** MEDIUM (isolation bugs missed)
- **Scenario:** Tests pass but isolation is actually broken
- **Mitigation:**
  - Comprehensive test coverage (10+ tests)
  - Manual penetration testing
  - Production monitoring
- **Contingency:** Fix isolation bugs in production, hotfix

### Low Risk

**Risk 5: gRPC Client Example Bugs**
- **Probability:** Medium (25%)
- **Impact:** LOW (user confusion)
- **Scenario:** Examples don't work for users
- **Mitigation:**
  - Test examples manually
  - CI testing for examples
  - Clear documentation
- **Contingency:** Fix examples, update docs

---

## Success Metrics

### Functional Metrics

**MUST-HAVE:**
- ✅ All 17 RBAC actions supported
- ✅ Wildcard permissions working
- ✅ Multi-tenant isolation verified (10 tests)
- ✅ gRPC auth working (Python + Rust examples)
- ✅ Audit logging for all auth events
- ✅ API key caching working
- ✅ 195+ tests passing

### Performance Metrics

**Target:**
- Auth overhead with cache: <1ms P95
- Auth overhead without cache: <2ms P95
- Cache hit rate: >80%
- Permission check overhead: <0.1ms

**Measurement:**
```bash
cargo bench --bench auth_bench

# Expected:
# api_key_validation_cached     time: [500 µs 700 µs 900 µs]
# api_key_validation_uncached   time: [1.5 ms 1.8 ms 2.0 ms]
# permission_check              time: [50 µs 80 µs 100 µs]
```

### Observability Metrics

**Target:**
- 11 Prometheus metrics exposed
- Grafana dashboard with 7 panels
- 3 Prometheus alert rules
- OpenTelemetry spans for all auth requests

**Measurement:**
```bash
# Check metrics endpoint
curl http://localhost:8080/metrics | grep auth_

# Should see all 11 metrics
```

---

## Conclusion

Week 2 transforms Week 1's functional authentication into a **production-grade, enterprise-ready authentication system** with comprehensive observability, security hardening, and operational polish.

**Timeline:** 5 working days (40 hours)
**Effort:** Days 6-10
**Risk:** Medium (security and performance critical)
**Impact:** HIGH (production-ready authentication)

**Success Criteria:**
- ✅ All 17 RBAC permissions supported
- ✅ Complete observability (metrics, tracing, dashboards)
- ✅ Multi-tenant isolation verified
- ✅ gRPC authentication working
- ✅ Performance optimized (<2ms)
- ✅ 195+ tests passing

**Next Steps:** Week 3 - TLS & Security Hardening

---

**Document Version:** 1.0
**Last Updated:** 2025-11-08
**Status:** READY FOR IMPLEMENTATION ✅
