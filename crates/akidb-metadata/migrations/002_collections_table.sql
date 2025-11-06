-- Add collections table for Phase 2 (Embedding Service)
-- Vector collection metadata with HNSW parameters and embedding configuration

CREATE TABLE IF NOT EXISTS collections (
    collection_id BLOB PRIMARY KEY,
    database_id BLOB NOT NULL REFERENCES databases(database_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    dimension INTEGER NOT NULL CHECK(dimension BETWEEN 16 AND 4096),
    metric TEXT NOT NULL CHECK(metric IN ('cosine','dot','l2')),
    embedding_model TEXT NOT NULL,
    hnsw_m INTEGER NOT NULL DEFAULT 32,
    hnsw_ef_construction INTEGER NOT NULL DEFAULT 200,
    max_doc_count INTEGER NOT NULL DEFAULT 50000000,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE(database_id, name)
) STRICT;

CREATE TRIGGER IF NOT EXISTS trg_collections_updated_at
AFTER UPDATE ON collections
FOR EACH ROW
WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE collections
       SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
     WHERE rowid = NEW.rowid;
END;
