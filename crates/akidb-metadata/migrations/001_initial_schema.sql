PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS tenants (
    tenant_id BLOB PRIMARY KEY,
    external_id TEXT UNIQUE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL CHECK(status IN ('provisioning','active','suspended','decommissioned')),
    memory_quota_bytes INTEGER NOT NULL DEFAULT 34359738368,
    storage_quota_bytes INTEGER NOT NULL DEFAULT 1099511627776,
    qps_quota INTEGER NOT NULL DEFAULT 200,
    metadata TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

-- Users table moved to migration 003_users_table.sql for Phase 3

CREATE TABLE IF NOT EXISTS databases (
    database_id BLOB PRIMARY KEY,
    tenant_id BLOB NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,
    state TEXT NOT NULL CHECK(state IN ('provisioning','ready','migrating','deleting')),
    schema_version INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE(tenant_id, name)
) STRICT;

CREATE TRIGGER IF NOT EXISTS trg_tenants_updated_at
AFTER UPDATE ON tenants
FOR EACH ROW
WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE tenants
       SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
     WHERE rowid = NEW.rowid;
END;

-- Users trigger moved to migration 003_users_table.sql for Phase 3

CREATE TRIGGER IF NOT EXISTS trg_databases_updated_at
AFTER UPDATE ON databases
FOR EACH ROW
WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE databases
       SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
     WHERE rowid = NEW.rowid;
END;
