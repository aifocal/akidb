# Phase 8 Week 1: API Key Authentication - COMPREHENSIVE MEGATHINK

**Date:** 2025-11-08
**Status:** PLANNING
**Timeline:** 5 working days (Days 1-5)
**Dependencies:** Phase 7 Complete ✅
**Target:** Secure API access with API keys and JWT tokens

---

## Executive Summary

Week 1 implements **API key authentication** and **JWT token support** for AkiDB 2.0, transforming the open API into a secure, enterprise-ready authentication system.

**Current State:**
- Phase 7 complete with admin endpoints
- No authentication (anyone can access API)
- Cannot deploy to public internet
- Fails compliance requirements (SOC 2, HIPAA)

**Week 1 Deliverables:**
1. **API Key Generation** - Cryptographically secure 32-byte tokens
2. **API Key Storage** - SHA-256 hashed in SQLite
3. **Authentication Middleware** - REST + gRPC validation
4. **Admin Endpoints** - Create, revoke, list API keys
5. **JWT Token Support** - Login endpoint with HS256 signing
6. **Multi-Tenant Isolation** - Tenant-scoped API keys
7. **20+ Tests** - Unit + integration + E2E

**Success Criteria:**
- ✅ All API requests require authentication (401 if missing)
- ✅ API keys work for both REST and gRPC
- ✅ JWT tokens work for both REST and gRPC
- ✅ Tenant isolation enforced (no cross-tenant access)
- ✅ 20+ tests passing (authentication coverage)
- ✅ Documentation complete (API examples)

---

## Table of Contents

1. [Technical Requirements](#technical-requirements)
2. [Database Schema](#database-schema)
3. [API Key Design](#api-key-design)
4. [JWT Token Design](#jwt-token-design)
5. [Authentication Middleware](#authentication-middleware)
6. [Admin Endpoints](#admin-endpoints)
7. [Day-by-Day Action Plan](#day-by-day-action-plan)
8. [Testing Strategy](#testing-strategy)
9. [Code Structure](#code-structure)
10. [Risk Assessment](#risk-assessment)
11. [Success Metrics](#success-metrics)

---

## Technical Requirements

### Functional Requirements

**FR-1: API Key Authentication**
- API keys are 32-byte random tokens (cryptographically secure)
- Keys are hex-encoded (64 characters: `ak_` prefix + 62 hex chars)
- Keys stored as SHA-256 hashes in database (never plaintext)
- Keys scoped to single tenant (multi-tenant isolation)
- Keys have optional expiration date
- Keys have permissions array (maps to RBAC roles)
- Keys track last_used_at timestamp
- Invalid/expired keys return 401 Unauthorized

**FR-2: JWT Token Authentication**
- JWT tokens issued by `/auth/login` endpoint
- Tokens signed with HS256 (HMAC-SHA256)
- Secret key loaded from environment variable
- Default expiration: 24 hours (configurable)
- Token payload: tenant_id, user_id, email, role, exp, iat
- Invalid/expired tokens return 401 Unauthorized
- Optional refresh token support (7-day expiration)

**FR-3: Authentication Middleware**
- Extract `Authorization: Bearer <token>` header (REST)
- Extract `authorization` metadata (gRPC)
- Validate API key OR JWT token
- Load tenant + user context from token
- Inject `AuthContext` into request
- Public endpoints bypass authentication (health check)
- Failed authentication logged for audit

**FR-4: Admin Endpoints**
- `POST /admin/api-keys` - Create API key (admin only)
- `GET /admin/api-keys` - List API keys (filtered by tenant)
- `DELETE /admin/api-keys/{id}` - Revoke API key (admin only)
- `GET /admin/api-keys/{id}` - Get API key details (admin only)
- OpenAPI spec updated

**FR-5: Multi-Tenant Isolation**
- API keys scoped to single tenant
- Tenant A cannot list/revoke Tenant B's keys
- Tenant A cannot use Tenant B's keys
- Admin keys can manage all tenants (super-admin)

### Non-Functional Requirements

**NFR-1: Performance**
- API key validation: <1ms (in-memory cache after first lookup)
- JWT validation: <0.5ms (signature verification)
- Database lookup: <5ms (indexed by key_hash)
- Total auth overhead: <2ms per request

**NFR-2: Security**
- API keys generated with CSPRNG (cryptographically secure)
- SHA-256 hashing for key storage (bcrypt not needed, keys are random)
- JWT secret minimum 256 bits (32 bytes)
- JWT secret never logged or exposed
- Timing-safe comparison for key hashes

**NFR-3: Auditability**
- All authentication events logged (success + failure)
- Audit logs include: timestamp, tenant_id, user_id, action, result, ip_address
- Failed authentication attempts tracked (rate limiting future)

**NFR-4: Reliability**
- Zero downtime for key revocation (immediate effect)
- Graceful handling of expired keys
- Graceful handling of missing/malformed tokens

---

## Database Schema

### Migration: 006_api_keys_table.sql

```sql
-- API Keys Table
-- Stores hashed API keys with metadata
CREATE TABLE api_keys (
    key_id BLOB PRIMARY KEY,                    -- UUID v7
    tenant_id BLOB NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    key_hash TEXT NOT NULL UNIQUE,              -- SHA-256 hash (64 hex chars)
    name TEXT NOT NULL,                         -- Human-readable name (e.g., "production-service")
    permissions TEXT NOT NULL,                  -- JSON array: ["collection::read", "collection::write"]
    created_at TEXT NOT NULL,                   -- ISO-8601 timestamp
    expires_at TEXT,                            -- NULL = never expires
    last_used_at TEXT,                          -- Updated on every API call
    created_by BLOB REFERENCES users(user_id) ON DELETE SET NULL,
    is_revoked INTEGER NOT NULL DEFAULT 0,      -- 0=active, 1=revoked (soft delete)
    revoked_at TEXT,
    revoked_by BLOB REFERENCES users(user_id) ON DELETE SET NULL
) STRICT;

-- Indexes for performance
CREATE INDEX ix_api_keys_tenant ON api_keys(tenant_id);
CREATE INDEX ix_api_keys_hash ON api_keys(key_hash) WHERE is_revoked = 0;
CREATE INDEX ix_api_keys_created_by ON api_keys(created_by);

-- Ensure unique key names per tenant (UX improvement)
CREATE UNIQUE INDEX ux_api_keys_tenant_name ON api_keys(tenant_id, name) WHERE is_revoked = 0;
```

### Schema Design Rationale

**Why SHA-256 instead of bcrypt?**
- API keys are 32-byte random tokens (256 bits of entropy)
- bcrypt is for passwords (low entropy, needs slow hashing)
- SHA-256 is sufficient for high-entropy tokens
- Faster validation (<1ms vs ~100ms for bcrypt)

**Why soft delete (is_revoked)?**
- Preserve audit trail (who revoked, when)
- Can restore accidentally revoked keys
- Metrics: track total keys created vs active

**Why key_hash unique constraint?**
- Prevent duplicate keys (collision detection)
- 2^256 keyspace makes collisions impossible in practice
- Unique index speeds up lookups (O(log n) instead of O(n))

**Why last_used_at tracking?**
- Identify stale keys (security hygiene)
- Compliance: show key usage history
- Future: auto-expire unused keys

---

## API Key Design

### API Key Format

**Structure:**
```
ak_<62 hex characters>

Example:
ak_f8d7c6b5a4e3d2c1b0a9f8e7d6c5b4a3f2e1d0c9b8a7f6e5d4c3b2a1f0e9d8c7
│  └─────────────────────────────────────────────────────────────┘
│                          32 bytes (hex-encoded)
└─ Prefix (human-readable type indicator)
```

**Prefix Rationale:**
- `ak_` = "API Key" (human-readable)
- Prevents accidental paste into code/docs
- Easy to identify in logs (grep for `ak_`)
- Future: `jwt_` for refresh tokens, `sk_` for secret keys

**Character Set:**
- Hex encoding (0-9, a-f)
- No ambiguous characters (1/l/I, 0/O)
- URL-safe (no special encoding needed)

### Key Generation Algorithm

```rust
use rand::RngCore;
use sha2::{Sha256, Digest};

pub struct ApiKey {
    pub key_id: Uuid,
    pub plaintext_key: String,  // Only available at creation!
    pub key_hash: String,
    pub tenant_id: TenantId,
    pub name: String,
    pub permissions: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl ApiKey {
    pub fn generate(tenant_id: TenantId, name: String, permissions: Vec<String>, expires_at: Option<DateTime<Utc>>) -> Self {
        // Generate 32 random bytes using CSPRNG
        let mut rng = rand::thread_rng();
        let mut bytes = [0u8; 32];
        rng.fill_bytes(&mut bytes);

        // Hex-encode and add prefix
        let plaintext_key = format!("ak_{}", hex::encode(&bytes));

        // SHA-256 hash for storage
        let mut hasher = Sha256::new();
        hasher.update(plaintext_key.as_bytes());
        let key_hash = format!("{:x}", hasher.finalize());

        Self {
            key_id: Uuid::now_v7(),
            plaintext_key,
            key_hash,
            tenant_id,
            name,
            permissions,
            expires_at,
        }
    }

    pub fn verify(&self, candidate_key: &str) -> bool {
        // Constant-time comparison to prevent timing attacks
        let mut hasher = Sha256::new();
        hasher.update(candidate_key.as_bytes());
        let candidate_hash = format!("{:x}", hasher.finalize());

        // Use constant-time comparison
        use subtle::ConstantTimeEq;
        self.key_hash.as_bytes().ct_eq(candidate_hash.as_bytes()).into()
    }

    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.is_expired() && !self.is_revoked
    }
}
```

### Security Considerations

**Entropy Analysis:**
- 32 bytes = 256 bits of entropy
- 2^256 possible keys (more than atoms in universe)
- Brute force: ~10^77 attempts (impossible)
- Birthday paradox: collision after 2^128 keys (impossible)

**Timing Attack Prevention:**
- Use `subtle::ConstantTimeEq` for hash comparison
- Prevents timing oracle attacks
- Constant-time comparison ensures attacker can't distinguish valid vs invalid hashes by timing

**Key Rotation:**
- Keys don't expire by default (set expires_at if needed)
- Revoked keys immediately invalidated (no grace period)
- Future: automatic rotation policies (90-day expiration)

---

## JWT Token Design

### JWT Token Structure

**Header:**
```json
{
  "alg": "HS256",
  "typ": "JWT"
}
```

**Payload:**
```json
{
  "tenant_id": "01JC1234-5678-90ab-cdef-1234567890ab",
  "user_id": "01JC9876-5432-10fe-dcba-0987654321fe",
  "email": "user@example.com",
  "role": "developer",
  "permissions": ["collection::read", "collection::write"],
  "iat": 1699564800,     // Issued at (Unix timestamp)
  "exp": 1699651200,     // Expires at (Unix timestamp, +24h)
  "nbf": 1699564800      // Not before (Unix timestamp)
}
```

**Signature:**
```
HMACSHA256(
  base64UrlEncode(header) + "." + base64UrlEncode(payload),
  secret_key
)
```

### JWT Implementation

```rust
use jsonwebtoken::{encode, decode, Header, Algorithm, Validation, EncodingKey, DecodingKey};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    pub tenant_id: String,
    pub user_id: String,
    pub email: String,
    pub role: String,
    pub permissions: Vec<String>,
    pub iat: i64,  // Issued at
    pub exp: i64,  // Expiration
    pub nbf: i64,  // Not before
}

pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    expiration_hours: i64,
}

impl JwtService {
    pub fn new(secret: &str, expiration_hours: i64) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            expiration_hours,
        }
    }

    pub fn issue_token(&self, user: &User) -> CoreResult<String> {
        let now = Utc::now().timestamp();
        let exp = now + (self.expiration_hours * 3600);

        let claims = JwtClaims {
            tenant_id: user.tenant_id.to_string(),
            user_id: user.user_id.to_string(),
            email: user.email.clone(),
            role: user.role.as_str().to_string(),
            permissions: user.get_permissions(),
            iat: now,
            exp,
            nbf: now,
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| CoreError::internal(format!("JWT encoding failed: {}", e)))
    }

    pub fn validate_token(&self, token: &str) -> CoreResult<JwtClaims> {
        let validation = Validation::new(Algorithm::HS256);

        decode::<JwtClaims>(token, &self.decoding_key, &validation)
            .map(|data| data.claims)
            .map_err(|e| CoreError::unauthorized(format!("JWT validation failed: {}", e)))
    }
}
```

### Login Endpoint

```rust
#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_token: Option<String>,
}

pub async fn login(
    State(service): State<Arc<CollectionService>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    // Validate email + password (Argon2id verification)
    let user = service.authenticate_user(&req.email, &req.password).await
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid credentials".to_string()))?;

    // Issue JWT token
    let jwt_service = service.jwt_service();
    let access_token = jwt_service.issue_token(&user)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(LoginResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: 86400,  // 24 hours
        refresh_token: None,  // Future: implement refresh tokens
    }))
}
```

---

## Authentication Middleware

### REST Middleware (Axum)

```rust
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
    http::StatusCode,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct AuthContext {
    pub tenant_id: TenantId,
    pub user_id: Option<UserId>,  // None for API keys without user
    pub email: Option<String>,
    pub role: String,
    pub permissions: Vec<String>,
    pub auth_method: AuthMethod,
}

pub enum AuthMethod {
    ApiKey { key_id: Uuid },
    Jwt { exp: i64 },
}

pub async fn auth_middleware(
    State(service): State<Arc<CollectionService>>,
    mut request: Request,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    // Skip authentication for public endpoints
    let path = request.uri().path();
    if is_public_endpoint(path) {
        return Ok(next.run(request).await);
    }

    // Extract Authorization header
    let auth_header = request.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or((StatusCode::UNAUTHORIZED, "Missing Authorization header".to_string()))?;

    // Validate Bearer token
    if !auth_header.starts_with("Bearer ") {
        return Err((StatusCode::UNAUTHORIZED, "Invalid Authorization header format".to_string()));
    }

    let token = &auth_header[7..];  // Skip "Bearer "

    // Determine auth method: API key vs JWT
    let auth_context = if token.starts_with("ak_") {
        // API key authentication
        service.validate_api_key(token).await
            .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?
    } else {
        // JWT authentication
        service.validate_jwt(token).await
            .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?
    };

    // Inject auth context into request extensions
    request.extensions_mut().insert(auth_context);

    // Update last_used_at for API keys (async, don't block request)
    // TODO: implement background task for this

    Ok(next.run(request).await)
}

fn is_public_endpoint(path: &str) -> bool {
    matches!(path, "/health" | "/admin/health" | "/metrics")
}
```

### gRPC Interceptor (Tonic)

```rust
use tonic::{Request, Status};
use std::sync::Arc;

pub async fn auth_interceptor(
    service: Arc<CollectionService>,
    mut request: Request<()>,
) -> Result<Request<()>, Status> {
    // Skip authentication for health check
    let method = request.uri().path();
    if method.contains("Health") {
        return Ok(request);
    }

    // Extract authorization metadata
    let metadata = request.metadata();
    let auth_value = metadata
        .get("authorization")
        .ok_or_else(|| Status::unauthenticated("Missing authorization metadata"))?
        .to_str()
        .map_err(|_| Status::unauthenticated("Invalid authorization metadata"))?;

    // Validate Bearer token
    if !auth_value.starts_with("Bearer ") {
        return Err(Status::unauthenticated("Invalid authorization format"));
    }

    let token = &auth_value[7..];

    // Determine auth method: API key vs JWT
    let auth_context = if token.starts_with("ak_") {
        service.validate_api_key(token).await
            .map_err(|e| Status::unauthenticated(e.to_string()))?
    } else {
        service.validate_jwt(token).await
            .map_err(|e| Status::unauthenticated(e.to_string()))?
    };

    // Inject auth context into request extensions
    request.extensions_mut().insert(auth_context);

    Ok(request)
}
```

### Validation Logic

```rust
impl CollectionService {
    pub async fn validate_api_key(&self, plaintext_key: &str) -> CoreResult<AuthContext> {
        // Hash the provided key
        let mut hasher = Sha256::new();
        hasher.update(plaintext_key.as_bytes());
        let key_hash = format!("{:x}", hasher.finalize());

        // Lookup by hash
        let api_key = self.api_key_repository.find_by_hash(&key_hash).await?;

        // Check if revoked
        if api_key.is_revoked {
            return Err(CoreError::unauthorized("API key has been revoked"));
        }

        // Check expiration
        if api_key.is_expired() {
            return Err(CoreError::unauthorized("API key has expired"));
        }

        // Update last_used_at (async, non-blocking)
        let repo = self.api_key_repository.clone();
        let key_id = api_key.key_id;
        tokio::spawn(async move {
            let _ = repo.update_last_used(key_id).await;
        });

        // Build auth context
        Ok(AuthContext {
            tenant_id: api_key.tenant_id,
            user_id: None,
            email: None,
            role: infer_role_from_permissions(&api_key.permissions),
            permissions: api_key.permissions,
            auth_method: AuthMethod::ApiKey { key_id: api_key.key_id },
        })
    }

    pub async fn validate_jwt(&self, token: &str) -> CoreResult<AuthContext> {
        let claims = self.jwt_service.validate_token(token)?;

        // Load user from database (verify still active)
        let user_id = UserId::from_str(&claims.user_id)?;
        let user = self.user_repository.find_by_id(user_id).await?;

        if user.status != UserStatus::Active {
            return Err(CoreError::unauthorized("User account is not active"));
        }

        Ok(AuthContext {
            tenant_id: TenantId::from_str(&claims.tenant_id)?,
            user_id: Some(user_id),
            email: Some(claims.email),
            role: claims.role,
            permissions: claims.permissions,
            auth_method: AuthMethod::Jwt { exp: claims.exp },
        })
    }
}
```

---

## Admin Endpoints

### POST /admin/api-keys - Create API Key

**Request:**
```json
{
  "name": "production-service",
  "permissions": ["collection::read", "collection::write"],
  "expires_at": "2026-12-31T23:59:59Z"  // Optional
}
```

**Response (201 Created):**
```json
{
  "key_id": "01JC1234-5678-90ab-cdef-1234567890ab",
  "api_key": "ak_f8d7c6b5a4e3d2c1b0a9f8e7d6c5b4a3f2e1d0c9b8a7f6e5d4c3b2a1f0e9d8c7",
  "name": "production-service",
  "permissions": ["collection::read", "collection::write"],
  "expires_at": "2026-12-31T23:59:59Z",
  "created_at": "2025-11-08T12:00:00Z"
}
```

**⚠️ IMPORTANT:** Plaintext API key is only returned once! Client must save immediately.

**Implementation:**
```rust
#[derive(Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub permissions: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub struct CreateApiKeyResponse {
    pub key_id: String,
    pub api_key: String,  // ⚠️ Only returned once!
    pub name: String,
    pub permissions: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub async fn create_api_key(
    State(service): State<Arc<CollectionService>>,
    auth: Extension<AuthContext>,
    Json(req): Json<CreateApiKeyRequest>,
) -> Result<Json<CreateApiKeyResponse>, (StatusCode, String)> {
    // Only admins can create API keys
    if auth.role != "admin" {
        return Err((StatusCode::FORBIDDEN, "Admin role required".to_string()));
    }

    // Validate permissions
    for perm in &req.permissions {
        if !is_valid_permission(perm) {
            return Err((StatusCode::BAD_REQUEST, format!("Invalid permission: {}", perm)));
        }
    }

    // Generate API key
    let api_key = ApiKey::generate(
        auth.tenant_id,
        req.name,
        req.permissions,
        req.expires_at,
    );

    // Save to database
    service.api_key_repository.create(&api_key).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(CreateApiKeyResponse {
        key_id: api_key.key_id.to_string(),
        api_key: api_key.plaintext_key,  // ⚠️ Only time plaintext is exposed!
        name: api_key.name,
        permissions: api_key.permissions,
        expires_at: api_key.expires_at,
        created_at: Utc::now(),
    }))
}
```

### GET /admin/api-keys - List API Keys

**Response (200 OK):**
```json
{
  "api_keys": [
    {
      "key_id": "01JC1234-5678-90ab-cdef-1234567890ab",
      "name": "production-service",
      "permissions": ["collection::read", "collection::write"],
      "created_at": "2025-11-08T12:00:00Z",
      "expires_at": "2026-12-31T23:59:59Z",
      "last_used_at": "2025-11-08T14:30:00Z",
      "is_revoked": false
    }
  ],
  "total": 1
}
```

**⚠️ NOTE:** Plaintext API key is never returned (only hash stored).

### DELETE /admin/api-keys/{id} - Revoke API Key

**Response (200 OK):**
```json
{
  "key_id": "01JC1234-5678-90ab-cdef-1234567890ab",
  "name": "production-service",
  "revoked_at": "2025-11-08T15:00:00Z",
  "revoked_by": "user@example.com"
}
```

**Implementation:**
```rust
pub async fn revoke_api_key(
    State(service): State<Arc<CollectionService>>,
    auth: Extension<AuthContext>,
    Path(key_id): Path<String>,
) -> Result<Json<RevokeApiKeyResponse>, (StatusCode, String)> {
    // Only admins can revoke API keys
    if auth.role != "admin" {
        return Err((StatusCode::FORBIDDEN, "Admin role required".to_string()));
    }

    let key_id = Uuid::parse_str(&key_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid key ID".to_string()))?;

    // Soft delete (set is_revoked = 1)
    service.api_key_repository.revoke(key_id, auth.user_id).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(RevokeApiKeyResponse {
        key_id: key_id.to_string(),
        revoked_at: Utc::now(),
    }))
}
```

---

## Day-by-Day Action Plan

### Day 1: Database Schema + API Key Generation (8 hours)

**Morning (4 hours): Database Schema**

**Task 1.1: Create Migration File (1 hour)**
```bash
# Create migration file
touch crates/akidb-metadata/migrations/006_api_keys_table.sql

# Content: See "Database Schema" section above
```

**Task 1.2: Implement ApiKey Domain Model (1 hour)**
```rust
// File: crates/akidb-core/src/auth.rs (NEW FILE)

pub struct ApiKey {
    pub key_id: Uuid,
    pub tenant_id: TenantId,
    pub key_hash: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub is_revoked: bool,
}

impl ApiKey {
    pub fn generate(...) -> Self { ... }
    pub fn verify(&self, candidate: &str) -> bool { ... }
    pub fn is_expired(&self) -> bool { ... }
    pub fn is_valid(&self) -> bool { ... }
}
```

**Task 1.3: Implement ApiKeyRepository Trait (1 hour)**
```rust
// File: crates/akidb-core/src/traits.rs

#[async_trait]
pub trait ApiKeyRepository: Send + Sync {
    async fn create(&self, api_key: &ApiKey) -> CoreResult<()>;
    async fn find_by_hash(&self, key_hash: &str) -> CoreResult<ApiKey>;
    async fn find_by_id(&self, key_id: Uuid) -> CoreResult<ApiKey>;
    async fn list_by_tenant(&self, tenant_id: TenantId) -> CoreResult<Vec<ApiKey>>;
    async fn revoke(&self, key_id: Uuid, revoked_by: Option<UserId>) -> CoreResult<()>;
    async fn update_last_used(&self, key_id: Uuid) -> CoreResult<()>;
}
```

**Task 1.4: Unit Tests for ApiKey (1 hour)**
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_api_key_generation() { ... }

    #[test]
    fn test_api_key_verify() { ... }

    #[test]
    fn test_api_key_expiration() { ... }

    #[test]
    fn test_api_key_format() { ... }
}
```

**Afternoon (4 hours): SQLite Implementation**

**Task 1.5: Implement SqliteApiKeyRepository (2 hours)**
```rust
// File: crates/akidb-metadata/src/api_key_repository.rs (NEW FILE)

pub struct SqliteApiKeyRepository {
    pool: SqlitePool,
}

impl SqliteApiKeyRepository {
    pub fn new(pool: SqlitePool) -> Self { ... }
}

#[async_trait]
impl ApiKeyRepository for SqliteApiKeyRepository {
    async fn create(&self, api_key: &ApiKey) -> CoreResult<()> {
        sqlx::query!(...)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ... other methods
}
```

**Task 1.6: Integration Tests (2 hours)**
```rust
// File: crates/akidb-metadata/tests/api_key_repository_test.rs (NEW FILE)

#[tokio::test]
async fn test_create_api_key() { ... }

#[tokio::test]
async fn test_find_by_hash() { ... }

#[tokio::test]
async fn test_revoke_api_key() { ... }

#[tokio::test]
async fn test_list_by_tenant() { ... }

#[tokio::test]
async fn test_update_last_used() { ... }
```

**Day 1 Deliverables:**
- ✅ Migration file: `006_api_keys_table.sql`
- ✅ Domain model: `ApiKey` with generation + verification
- ✅ Repository trait: `ApiKeyRepository`
- ✅ SQLite implementation: `SqliteApiKeyRepository`
- ✅ 9 tests passing (4 unit + 5 integration)

---

### Day 2: Authentication Middleware (8 hours)

**Morning (4 hours): REST Middleware**

**Task 2.1: Implement AuthContext (1 hour)**
```rust
// File: crates/akidb-core/src/auth.rs

#[derive(Clone)]
pub struct AuthContext {
    pub tenant_id: TenantId,
    pub user_id: Option<UserId>,
    pub email: Option<String>,
    pub role: String,
    pub permissions: Vec<String>,
    pub auth_method: AuthMethod,
}

pub enum AuthMethod {
    ApiKey { key_id: Uuid },
    Jwt { exp: i64 },
}
```

**Task 2.2: Implement REST Middleware (2 hours)**
```rust
// File: crates/akidb-rest/src/middleware/auth.rs (NEW FILE)

pub async fn auth_middleware(
    State(service): State<Arc<CollectionService>>,
    mut request: Request,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    // See "Authentication Middleware" section above
}
```

**Task 2.3: Unit Tests for Middleware (1 hour)**
```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_auth_middleware_valid_api_key() { ... }

    #[tokio::test]
    async fn test_auth_middleware_invalid_api_key() { ... }

    #[tokio::test]
    async fn test_auth_middleware_missing_header() { ... }

    #[tokio::test]
    async fn test_auth_middleware_public_endpoint() { ... }
}
```

**Afternoon (4 hours): gRPC Interceptor**

**Task 2.4: Implement gRPC Interceptor (2 hours)**
```rust
// File: crates/akidb-grpc/src/interceptors/auth.rs (NEW FILE)

pub async fn auth_interceptor(
    service: Arc<CollectionService>,
    mut request: Request<()>,
) -> Result<Request<()>, Status> {
    // See "Authentication Middleware" section above
}
```

**Task 2.5: Integration with Axum Router (1 hour)**
```rust
// File: crates/akidb-rest/src/main.rs

let app = Router::new()
    // ... routes
    .layer(middleware::from_fn_with_state(service.clone(), auth_middleware))
    .with_state(service);
```

**Task 2.6: Integration Tests (1 hour)**
```rust
// File: crates/akidb-rest/tests/auth_middleware_test.rs (NEW FILE)

#[tokio::test]
async fn test_rest_api_requires_auth() { ... }

#[tokio::test]
async fn test_grpc_api_requires_auth() { ... }

#[tokio::test]
async fn test_public_endpoints_no_auth() { ... }
```

**Day 2 Deliverables:**
- ✅ AuthContext model
- ✅ REST auth middleware (Axum)
- ✅ gRPC auth interceptor (Tonic)
- ✅ Integration with routers
- ✅ 7 tests passing (4 unit + 3 integration)

---

### Day 3: Admin Endpoints (8 hours)

**Morning (4 hours): Create & List Endpoints**

**Task 3.1: POST /admin/api-keys (2 hours)**
```rust
// File: crates/akidb-rest/src/handlers/api_keys.rs (NEW FILE)

pub async fn create_api_key(...) -> Result<Json<CreateApiKeyResponse>, (StatusCode, String)> {
    // See "Admin Endpoints" section above
}
```

**Task 3.2: GET /admin/api-keys (1 hour)**
```rust
pub async fn list_api_keys(...) -> Result<Json<ListApiKeysResponse>, (StatusCode, String)> {
    // List API keys for authenticated tenant
}
```

**Task 3.3: Unit Tests (1 hour)**
```rust
#[tokio::test]
async fn test_create_api_key_success() { ... }

#[tokio::test]
async fn test_create_api_key_invalid_permissions() { ... }

#[tokio::test]
async fn test_list_api_keys() { ... }
```

**Afternoon (4 hours): Revoke & Get Endpoints**

**Task 3.4: DELETE /admin/api-keys/{id} (1 hour)**
```rust
pub async fn revoke_api_key(...) -> Result<Json<RevokeApiKeyResponse>, (StatusCode, String)> {
    // Soft delete API key
}
```

**Task 3.5: GET /admin/api-keys/{id} (1 hour)**
```rust
pub async fn get_api_key(...) -> Result<Json<ApiKeyDetailsResponse>, (StatusCode, String)> {
    // Get API key details (without plaintext key)
}
```

**Task 3.6: Add Routes to Router (1 hour)**
```rust
// File: crates/akidb-rest/src/main.rs

.route("/admin/api-keys", post(handlers::create_api_key))
.route("/admin/api-keys", get(handlers::list_api_keys))
.route("/admin/api-keys/:id", get(handlers::get_api_key))
.route("/admin/api-keys/:id", delete(handlers::revoke_api_key))
```

**Task 3.7: E2E Tests (1 hour)**
```rust
// File: crates/akidb-rest/tests/api_key_endpoints_test.rs (NEW FILE)

#[tokio::test]
async fn test_e2e_create_and_use_api_key() {
    // 1. Create API key via admin endpoint
    // 2. Use API key to access protected endpoint
    // 3. Verify access granted
}

#[tokio::test]
async fn test_e2e_revoke_api_key() {
    // 1. Create API key
    // 2. Use key (success)
    // 3. Revoke key
    // 4. Use key again (401)
}
```

**Day 3 Deliverables:**
- ✅ POST /admin/api-keys endpoint
- ✅ GET /admin/api-keys endpoint
- ✅ DELETE /admin/api-keys/{id} endpoint
- ✅ GET /admin/api-keys/{id} endpoint
- ✅ Routes integrated
- ✅ 6 tests passing (3 unit + 3 E2E)

---

### Day 4: JWT Token Support (8 hours)

**Morning (4 hours): JWT Service**

**Task 4.1: Add Dependencies (30 min)**
```toml
# Cargo.toml
jsonwebtoken = "9.2"
serde = { version = "1.0", features = ["derive"] }
```

**Task 4.2: Implement JwtService (2 hours)**
```rust
// File: crates/akidb-core/src/jwt.rs (NEW FILE)

pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    expiration_hours: i64,
}

impl JwtService {
    pub fn new(secret: &str, expiration_hours: i64) -> Self { ... }
    pub fn issue_token(&self, user: &User) -> CoreResult<String> { ... }
    pub fn validate_token(&self, token: &str) -> CoreResult<JwtClaims> { ... }
}
```

**Task 4.3: Unit Tests for JwtService (1.5 hours)**
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_jwt_issue_and_validate() { ... }

    #[test]
    fn test_jwt_expired_token() { ... }

    #[test]
    fn test_jwt_invalid_signature() { ... }

    #[test]
    fn test_jwt_malformed_token() { ... }
}
```

**Afternoon (4 hours): Login Endpoint**

**Task 4.4: POST /auth/login Endpoint (2 hours)**
```rust
// File: crates/akidb-rest/src/handlers/auth.rs (NEW FILE)

pub async fn login(
    State(service): State<Arc<CollectionService>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    // See "JWT Token Design" section above
}
```

**Task 4.5: Integrate JWT Validation in Middleware (1 hour)**
```rust
// File: crates/akidb-rest/src/middleware/auth.rs

// In auth_middleware:
let auth_context = if token.starts_with("ak_") {
    service.validate_api_key(token).await?
} else {
    service.validate_jwt(token).await?  // NEW
};
```

**Task 4.6: E2E Tests for JWT (1 hour)**
```rust
// File: crates/akidb-rest/tests/jwt_auth_test.rs (NEW FILE)

#[tokio::test]
async fn test_e2e_login_and_use_jwt() {
    // 1. Login with email/password
    // 2. Receive JWT token
    // 3. Use JWT to access protected endpoint
    // 4. Verify access granted
}

#[tokio::test]
async fn test_e2e_expired_jwt() {
    // 1. Create JWT with past expiration
    // 2. Use JWT to access endpoint
    // 3. Verify 401 Unauthorized
}
```

**Day 4 Deliverables:**
- ✅ JwtService implementation
- ✅ POST /auth/login endpoint
- ✅ JWT validation in middleware
- ✅ Configuration for JWT secret
- ✅ 6 tests passing (4 unit + 2 E2E)

---

### Day 5: Validation + Documentation (8 hours)

**Morning (4 hours): Final Validation**

**Task 5.1: Run All Tests (1 hour)**
```bash
# Run workspace tests
cargo test --workspace

# Expected: 175+ tests passing (155 baseline + 20 new)
# - 4 ApiKey unit tests
# - 5 ApiKeyRepository integration tests
# - 4 middleware unit tests
# - 3 middleware integration tests
# - 3 admin endpoint unit tests
# - 3 API key E2E tests
# - 4 JWT unit tests
# - 2 JWT E2E tests
# Total: 28 new tests
```

**Task 5.2: Fix Any Failing Tests (1 hour)**
```bash
# If tests fail, debug and fix
cargo test --workspace -- --nocapture
```

**Task 5.3: Performance Testing (1 hour)**
```bash
# Test auth overhead with simple benchmark
cargo bench --bench auth_bench

# Expected results:
# - API key validation: <1ms
# - JWT validation: <0.5ms
```

**Task 5.4: Security Review (1 hour)**
```bash
# Run cargo-audit
cargo audit

# Check for timing attacks in hash comparison
# Verify secret key not logged
# Verify plaintext API keys never stored
```

**Afternoon (4 hours): Documentation**

**Task 5.5: Update API-TUTORIAL.md (2 hours)**
```markdown
# API Authentication

## API Key Authentication

### Create API Key

```bash
curl -X POST https://akidb.example.com/admin/api-keys \
  -H "Authorization: Bearer <admin-api-key>" \
  -d '{
    "name": "production-service",
    "permissions": ["collection::read", "collection::write"]
  }'
```

### Use API Key

```bash
curl https://akidb.example.com/api/v1/collections \
  -H "Authorization: Bearer ak_..."
```

## JWT Authentication

### Login

```bash
curl -X POST https://akidb.example.com/auth/login \
  -d '{"email":"user@example.com","password":"secret"}'
```

### Use JWT Token

```bash
curl https://akidb.example.com/api/v1/collections \
  -H "Authorization: Bearer eyJhbGciOiJIUzI1NiIs..."
```
```

**Task 5.6: Create SECURITY.md (1 hour)**
```markdown
# Security Guide

## API Key Best Practices

1. Store API keys securely (environment variables, secrets manager)
2. Never commit API keys to version control
3. Rotate API keys regularly (90-day policy)
4. Use minimum permissions (principle of least privilege)
5. Revoke unused keys immediately

## JWT Token Best Practices

1. Store JWT secret in environment variables only
2. Use strong secret (minimum 256 bits)
3. Never log JWT secret
4. Set appropriate expiration (24 hours default)
5. Implement refresh token rotation (future)

## Common Security Pitfalls

- ❌ Hardcoding API keys in code
- ❌ Sharing API keys between environments
- ❌ Using weak JWT secrets
- ❌ Not revoking compromised keys
```

**Task 5.7: Update DEPLOYMENT-GUIDE.md (1 hour)**
```markdown
# Authentication Setup

## Environment Variables

```bash
# JWT secret (minimum 32 bytes)
export JWT_SECRET=$(openssl rand -hex 32)

# JWT expiration (hours)
export JWT_EXPIRATION_HOURS=24
```

## Creating First Admin API Key

```bash
# 1. Start server
cargo run -p akidb-rest

# 2. Create default admin user (if not exists)
# This is done automatically on first startup

# 3. Login as admin
curl -X POST http://localhost:8080/auth/login \
  -d '{"email":"admin@localhost","password":"changeme"}'

# 4. Create API key with admin token
curl -X POST http://localhost:8080/admin/api-keys \
  -H "Authorization: Bearer <jwt-token>" \
  -d '{"name":"admin-key","permissions":["admin"]}'

# 5. Save the returned API key securely!
```
```

**Day 5 Deliverables:**
- ✅ All tests passing (175+ tests)
- ✅ Performance benchmarks (auth overhead <2ms)
- ✅ Security review complete (zero critical issues)
- ✅ API-TUTORIAL.md updated
- ✅ SECURITY.md created
- ✅ DEPLOYMENT-GUIDE.md updated
- ✅ Week 1 completion report

---

## Testing Strategy

### Unit Tests (15 tests)

**ApiKey Tests (4 tests):**
```rust
#[test]
fn test_api_key_generation() {
    let key = ApiKey::generate(...);
    assert!(key.plaintext_key.starts_with("ak_"));
    assert_eq!(key.plaintext_key.len(), 65); // "ak_" + 62 hex chars
}

#[test]
fn test_api_key_verify() {
    let key = ApiKey::generate(...);
    assert!(key.verify(&key.plaintext_key));
    assert!(!key.verify("ak_invalid"));
}

#[test]
fn test_api_key_expiration() {
    let expired_key = ApiKey { expires_at: Some(Utc::now() - Duration::hours(1)), ... };
    assert!(expired_key.is_expired());
}

#[test]
fn test_api_key_hash_uniqueness() {
    let key1 = ApiKey::generate(...);
    let key2 = ApiKey::generate(...);
    assert_ne!(key1.key_hash, key2.key_hash);
}
```

**Middleware Tests (4 tests):**
```rust
#[tokio::test]
async fn test_auth_middleware_valid_api_key() {
    // Create valid API key
    // Make request with API key
    // Verify request succeeds
}

#[tokio::test]
async fn test_auth_middleware_invalid_api_key() {
    // Make request with invalid API key
    // Verify 401 Unauthorized
}

#[tokio::test]
async fn test_auth_middleware_missing_header() {
    // Make request without Authorization header
    // Verify 401 Unauthorized
}

#[tokio::test]
async fn test_auth_middleware_public_endpoint() {
    // Make request to /health without auth
    // Verify request succeeds
}
```

**JWT Tests (4 tests):**
```rust
#[test]
fn test_jwt_issue_and_validate() {
    let jwt_service = JwtService::new("secret", 24);
    let token = jwt_service.issue_token(&user).unwrap();
    let claims = jwt_service.validate_token(&token).unwrap();
    assert_eq!(claims.user_id, user.user_id.to_string());
}

#[test]
fn test_jwt_expired_token() {
    let jwt_service = JwtService::new("secret", -1); // Already expired
    let token = jwt_service.issue_token(&user).unwrap();
    assert!(jwt_service.validate_token(&token).is_err());
}

#[test]
fn test_jwt_invalid_signature() {
    let jwt_service1 = JwtService::new("secret1", 24);
    let jwt_service2 = JwtService::new("secret2", 24);
    let token = jwt_service1.issue_token(&user).unwrap();
    assert!(jwt_service2.validate_token(&token).is_err());
}

#[test]
fn test_jwt_malformed_token() {
    let jwt_service = JwtService::new("secret", 24);
    assert!(jwt_service.validate_token("not-a-jwt").is_err());
}
```

**Admin Endpoint Tests (3 tests):**
```rust
#[tokio::test]
async fn test_create_api_key_success() {
    // Login as admin
    // Create API key
    // Verify 201 Created
    // Verify plaintext key returned
}

#[tokio::test]
async fn test_create_api_key_forbidden() {
    // Login as developer (non-admin)
    // Attempt to create API key
    // Verify 403 Forbidden
}

#[tokio::test]
async fn test_list_api_keys() {
    // Create 3 API keys
    // List API keys
    // Verify all 3 returned
}
```

### Integration Tests (8 tests)

**ApiKeyRepository Tests (5 tests):**
```rust
#[tokio::test]
async fn test_create_api_key() {
    let pool = SqlitePool::connect(":memory:").await.unwrap();
    let repo = SqliteApiKeyRepository::new(pool);
    let api_key = ApiKey::generate(...);
    repo.create(&api_key).await.unwrap();
}

#[tokio::test]
async fn test_find_by_hash() {
    // Create API key
    // Find by hash
    // Verify found
}

#[tokio::test]
async fn test_revoke_api_key() {
    // Create API key
    // Revoke key
    // Verify is_revoked = true
}

#[tokio::test]
async fn test_list_by_tenant() {
    // Create 2 keys for Tenant A
    // Create 1 key for Tenant B
    // List by Tenant A
    // Verify 2 keys returned
}

#[tokio::test]
async fn test_update_last_used() {
    // Create API key
    // Update last_used_at
    // Verify timestamp updated
}
```

**Middleware Integration Tests (3 tests):**
```rust
#[tokio::test]
async fn test_rest_api_requires_auth() {
    // Start REST server
    // Make request without auth to /api/v1/collections
    // Verify 401 Unauthorized
}

#[tokio::test]
async fn test_grpc_api_requires_auth() {
    // Start gRPC server
    // Make request without auth
    // Verify UNAUTHENTICATED status
}

#[tokio::test]
async fn test_public_endpoints_no_auth() {
    // Start server
    // Make request to /health without auth
    // Verify 200 OK
}
```

### E2E Tests (5 tests)

**API Key E2E Tests (3 tests):**
```rust
#[tokio::test]
async fn test_e2e_create_and_use_api_key() {
    // 1. Login as admin → JWT token
    // 2. Create API key with JWT
    // 3. Use API key to list collections
    // 4. Verify success
}

#[tokio::test]
async fn test_e2e_revoke_api_key() {
    // 1. Create API key
    // 2. Use key (success)
    // 3. Revoke key
    // 4. Use key again → 401
}

#[tokio::test]
async fn test_e2e_expired_api_key() {
    // 1. Create API key with past expiration
    // 2. Use key → 401
}
```

**JWT E2E Tests (2 tests):**
```rust
#[tokio::test]
async fn test_e2e_login_and_use_jwt() {
    // 1. Login with email/password
    // 2. Receive JWT token
    // 3. Use JWT to list collections
    // 4. Verify success
}

#[tokio::test]
async fn test_e2e_jwt_expiration() {
    // 1. Create JWT with 1-second expiration
    // 2. Wait 2 seconds
    // 3. Use JWT → 401
}
```

### Total Tests: 28 new tests
- 15 unit tests
- 8 integration tests
- 5 E2E tests

---

## Code Structure

### New Files Created

```
crates/
├── akidb-core/
│   └── src/
│       ├── auth.rs (NEW)          # ApiKey, AuthContext, AuthMethod
│       ├── jwt.rs (NEW)           # JwtService, JwtClaims
│       └── traits.rs              # ApiKeyRepository trait (added)
│
├── akidb-metadata/
│   ├── migrations/
│   │   └── 006_api_keys_table.sql (NEW)
│   ├── src/
│   │   └── api_key_repository.rs (NEW)
│   └── tests/
│       └── api_key_repository_test.rs (NEW)
│
├── akidb-rest/
│   ├── src/
│   │   ├── handlers/
│   │   │   ├── api_keys.rs (NEW)
│   │   │   ├── auth.rs (NEW)
│   │   │   └── mod.rs (updated)
│   │   └── middleware/
│   │       └── auth.rs (NEW)
│   └── tests/
│       ├── api_key_endpoints_test.rs (NEW)
│       ├── auth_middleware_test.rs (NEW)
│       └── jwt_auth_test.rs (NEW)
│
└── akidb-grpc/
    └── src/
        └── interceptors/
            └── auth.rs (NEW)
```

### Modified Files

```
crates/
├── akidb-core/
│   └── Cargo.toml (add: sha2, jsonwebtoken, subtle)
│
├── akidb-rest/
│   ├── Cargo.toml (add: jsonwebtoken)
│   └── src/main.rs (add auth middleware + routes)
│
├── akidb-grpc/
│   └── src/main.rs (add auth interceptor)
│
└── akidb-service/
    └── src/
        ├── collection_service.rs (add JWT service field)
        └── lib.rs (export auth types)
```

### Dependencies Added

**crates/akidb-core/Cargo.toml:**
```toml
[dependencies]
sha2 = "0.10"              # SHA-256 hashing
jsonwebtoken = "9.2"       # JWT encoding/decoding
subtle = "2.5"             # Constant-time comparison
rand = "0.8"               # CSPRNG for key generation
hex = "0.4"                # Hex encoding
```

---

## Risk Assessment

### High Risk

**Risk 1: API Key Collision**
- **Probability:** Extremely Low (<0.0001%)
- **Impact:** CRITICAL (two tenants share same key)
- **Scenario:** Random key generator produces duplicate key
- **Mitigation:**
  - Unique index on key_hash (database enforces uniqueness)
  - 256-bit keyspace (2^256 possible keys)
  - Birthday paradox: collision after 2^128 keys (impossible)
- **Contingency:** Database constraint prevents insertion, user retries

**Risk 2: Timing Attack on Hash Comparison**
- **Probability:** Low (10%)
- **Impact:** HIGH (attacker learns partial hash)
- **Scenario:** Non-constant-time comparison leaks hash information
- **Mitigation:**
  - Use `subtle::ConstantTimeEq` for all hash comparisons
  - Unit test timing variance (should be constant)
- **Contingency:** Patch with constant-time comparison, rotate all keys

### Medium Risk

**Risk 3: JWT Secret Leak**
- **Probability:** Medium (20%)
- **Impact:** HIGH (all tokens compromised)
- **Scenario:** JWT secret logged or exposed in error message
- **Mitigation:**
  - Load secret from environment variable only
  - Never log secret (unit test to verify)
  - Minimum 256-bit secret (32 bytes)
- **Contingency:** Rotate secret immediately, revoke all JWT tokens, force re-login

**Risk 4: Performance Degradation**
- **Probability:** Medium (25%)
- **Impact:** MEDIUM (slower requests)
- **Scenario:** Database lookup for every API key validation adds latency
- **Mitigation:**
  - In-memory cache for API keys (cache key_hash → API key for 5 minutes)
  - JWT validation is fast (signature check only, no DB lookup)
  - Benchmark: auth overhead <2ms
- **Contingency:** Add caching layer (LRU cache, 1000 entries)

### Low Risk

**Risk 5: Migration Failure**
- **Probability:** Low (10%)
- **Impact:** LOW (deployment delayed)
- **Scenario:** SQL migration fails on production database
- **Mitigation:**
  - Test migration on local database
  - Test migration on staging database
  - Dry-run migration before applying
- **Contingency:** Rollback migration, fix SQL, re-apply

---

## Success Metrics

### Code Quality Metrics

**Target:**
- 175+ tests passing (155 baseline + 20 new)
- Zero compiler errors
- Zero compiler warnings
- All clippy checks passing
- Zero unsafe code in auth module

**Measurement:**
```bash
cargo test --workspace
cargo clippy --all-targets -- -D warnings
cargo build --workspace
```

### Performance Metrics

**Target:**
- API key validation: <1ms per request
- JWT validation: <0.5ms per request
- Database lookup: <5ms (indexed query)
- Total auth overhead: <2ms per request

**Measurement:**
```bash
cargo bench --bench auth_bench

# Expected output:
# api_key_validation    time:   [800 µs 900 µs 1.0 ms]
# jwt_validation        time:   [300 µs 400 µs 500 µs]
```

### Security Metrics

**Target:**
- Zero hardcoded secrets
- Zero plaintext API keys in database
- Zero timing attack vulnerabilities
- SHA-256 hashing for all keys
- Constant-time comparison for all hashes

**Measurement:**
```bash
# Secret scanning
git grep -i "jwt_secret\s*=\s*\"" || echo "No hardcoded secrets"

# Verify SHA-256 usage
rg "bcrypt|md5" crates/akidb-core/src/auth.rs && echo "FAIL" || echo "PASS"

# Verify constant-time comparison
rg "ConstantTimeEq" crates/akidb-core/src/auth.rs && echo "PASS" || echo "FAIL"
```

### Functional Metrics

**Target:**
- API keys work for REST API
- API keys work for gRPC API
- JWT tokens work for REST API
- JWT tokens work for gRPC API
- Expired keys rejected (401)
- Revoked keys rejected (401)
- Invalid keys rejected (401)
- Public endpoints accessible without auth

**Measurement:**
- Manual testing with curl/grpcurl
- E2E tests (5 tests passing)

---

## Appendix: API Examples

### Example 1: Create API Key

**Request:**
```bash
curl -X POST https://akidb.example.com/admin/api-keys \
  -H "Authorization: Bearer eyJhbGciOiJIUzI1NiIs..." \
  -H "Content-Type: application/json" \
  -d '{
    "name": "production-service",
    "permissions": ["collection::read", "collection::write"],
    "expires_at": "2026-12-31T23:59:59Z"
  }'
```

**Response:**
```json
{
  "key_id": "01JC1234-5678-90ab-cdef-1234567890ab",
  "api_key": "ak_f8d7c6b5a4e3d2c1b0a9f8e7d6c5b4a3f2e1d0c9b8a7f6e5d4c3b2a1f0e9d8c7",
  "name": "production-service",
  "permissions": ["collection::read", "collection::write"],
  "expires_at": "2026-12-31T23:59:59Z",
  "created_at": "2025-11-08T12:00:00Z"
}
```

**⚠️ IMPORTANT:** Save `api_key` immediately! It's only shown once.

### Example 2: Use API Key

**REST API:**
```bash
curl https://akidb.example.com/api/v1/collections \
  -H "Authorization: Bearer ak_f8d7c6b5a4e3d2c1b0a9f8e7d6c5b4a3f2e1d0c9b8a7f6e5d4c3b2a1f0e9d8c7"
```

**gRPC API:**
```bash
grpcurl \
  -H "authorization: Bearer ak_f8d7c6b5a4e3d2c1b0a9f8e7d6c5b4a3f2e1d0c9b8a7f6e5d4c3b2a1f0e9d8c7" \
  akidb.example.com:9000 \
  akidb.collection.v1.CollectionService/ListCollections
```

### Example 3: Login with JWT

**Request:**
```bash
curl -X POST https://akidb.example.com/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "email": "user@example.com",
    "password": "secure-password"
  }'
```

**Response:**
```json
{
  "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ0ZW5hbnRfaWQiOiIwMUpDMTIzNC01Njc4LTkwYWItY2RlZi0xMjM0NTY3ODkwYWIiLCJ1c2VyX2lkIjoiMDFKQzk4NzYtNTQzMi0xMGZlLWRjYmEtMDk4NzY1NDMyMWZlIiwiZW1haWwiOiJ1c2VyQGV4YW1wbGUuY29tIiwicm9sZSI6ImRldmVsb3BlciIsInBlcm1pc3Npb25zIjpbImNvbGxlY3Rpb246OnJlYWQiLCJjb2xsZWN0aW9uOjp3cml0ZSJdLCJpYXQiOjE2OTk1NjQ4MDAsImV4cCI6MTY5OTY1MTIwMCwibmJmIjoxNjk5NTY0ODAwfQ.xyz",
  "token_type": "Bearer",
  "expires_in": 86400,
  "refresh_token": null
}
```

### Example 4: Use JWT Token

**REST API:**
```bash
curl https://akidb.example.com/api/v1/collections \
  -H "Authorization: Bearer eyJhbGciOiJIUzI1NiIs..."
```

---

## Conclusion

Week 1 delivers a **production-ready authentication system** with API keys and JWT tokens, securing all API endpoints and enabling safe deployment to public internet.

**Timeline:** 5 working days
**Effort:** 40 hours
**Risk:** Medium (security-critical features)
**Impact:** HIGH (enables secure deployment)

**Success Criteria:**
- ✅ All API requests require authentication
- ✅ API keys + JWT tokens both work
- ✅ Multi-tenant isolation enforced
- ✅ 20+ tests passing
- ✅ Documentation complete

**Next Steps:** Week 2 - Authentication Polish (permission mapping, metrics, observability)

---

**Document Version:** 1.0
**Last Updated:** 2025-11-08
**Status:** READY FOR IMPLEMENTATION ✅
