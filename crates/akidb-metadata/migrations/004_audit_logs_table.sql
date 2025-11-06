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
