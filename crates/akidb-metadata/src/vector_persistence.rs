//! Vector persistence layer for storing vector documents in SQLite.
//!
//! This module provides CRUD operations for vector documents with binary
//! serialization using bincode for efficient storage.

use akidb_core::{CollectionId, CoreError, CoreResult, DocumentId, VectorDocument};
use chrono::Utc;
use sqlx::SqlitePool;

/// Repository for persisting vector documents to SQLite.
///
/// Vectors are serialized as binary blobs using bincode for efficient storage.
/// Supports batch operations for high-throughput ingestion.
pub struct VectorPersistence {
    pool: SqlitePool,
}

impl VectorPersistence {
    /// Creates a new vector persistence repository.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Saves a single vector document to SQLite.
    ///
    /// If a document with the same (collection_id, doc_id) already exists,
    /// it will be replaced (upsert behavior).
    pub async fn save_vector(
        &self,
        collection_id: CollectionId,
        doc: &VectorDocument,
    ) -> CoreResult<()> {
        let collection_id_bytes = collection_id.to_bytes();
        let doc_id_bytes = doc.doc_id.to_bytes();

        // Serialize vector as binary (bincode)
        let vector_bytes = bincode::serialize(&doc.vector)
            .map_err(|e| CoreError::internal(format!("Failed to serialize vector: {}", e)))?;

        // Serialize metadata as JSON (if present)
        let metadata_json = doc
            .metadata
            .as_ref()
            .map(|m| serde_json::to_string(m))
            .transpose()
            .map_err(|e| CoreError::internal(format!("Failed to serialize metadata: {}", e)))?;

        let inserted_at = doc.inserted_at.to_rfc3339();
        let updated_at = Utc::now().to_rfc3339();

        // UPSERT: INSERT OR REPLACE
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO vector_documents
            (collection_id, doc_id, vector, external_id, metadata, inserted_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(&collection_id_bytes[..])
        .bind(&doc_id_bytes[..])
        .bind(&vector_bytes)
        .bind(&doc.external_id)
        .bind(&metadata_json)
        .bind(&inserted_at)
        .bind(&updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::internal(format!("Failed to save vector: {}", e)))?;

        Ok(())
    }

    /// Loads a single vector document from SQLite.
    ///
    /// Returns `None` if the document does not exist.
    pub async fn load_vector(
        &self,
        collection_id: CollectionId,
        doc_id: DocumentId,
    ) -> CoreResult<Option<VectorDocument>> {
        let collection_id_bytes = collection_id.to_bytes();
        let doc_id_bytes = doc_id.to_bytes();

        let row: Option<(Vec<u8>, Option<String>, Option<String>, String)> = sqlx::query_as(
            r#"
            SELECT vector, external_id, metadata, inserted_at
            FROM vector_documents
            WHERE collection_id = ?1 AND doc_id = ?2
            "#,
        )
        .bind(&collection_id_bytes[..])
        .bind(&doc_id_bytes[..])
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CoreError::internal(format!("Failed to load vector: {}", e)))?;

        let Some((vector_bytes, external_id, metadata_json, inserted_at)) = row else {
            return Ok(None);
        };

        // Deserialize vector
        let vector: Vec<f32> = bincode::deserialize(&vector_bytes)
            .map_err(|e| CoreError::internal(format!("Failed to deserialize vector: {}", e)))?;

        // Deserialize metadata
        let metadata = metadata_json
            .map(|json| serde_json::from_str(&json))
            .transpose()
            .map_err(|e| CoreError::internal(format!("Failed to deserialize metadata: {}", e)))?;

        // Parse timestamp
        let inserted_at = chrono::DateTime::parse_from_rfc3339(&inserted_at)
            .map_err(|e| CoreError::internal(format!("Failed to parse timestamp: {}", e)))?
            .with_timezone(&Utc);

        let mut doc = VectorDocument::new(doc_id, vector);
        if let Some(ext_id) = external_id {
            doc = doc.with_external_id(ext_id);
        }
        if let Some(meta) = metadata {
            doc = doc.with_metadata(meta);
        }
        doc.inserted_at = inserted_at;

        Ok(Some(doc))
    }

    /// Deletes a vector document from SQLite.
    ///
    /// Returns `Ok(())` even if the document didn't exist (idempotent).
    pub async fn delete_vector(
        &self,
        collection_id: CollectionId,
        doc_id: DocumentId,
    ) -> CoreResult<()> {
        let collection_id_bytes = collection_id.to_bytes();
        let doc_id_bytes = doc_id.to_bytes();

        sqlx::query(
            r#"
            DELETE FROM vector_documents
            WHERE collection_id = ?1 AND doc_id = ?2
            "#,
        )
        .bind(&collection_id_bytes[..])
        .bind(&doc_id_bytes[..])
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::internal(format!("Failed to delete vector: {}", e)))?;

        Ok(())
    }

    /// Loads all vector documents for a collection.
    ///
    /// This is called on startup to rebuild in-memory indexes.
    /// For large collections (>100k docs), this may be slow.
    pub async fn load_all_vectors(
        &self,
        collection_id: CollectionId,
    ) -> CoreResult<Vec<VectorDocument>> {
        let collection_id_bytes = collection_id.to_bytes();

        let rows: Vec<(Vec<u8>, Vec<u8>, Option<String>, Option<String>, String)> = sqlx::query_as(
            r#"
            SELECT doc_id, vector, external_id, metadata, inserted_at
            FROM vector_documents
            WHERE collection_id = ?1
            ORDER BY inserted_at ASC
            "#,
        )
        .bind(&collection_id_bytes[..])
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::internal(format!("Failed to load all vectors: {}", e)))?;

        let mut documents = Vec::with_capacity(rows.len());

        for (doc_id_bytes, vector_bytes, external_id, metadata_json, inserted_at) in rows {
            // Parse doc_id
            let doc_id = DocumentId::from_bytes(&doc_id_bytes)
                .map_err(|e| CoreError::internal(format!("Failed to parse document ID: {}", e)))?;

            // Deserialize vector
            let vector: Vec<f32> = bincode::deserialize(&vector_bytes)
                .map_err(|e| CoreError::internal(format!("Failed to deserialize vector: {}", e)))?;

            // Deserialize metadata
            let metadata = metadata_json
                .map(|json| serde_json::from_str(&json))
                .transpose()
                .map_err(|e| {
                    CoreError::internal(format!("Failed to deserialize metadata: {}", e))
                })?;

            // Parse timestamp
            let inserted_at = chrono::DateTime::parse_from_rfc3339(&inserted_at)
                .map_err(|e| CoreError::internal(format!("Failed to parse timestamp: {}", e)))?
                .with_timezone(&Utc);

            let mut doc = VectorDocument::new(doc_id, vector);
            if let Some(ext_id) = external_id {
                doc = doc.with_external_id(ext_id);
            }
            if let Some(meta) = metadata {
                doc = doc.with_metadata(meta);
            }
            doc.inserted_at = inserted_at;

            documents.push(doc);
        }

        Ok(documents)
    }

    /// Batch save multiple vector documents.
    ///
    /// More efficient than calling `save_vector()` in a loop.
    /// Uses a single transaction for atomicity.
    pub async fn save_batch(
        &self,
        collection_id: CollectionId,
        documents: &[VectorDocument],
    ) -> CoreResult<()> {
        if documents.is_empty() {
            return Ok(());
        }

        let collection_id_bytes = collection_id.to_bytes();

        // Start transaction
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| CoreError::internal(format!("Failed to begin transaction: {}", e)))?;

        for doc in documents {
            let doc_id_bytes = doc.doc_id.to_bytes();

            let vector_bytes = bincode::serialize(&doc.vector)
                .map_err(|e| CoreError::internal(format!("Failed to serialize vector: {}", e)))?;

            let metadata_json = doc
                .metadata
                .as_ref()
                .map(|m| serde_json::to_string(m))
                .transpose()
                .map_err(|e| CoreError::internal(format!("Failed to serialize metadata: {}", e)))?;

            let inserted_at = doc.inserted_at.to_rfc3339();
            let updated_at = Utc::now().to_rfc3339();

            sqlx::query(
                r#"
                INSERT OR REPLACE INTO vector_documents
                (collection_id, doc_id, vector, external_id, metadata, inserted_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                "#,
            )
            .bind(&collection_id_bytes[..])
            .bind(&doc_id_bytes[..])
            .bind(&vector_bytes)
            .bind(&doc.external_id)
            .bind(&metadata_json)
            .bind(&inserted_at)
            .bind(&updated_at)
            .execute(&mut *tx)
            .await
            .map_err(|e| CoreError::internal(format!("Failed to insert vector in batch: {}", e)))?;
        }

        // Commit transaction
        tx.commit()
            .await
            .map_err(|e| CoreError::internal(format!("Failed to commit batch save: {}", e)))?;

        Ok(())
    }

    /// Gets the count of vector documents in a collection.
    pub async fn count_vectors(&self, collection_id: CollectionId) -> CoreResult<usize> {
        let collection_id_bytes = collection_id.to_bytes();

        let count: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM vector_documents WHERE collection_id = ?1
            "#,
        )
        .bind(&collection_id_bytes[..])
        .fetch_one(&self.pool)
        .await
        .map_err(|e| CoreError::internal(format!("Failed to count vectors: {}", e)))?;

        Ok(count.0 as usize)
    }

    /// Deletes all vector documents for a collection.
    ///
    /// Called when a collection is deleted. Cascade delete should handle this,
    /// but this method is provided for explicit cleanup.
    pub async fn delete_all_vectors(&self, collection_id: CollectionId) -> CoreResult<()> {
        let collection_id_bytes = collection_id.to_bytes();

        sqlx::query(
            r#"
            DELETE FROM vector_documents WHERE collection_id = ?1
            "#,
        )
        .bind(&collection_id_bytes[..])
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::internal(format!("Failed to delete all vectors: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use akidb_core::DocumentId;

    async fn create_test_pool() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("Failed to create in-memory database");

        // Run migrations
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        pool
    }

    #[tokio::test]
    async fn test_save_and_load_vector() {
        let pool = create_test_pool().await;
        let persistence = VectorPersistence::new(pool.clone());

        // Create test tenant, database, and collection
        let tenant_id = akidb_core::TenantId::new();
        let database_id = akidb_core::DatabaseId::new();
        let collection_id = CollectionId::new();

        // Create tenant first
        sqlx::query(
            r#"
            INSERT INTO tenants (tenant_id, name, slug, status, created_at, updated_at)
            VALUES (?1, 'test_tenant', 'test', 'active', datetime('now'), datetime('now'))
            "#,
        )
        .bind(&tenant_id.to_bytes()[..])
        .execute(&pool)
        .await
        .unwrap();

        // Create database
        sqlx::query(
            r#"
            INSERT INTO databases (database_id, tenant_id, name, state, created_at, updated_at)
            VALUES (?1, ?2, 'test_db', 'ready', datetime('now'), datetime('now'))
            "#,
        )
        .bind(&database_id.to_bytes()[..])
        .bind(&tenant_id.to_bytes()[..])
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            r#"
            INSERT INTO collections (collection_id, database_id, name, dimension, metric, embedding_model, created_at, updated_at)
            VALUES (?1, ?2, 'test_collection', 128, 'cosine', 'test', datetime('now'), datetime('now'))
            "#,
        )
        .bind(&collection_id.to_bytes()[..])
        .bind(&database_id.to_bytes()[..])
        .execute(&pool)
        .await
        .unwrap();

        // Create vector document
        let doc_id = DocumentId::new();
        let vector = vec![0.1; 128];
        let doc =
            VectorDocument::new(doc_id, vector.clone()).with_external_id("test-doc-1".to_string());

        // Save
        persistence.save_vector(collection_id, &doc).await.unwrap();

        // Load
        let loaded = persistence
            .load_vector(collection_id, doc_id)
            .await
            .unwrap()
            .expect("Document should exist");

        assert_eq!(loaded.doc_id, doc_id);
        assert_eq!(loaded.vector, vector);
        assert_eq!(loaded.external_id, Some("test-doc-1".to_string()));
    }

    #[tokio::test]
    async fn test_delete_vector() {
        let pool = create_test_pool().await;
        let persistence = VectorPersistence::new(pool.clone());

        // Create test tenant, database, and collection
        let tenant_id = akidb_core::TenantId::new();
        let database_id = akidb_core::DatabaseId::new();
        let collection_id = CollectionId::new();

        // Create tenant first
        sqlx::query(
            r#"
            INSERT INTO tenants (tenant_id, name, slug, status, created_at, updated_at)
            VALUES (?1, 'test_tenant', 'test', 'active', datetime('now'), datetime('now'))
            "#,
        )
        .bind(&tenant_id.to_bytes()[..])
        .execute(&pool)
        .await
        .unwrap();

        // Create database
        sqlx::query(
            r#"
            INSERT INTO databases (database_id, tenant_id, name, state, created_at, updated_at)
            VALUES (?1, ?2, 'test_db', 'ready', datetime('now'), datetime('now'))
            "#,
        )
        .bind(&database_id.to_bytes()[..])
        .bind(&tenant_id.to_bytes()[..])
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            r#"
            INSERT INTO collections (collection_id, database_id, name, dimension, metric, embedding_model, created_at, updated_at)
            VALUES (?1, ?2, 'test_collection', 128, 'cosine', 'test', datetime('now'), datetime('now'))
            "#,
        )
        .bind(&collection_id.to_bytes()[..])
        .bind(&database_id.to_bytes()[..])
        .execute(&pool)
        .await
        .unwrap();

        let doc_id = DocumentId::new();
        let doc = VectorDocument::new(doc_id, vec![0.1; 128]);

        // Save and delete
        persistence.save_vector(collection_id, &doc).await.unwrap();
        persistence
            .delete_vector(collection_id, doc_id)
            .await
            .unwrap();

        // Verify deleted
        let loaded = persistence
            .load_vector(collection_id, doc_id)
            .await
            .unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_batch_save() {
        let pool = create_test_pool().await;
        let persistence = VectorPersistence::new(pool.clone());

        // Create test tenant, database, and collection
        let tenant_id = akidb_core::TenantId::new();
        let database_id = akidb_core::DatabaseId::new();
        let collection_id = CollectionId::new();

        // Create tenant first
        sqlx::query(
            r#"
            INSERT INTO tenants (tenant_id, name, slug, status, created_at, updated_at)
            VALUES (?1, 'test_tenant', 'test', 'active', datetime('now'), datetime('now'))
            "#,
        )
        .bind(&tenant_id.to_bytes()[..])
        .execute(&pool)
        .await
        .unwrap();

        // Create database
        sqlx::query(
            r#"
            INSERT INTO databases (database_id, tenant_id, name, state, created_at, updated_at)
            VALUES (?1, ?2, 'test_db', 'ready', datetime('now'), datetime('now'))
            "#,
        )
        .bind(&database_id.to_bytes()[..])
        .bind(&tenant_id.to_bytes()[..])
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            r#"
            INSERT INTO collections (collection_id, database_id, name, dimension, metric, embedding_model, created_at, updated_at)
            VALUES (?1, ?2, 'test_collection', 128, 'cosine', 'test', datetime('now'), datetime('now'))
            "#,
        )
        .bind(&collection_id.to_bytes()[..])
        .bind(&database_id.to_bytes()[..])
        .execute(&pool)
        .await
        .unwrap();

        // Create batch of documents
        let docs: Vec<VectorDocument> = (0..10)
            .map(|i| {
                let doc_id = DocumentId::new();
                let vector = vec![0.1 * i as f32; 128];
                VectorDocument::new(doc_id, vector)
            })
            .collect();

        // Batch save
        persistence.save_batch(collection_id, &docs).await.unwrap();

        // Verify count
        let count = persistence.count_vectors(collection_id).await.unwrap();
        assert_eq!(count, 10);

        // Load all
        let loaded = persistence.load_all_vectors(collection_id).await.unwrap();
        assert_eq!(loaded.len(), 10);
    }
}
