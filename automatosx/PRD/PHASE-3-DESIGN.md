# Phase 3 Design: User Management & RBAC

**Project:** AkiDB 2.0
**Phase:** 3 - User Management & Role-Based Access Control
**Date:** 2025-11-06
**Status:** Design Phase

---

## Executive Summary

Phase 3 introduces **user management and role-based access control (RBAC)** to AkiDB 2.0, enabling multi-tenant isolation and fine-grained authorization. This phase builds on Phase 1 (metadata layer) and Phase 2 (embedding service) to deliver enterprise-grade access control.

**Key Deliverables:**
- User domain model with authentication (password hashing)
- User repository with CRUD operations
- Role-based access control (Admin, Developer, Viewer, Auditor)
- Permission system with action-based authorization
- Audit logging for compliance
- Integration tests for user management and RBAC

**Design Philosophy:**
- **Pragmatic first, policy-driven later**: Use enum-based RBAC for Phase 3, prepare for Cedar policy engine in Phase 4+
- **Security by default**: Password hashing with Argon2, secure session management
- **Audit everything**: All authorization decisions logged for compliance

---

## Architecture Overview

### Component Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                      akidb-core                              │
│  ┌──────────────────┐  ┌──────────────────┐                │
│  │  User Domain     │  │  RBAC Domain     │                │
│  │  - UserDescriptor│  │  - Role          │                │
│  │  - UserStatus    │  │  - Permission    │                │
│  │  - Credentials   │  │  - Action        │                │
│  └──────────────────┘  └──────────────────┘                │
│  ┌──────────────────────────────────────────┐              │
│  │  Repository Traits                        │              │
│  │  - UserRepository                         │              │
│  │  - AuditLogRepository                     │              │
│  └──────────────────────────────────────────┘              │
└─────────────────────────────────────────────────────────────┘
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                   akidb-metadata                             │
│  ┌──────────────────────────────────────────┐              │
│  │  SQLite Implementations                   │              │
│  │  - SqliteUserRepository                   │              │
│  │  - SqliteAuditLogRepository               │              │
│  └──────────────────────────────────────────┘              │
│  ┌──────────────────────────────────────────┐              │
│  │  Migrations                               │              │
│  │  - 003_users_table.sql                    │              │
│  │  - 004_audit_logs_table.sql               │              │
│  └──────────────────────────────────────────┘              │
└─────────────────────────────────────────────────────────────┘
```

### Domain Model Hierarchy (Phase 3)

```
Tenant
  ├── User (NEW in Phase 3)
  │   ├── UserId (UUID v7)
  │   ├── email: String
  │   ├── password_hash: String (Argon2)
  │   ├── role: Role (Admin | Developer | Viewer | Auditor)
  │   ├── status: UserStatus (Active | Suspended | Deactivated)
  │   └── last_login_at: Option<DateTime<Utc>>
  │
  └── Database
      └── Collection
```

---

## Domain Models

### 1. User Domain Model

**File:** `crates/akidb-core/src/user.rs`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ids::{TenantId, UserId};

/// User within a tenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDescriptor {
    pub user_id: UserId,
    pub tenant_id: TenantId,
    pub email: String,
    pub password_hash: String,  // Argon2 hash
    pub role: Role,
    pub status: UserStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
}

/// User status lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserStatus {
    /// User is active and can authenticate.
    Active,
    /// User is temporarily suspended (can be reactivated).
    Suspended,
    /// User is deactivated (requires admin intervention).
    Deactivated,
}

/// User roles with predefined permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    /// Full access to tenant resources (create users, manage databases, etc.).
    Admin,
    /// Can create/read/update/delete collections and documents.
    Developer,
    /// Read-only access to collections and documents.
    Viewer,
    /// Read-only access to audit logs and metrics.
    Auditor,
}

/// Actions that can be performed on resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    // User management
    UserCreate,
    UserRead,
    UserUpdate,
    UserDelete,

    // Database management
    DatabaseCreate,
    DatabaseRead,
    DatabaseUpdate,
    DatabaseDelete,

    // Collection management
    CollectionCreate,
    CollectionRead,
    CollectionUpdate,
    CollectionDelete,

    // Document operations
    DocumentInsert,
    DocumentSearch,
    DocumentUpdate,
    DocumentDelete,

    // Audit logs
    AuditRead,
}

impl UserDescriptor {
    /// Create a new user descriptor.
    pub fn new(tenant_id: TenantId, email: impl Into<String>, role: Role) -> Self {
        let now = Utc::now();
        Self {
            user_id: UserId::new(),
            tenant_id,
            email: email.into(),
            password_hash: String::new(),  // Set via set_password()
            role,
            status: UserStatus::Active,
            created_at: now,
            updated_at: now,
            last_login_at: None,
        }
    }

    /// Update the last login timestamp.
    pub fn record_login(&mut self) {
        self.last_login_at = Some(Utc::now());
        self.touch();
    }

    /// Transition to a new status.
    pub fn transition_to(&mut self, status: UserStatus) {
        self.status = status;
        self.touch();
    }

    /// Update the updated_at timestamp.
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }

    /// Check if user has permission to perform an action.
    pub fn has_permission(&self, action: Action) -> bool {
        if self.status != UserStatus::Active {
            return false;
        }

        match self.role {
            Role::Admin => true,  // Admin has all permissions
            Role::Developer => matches!(
                action,
                Action::DatabaseRead
                    | Action::CollectionCreate
                    | Action::CollectionRead
                    | Action::CollectionUpdate
                    | Action::CollectionDelete
                    | Action::DocumentInsert
                    | Action::DocumentSearch
                    | Action::DocumentUpdate
                    | Action::DocumentDelete
            ),
            Role::Viewer => matches!(
                action,
                Action::DatabaseRead
                    | Action::CollectionRead
                    | Action::DocumentSearch
            ),
            Role::Auditor => matches!(
                action,
                Action::DatabaseRead
                    | Action::CollectionRead
                    | Action::AuditRead
            ),
        }
    }
}

impl Role {
    /// Convert role to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Developer => "developer",
            Role::Viewer => "viewer",
            Role::Auditor => "auditor",
        }
    }
}

impl std::str::FromStr for Role {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "admin" => Ok(Role::Admin),
            "developer" => Ok(Role::Developer),
            "viewer" => Ok(Role::Viewer),
            "auditor" => Ok(Role::Auditor),
            _ => Err(format!("invalid role: {s}")),
        }
    }
}

impl UserStatus {
    /// Convert status to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            UserStatus::Active => "active",
            UserStatus::Suspended => "suspended",
            UserStatus::Deactivated => "deactivated",
        }
    }
}

impl std::str::FromStr for UserStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(UserStatus::Active),
            "suspended" => Ok(UserStatus::Suspended),
            "deactivated" => Ok(UserStatus::Deactivated),
            _ => Err(format!("invalid user status: {s}")),
        }
    }
}

impl Action {
    /// Convert action to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Action::UserCreate => "user::create",
            Action::UserRead => "user::read",
            Action::UserUpdate => "user::update",
            Action::UserDelete => "user::delete",
            Action::DatabaseCreate => "database::create",
            Action::DatabaseRead => "database::read",
            Action::DatabaseUpdate => "database::update",
            Action::DatabaseDelete => "database::delete",
            Action::CollectionCreate => "collection::create",
            Action::CollectionRead => "collection::read",
            Action::CollectionUpdate => "collection::update",
            Action::CollectionDelete => "collection::delete",
            Action::DocumentInsert => "document::insert",
            Action::DocumentSearch => "document::search",
            Action::DocumentUpdate => "document::update",
            Action::DocumentDelete => "document::delete",
            Action::AuditRead => "audit::read",
        }
    }
}
```

### 2. Audit Log Domain Model

**File:** `crates/akidb-core/src/audit.rs`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::ids::{AuditLogId, TenantId, UserId};
use crate::user::Action;

/// Audit log entry for compliance and security monitoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub audit_log_id: AuditLogId,
    pub tenant_id: TenantId,
    pub user_id: Option<UserId>,
    pub action: Action,
    pub resource_type: String,
    pub resource_id: String,
    pub result: AuditResult,
    pub reason: Option<String>,
    pub metadata: Option<JsonValue>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Result of an authorization decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditResult {
    /// Action was allowed.
    Allowed,
    /// Action was denied.
    Denied,
}

impl AuditLogEntry {
    /// Create a new audit log entry.
    pub fn new(
        tenant_id: TenantId,
        user_id: Option<UserId>,
        action: Action,
        resource_type: impl Into<String>,
        resource_id: impl Into<String>,
        result: AuditResult,
    ) -> Self {
        Self {
            audit_log_id: AuditLogId::new(),
            tenant_id,
            user_id,
            action,
            resource_type: resource_type.into(),
            resource_id: resource_id.into(),
            result,
            reason: None,
            metadata: None,
            ip_address: None,
            user_agent: None,
            created_at: Utc::now(),
        }
    }

    /// Add a reason for the authorization decision.
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Add metadata (e.g., request details).
    pub fn with_metadata(mut self, metadata: JsonValue) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Add client IP address.
    pub fn with_ip(mut self, ip: impl Into<String>) -> Self {
        self.ip_address = Some(ip.into());
        self
    }

    /// Add user agent string.
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }
}

impl AuditResult {
    /// Convert result to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditResult::Allowed => "allowed",
            AuditResult::Denied => "denied",
        }
    }
}

impl std::str::FromStr for AuditResult {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "allowed" => Ok(AuditResult::Allowed),
            "denied" => Ok(AuditResult::Denied),
            _ => Err(format!("invalid audit result: {s}")),
        }
    }
}
```

### 3. Repository Traits

**File:** `crates/akidb-core/src/traits.rs` (additions)

```rust
/// Repository for user management.
#[async_trait]
pub trait UserRepository: Send + Sync {
    /// Create a new user.
    async fn create(&self, user: &UserDescriptor) -> CoreResult<()>;

    /// Get a user by ID.
    async fn get(&self, user_id: UserId) -> CoreResult<Option<UserDescriptor>>;

    /// Get a user by email within a tenant.
    async fn get_by_email(&self, tenant_id: TenantId, email: &str) -> CoreResult<Option<UserDescriptor>>;

    /// List all users for a tenant.
    async fn list_by_tenant(&self, tenant_id: TenantId) -> CoreResult<Vec<UserDescriptor>>;

    /// Update a user.
    async fn update(&self, user: &UserDescriptor) -> CoreResult<()>;

    /// Delete a user.
    async fn delete(&self, user_id: UserId) -> CoreResult<()>;
}

/// Repository for audit logs.
#[async_trait]
pub trait AuditLogRepository: Send + Sync {
    /// Create a new audit log entry.
    async fn create(&self, entry: &AuditLogEntry) -> CoreResult<()>;

    /// List audit logs for a tenant (with optional pagination).
    async fn list_by_tenant(&self, tenant_id: TenantId, limit: usize, offset: usize) -> CoreResult<Vec<AuditLogEntry>>;

    /// List audit logs for a specific user.
    async fn list_by_user(&self, user_id: UserId, limit: usize, offset: usize) -> CoreResult<Vec<AuditLogEntry>>;
}
```

---

## SQLite Schema

### Users Table

**File:** `crates/akidb-metadata/migrations/003_users_table.sql`

```sql
-- Users table for multi-tenant user management
CREATE TABLE IF NOT EXISTS users (
    user_id BLOB PRIMARY KEY,
    tenant_id BLOB NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('admin','developer','viewer','auditor')),
    status TEXT NOT NULL CHECK(status IN ('active','suspended','deactivated')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    last_login_at TEXT,
    UNIQUE(tenant_id, email)
) STRICT;

-- Index for fast email lookup within tenant
CREATE UNIQUE INDEX ux_users_tenant_email ON users(tenant_id, email);

-- Index for listing users by tenant
CREATE INDEX ix_users_tenant ON users(tenant_id);

-- Index for filtering by status
CREATE INDEX ix_users_status ON users(status);
```

### Audit Logs Table

**File:** `crates/akidb-metadata/migrations/004_audit_logs_table.sql`

```sql
-- Audit logs for compliance and security monitoring
CREATE TABLE IF NOT EXISTS audit_logs (
    audit_log_id BLOB PRIMARY KEY,
    tenant_id BLOB NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    user_id BLOB REFERENCES users(user_id) ON DELETE SET NULL,
    action TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    result TEXT NOT NULL CHECK(result IN ('allowed','denied')),
    reason TEXT,
    metadata TEXT,  -- JSON
    ip_address TEXT,
    user_agent TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

-- Index for querying by tenant and time
CREATE INDEX ix_audit_logs_tenant_created ON audit_logs(tenant_id, created_at DESC);

-- Index for querying by user
CREATE INDEX ix_audit_logs_user_created ON audit_logs(user_id, created_at DESC);

-- Index for querying denied actions (security monitoring)
CREATE INDEX ix_audit_logs_denied ON audit_logs(result, created_at DESC) WHERE result = 'denied';
```

---

## Password Hashing

**Approach:** Use **Argon2id** (winner of Password Hashing Competition 2015)

**Crate:** `argon2` (https://docs.rs/argon2/)

**Implementation:**

```rust
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand::rngs::OsRng;

pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(password.as_bytes(), &salt)?;
    Ok(password_hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, argon2::password_hash::Error> {
    let parsed_hash = PasswordHash::new(hash)?;
    let argon2 = Argon2::default();
    Ok(argon2.verify_password(password.as_bytes(), &parsed_hash).is_ok())
}
```

**Why Argon2?**
- Memory-hard (resistant to GPU/ASIC attacks)
- Recommended by OWASP
- Configurable cost parameters
- Standard in modern applications

---

## Implementation Plan

### Phase 3.1: User Management (Week 1)

1. **Day 1-2**: Domain models
   - Add `UserId` and `AuditLogId` to `akidb-core/src/ids.rs`
   - Implement `UserDescriptor` in `akidb-core/src/user.rs`
   - Implement `AuditLogEntry` in `akidb-core/src/audit.rs`
   - Add repository traits to `akidb-core/src/traits.rs`

2. **Day 3-4**: Persistence layer
   - Create migration `003_users_table.sql`
   - Create migration `004_audit_logs_table.sql`
   - Implement `SqliteUserRepository`
   - Implement `SqliteAuditLogRepository`

3. **Day 5**: Password hashing
   - Add `argon2` dependency
   - Implement `hash_password()` and `verify_password()` utilities
   - Add to `SqliteUserRepository::create()`

### Phase 3.2: RBAC & Tests (Week 2)

4. **Day 6-7**: Integration tests
   - User CRUD tests (create, read, update, delete)
   - Email uniqueness constraint tests
   - Password hashing tests
   - Role-based permission tests

5. **Day 8-9**: Audit logging tests
   - Create audit log entries
   - Query by tenant
   - Query by user
   - Test cascade delete (user deleted → audit logs remain)

6. **Day 10**: Documentation
   - Update `CLAUDE.md` with Phase 3 status
   - Create Phase 3 completion report

---

## Testing Strategy

### Integration Tests (15+ tests)

**User Management (8 tests):**
1. `create_user_successfully` - Create and fetch user
2. `get_user_by_email` - Lookup by email within tenant
3. `list_users_by_tenant` - List all users for tenant
4. `update_user_role` - Change user role
5. `update_user_status` - Suspend/reactivate user
6. `record_user_login` - Update last_login_at
7. `enforce_unique_email_per_tenant` - Duplicate email constraint
8. `cascade_delete_tenant_removes_users` - FK cascade

**RBAC (4 tests):**
9. `admin_has_all_permissions` - Admin can do everything
10. `developer_has_limited_permissions` - Developer can't manage users
11. `viewer_is_read_only` - Viewer can only read
12. `suspended_user_has_no_permissions` - Suspended users denied

**Audit Logging (3 tests):**
13. `create_audit_log_entry` - Create and fetch audit log
14. `list_audit_logs_by_tenant` - Query by tenant
15. `list_denied_actions` - Security monitoring

**Password Hashing (2 tests):**
16. `hash_password_is_secure` - Hash is not plaintext
17. `verify_password_works` - Verification succeeds

---

## Security Considerations

### Password Security
- **Argon2id** with default parameters (time=2, mem=19456 KiB, parallel=1)
- **Salt:** Unique per password (generated via `OsRng`)
- **Storage:** Never store plaintext passwords
- **Verification:** Timing-safe comparison

### Authorization
- **Deny by default**: Users must have explicit permission
- **Least privilege**: Viewer role has minimal permissions
- **Audit everything**: All authorization decisions logged
- **Multi-tenancy**: Users cannot access other tenants' resources

### Compliance
- **Audit logs**: Track all access attempts (allow + deny)
- **Retention**: Audit logs persist even if user is deleted (SET NULL FK)
- **Metadata**: IP address, user agent for forensic analysis
- **Time-series**: Indexed by `created_at DESC` for fast queries

---

## Future Work (Phase 4+)

### Cedar Policy Engine Integration

**When:** Phase 4 (after Week 0 sandbox validation)

**Migration Path:**
1. Keep enum-based RBAC as fallback
2. Add `cedar-policy` crate dependency
3. Create `PolicyEngine` struct
4. Implement Cedar-based authorization
5. Migrate policies from Rust enums to Cedar syntax

**Example Cedar Policy:**
```cedar
permit(
  principal in Role::"tenant-alpha#admin",
  action,
  resource
) when {
  resource.tenant == principal.tenant
};
```

**Benefits:**
- Centralized policy management
- Non-developers can author policies
- Policy versioning and rollback
- Attribute-based access control (ABAC)

---

## Success Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Test Pass Rate | 100% | 17+ tests passing |
| Password Hash Time | <100ms | Argon2 benchmark |
| Authorization Check | <1ms | Enum-based permission lookup |
| Audit Log Write | <5ms | SQLite insert latency |
| User CRUD | <10ms P95 | SQLite query latency |

---

## Summary

Phase 3 delivers **production-ready user management and RBAC** with:
- ✅ User domain model with secure password hashing (Argon2)
- ✅ Role-based permissions (Admin, Developer, Viewer, Auditor)
- ✅ Audit logging for compliance (SOC 2, HIPAA ready)
- ✅ 17+ integration tests (100% coverage)
- ✅ Multi-tenant isolation (users scoped to tenants)

**Design Principle:** Pragmatic enum-based RBAC first, Cedar policy engine later.

**Next Phase (4):** Vector engine with HNSW index, query API, and performance benchmarks.

---

**References:**
- ADR-002: Cedar Policy Engine for RBAC
- AkiDB 2.0 PRD: Multi-Tenant Data Governance
- OWASP Password Storage Cheat Sheet
- Argon2 Specification (RFC 9106)
