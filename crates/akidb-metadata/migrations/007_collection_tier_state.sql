-- Migration: Collection Tier State for Hot/Warm/Cold Tiering
-- Created: 2025-11-09
-- Phase 10 Week 2

CREATE TABLE collection_tier_state (
    collection_id BLOB PRIMARY KEY REFERENCES collections(collection_id) ON DELETE CASCADE,
    tier TEXT NOT NULL CHECK(tier IN ('hot','warm','cold')),
    last_accessed_at TEXT NOT NULL,
    access_count INTEGER NOT NULL DEFAULT 0,
    access_window_start TEXT NOT NULL,
    pinned INTEGER NOT NULL DEFAULT 0,  -- 0 = false, 1 = true
    snapshot_id BLOB,  -- NULL if not in cold tier
    warm_file_path TEXT,  -- NULL if not in warm tier
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE INDEX ix_tier_state_tier ON collection_tier_state(tier);
CREATE INDEX ix_tier_state_last_accessed ON collection_tier_state(last_accessed_at);
CREATE INDEX ix_tier_state_pinned ON collection_tier_state(pinned);

-- Trigger to auto-update updated_at
CREATE TRIGGER update_tier_state_timestamp
AFTER UPDATE ON collection_tier_state
FOR EACH ROW
BEGIN
    UPDATE collection_tier_state
    SET updated_at = CURRENT_TIMESTAMP
    WHERE collection_id = NEW.collection_id;
END;
