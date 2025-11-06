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
