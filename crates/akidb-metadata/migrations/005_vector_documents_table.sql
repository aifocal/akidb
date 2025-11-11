-- Migration: Add vector_documents table for vector persistence
-- Phase 5 Week 2: Vector persistence layer
-- Stores vector documents with binary serialized vectors

CREATE TABLE IF NOT EXISTS vector_documents (
    -- Primary key: collection_id + doc_id (composite)
    collection_id BLOB NOT NULL,
    doc_id BLOB NOT NULL,

    -- Vector data (binary serialized with bincode)
    vector BLOB NOT NULL,

    -- Optional metadata
    external_id TEXT,
    metadata TEXT,  -- JSON

    -- Timestamps
    inserted_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,

    -- Constraints
    PRIMARY KEY (collection_id, doc_id),
    FOREIGN KEY (collection_id) REFERENCES collections(collection_id) ON DELETE CASCADE
) STRICT;

-- Index for efficient lookup by external_id
CREATE INDEX IF NOT EXISTS ix_vector_documents_external_id
    ON vector_documents(collection_id, external_id)
    WHERE external_id IS NOT NULL;

-- Index for time-series queries
CREATE INDEX IF NOT EXISTS ix_vector_documents_inserted_at
    ON vector_documents(collection_id, inserted_at);

-- Trigger: Auto-update updated_at on UPDATE
CREATE TRIGGER IF NOT EXISTS trigger_vector_documents_updated_at
AFTER UPDATE ON vector_documents
FOR EACH ROW
BEGIN
    UPDATE vector_documents
    SET updated_at = datetime('now')
    WHERE collection_id = NEW.collection_id AND doc_id = NEW.doc_id;
END;
