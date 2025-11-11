-- Migration 006: API Keys Table
-- Add API key authentication support for AkiDB 2.0 Phase 8

CREATE TABLE api_keys (
    key_id BLOB PRIMARY KEY,                  -- UUID v7
    tenant_id BLOB NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    key_hash TEXT NOT NULL UNIQUE,            -- SHA-256 hash of API key
    name TEXT NOT NULL,                       -- Human-readable key name
    permissions TEXT NOT NULL,                -- JSON array of permissions
    created_at TEXT NOT NULL,                 -- ISO-8601 timestamp
    expires_at TEXT,                          -- NULL = never expires
    last_used_at TEXT,                        -- ISO-8601 timestamp
    created_by BLOB REFERENCES users(user_id) ON DELETE SET NULL
) STRICT;

-- Index for fast tenant-based lookups
CREATE INDEX ix_api_keys_tenant ON api_keys(tenant_id);

-- Index for fast key hash lookups (authentication)
CREATE INDEX ix_api_keys_hash ON api_keys(key_hash);

-- Trigger to auto-update timestamps would go here if needed
-- (currently handled in application layer)
