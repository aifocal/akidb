use std::str::FromStr;

use akidb_core::{
    CollectionDescriptor, CollectionId, CoreError, CoreResult, DatabaseId, DistanceMetric,
};
use chrono::{DateTime, SecondsFormat, Utc};
use sqlx::sqlite::SqliteRow;
use sqlx::{query, Executor, Row, Sqlite, SqlitePool};

/// SQLite-backed repository for collection descriptors.
pub struct SqliteCollectionRepository {
    pool: SqlitePool,
}

impl SqliteCollectionRepository {
    /// Creates a new repository backed by the provided pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Returns the underlying pool (useful for composing with other services).
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Inserts a collection descriptor via the supplied executor.
    pub async fn create_with_executor<'e, E>(
        executor: E,
        collection: &CollectionDescriptor,
    ) -> CoreResult<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        // Validate dimension bounds
        collection
            .validate_dimension()
            .map_err(CoreError::invalid_state)?;

        let collection_id = collection.collection_id.to_bytes().to_vec();
        let database_id = collection.database_id.to_bytes().to_vec();
        let name = &collection.name;
        let dimension = i64::from(collection.dimension);
        let metric = collection.metric.as_str();
        let embedding_model = &collection.embedding_model;
        let hnsw_m = i64::from(collection.hnsw_m);
        let hnsw_ef_construction = i64::from(collection.hnsw_ef_construction);
        let max_doc_count = i64::try_from(collection.max_doc_count)
            .map_err(|_| CoreError::invalid_state("max_doc_count exceeds 63-bit range"))?;
        let created_at = collection
            .created_at
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        let updated_at = collection
            .updated_at
            .to_rfc3339_opts(SecondsFormat::Millis, true);

        query(
            r#"
            INSERT INTO collections (
                collection_id,
                database_id,
                name,
                dimension,
                metric,
                embedding_model,
                hnsw_m,
                hnsw_ef_construction,
                max_doc_count,
                created_at,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
        )
        .bind(collection_id)
        .bind(database_id)
        .bind(name)
        .bind(dimension)
        .bind(metric)
        .bind(embedding_model)
        .bind(hnsw_m)
        .bind(hnsw_ef_construction)
        .bind(max_doc_count)
        .bind(created_at)
        .bind(updated_at)
        .execute(executor)
        .await
        .map(|_| ())
        .map_err(|err| map_sqlx_error("collection", collection.collection_id.to_string(), err))
    }

    /// Updates a collection descriptor via the supplied executor.
    pub async fn update_with_executor<'e, E>(
        executor: E,
        collection: &CollectionDescriptor,
    ) -> CoreResult<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        // Validate dimension bounds
        collection
            .validate_dimension()
            .map_err(CoreError::invalid_state)?;

        let collection_id = collection.collection_id.to_bytes().to_vec();
        let database_id = collection.database_id.to_bytes().to_vec();
        let name = &collection.name;
        let dimension = i64::from(collection.dimension);
        let metric = collection.metric.as_str();
        let embedding_model = &collection.embedding_model;
        let hnsw_m = i64::from(collection.hnsw_m);
        let hnsw_ef_construction = i64::from(collection.hnsw_ef_construction);
        let max_doc_count = i64::try_from(collection.max_doc_count)
            .map_err(|_| CoreError::invalid_state("max_doc_count exceeds 63-bit range"))?;
        let updated_at = collection
            .updated_at
            .to_rfc3339_opts(SecondsFormat::Millis, true);

        let result = query(
            r#"
            UPDATE collections
               SET database_id = ?2,
                   name = ?3,
                   dimension = ?4,
                   metric = ?5,
                   embedding_model = ?6,
                   hnsw_m = ?7,
                   hnsw_ef_construction = ?8,
                   max_doc_count = ?9,
                   updated_at = ?10
             WHERE collection_id = ?1
            "#,
        )
        .bind(collection_id)
        .bind(database_id)
        .bind(name)
        .bind(dimension)
        .bind(metric)
        .bind(embedding_model)
        .bind(hnsw_m)
        .bind(hnsw_ef_construction)
        .bind(max_doc_count)
        .bind(updated_at)
        .execute(executor)
        .await
        .map_err(|err| map_sqlx_error("collection", collection.collection_id.to_string(), err))?;

        if result.rows_affected() == 0 {
            return Err(CoreError::not_found(
                "collection",
                collection.collection_id.to_string(),
            ));
        }
        Ok(())
    }

    /// Deletes a collection descriptor via the supplied executor.
    pub async fn delete_with_executor<'e, E>(
        executor: E,
        collection_id: CollectionId,
    ) -> CoreResult<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let bytes = collection_id.to_bytes().to_vec();
        let result = query(
            r#"
            DELETE FROM collections
             WHERE collection_id = ?1
            "#,
        )
        .bind(bytes)
        .execute(executor)
        .await
        .map_err(|err| map_sqlx_error("collection", collection_id.to_string(), err))?;

        if result.rows_affected() == 0 {
            return Err(CoreError::not_found(
                "collection",
                collection_id.to_string(),
            ));
        }
        Ok(())
    }

    fn map_row(row: SqliteRow) -> CoreResult<CollectionDescriptor> {
        let collection_bytes: Vec<u8> = row.get("collection_id");
        let database_bytes: Vec<u8> = row.get("database_id");
        let collection_id = CollectionId::from_bytes(&collection_bytes)
            .map_err(|err| CoreError::internal(err.to_string()))?;
        let database_id = DatabaseId::from_bytes(&database_bytes)
            .map_err(|err| CoreError::internal(err.to_string()))?;
        let name: String = row.get("name");
        let dimension: i64 = row.get("dimension");
        let metric: String = row.get("metric");
        let metric = DistanceMetric::from_str(&metric)
            .map_err(|_| CoreError::invalid_state(format!("unknown distance metric `{metric}`")))?;
        let embedding_model: String = row.get("embedding_model");
        let hnsw_m: i64 = row.get("hnsw_m");
        let hnsw_ef_construction: i64 = row.get("hnsw_ef_construction");
        let max_doc_count: i64 = row.get("max_doc_count");
        let created_at: String = row.get("created_at");
        let updated_at: String = row.get("updated_at");

        let dimension = u32::try_from(dimension)
            .map_err(|_| CoreError::invalid_state("dimension stored negative value"))?;
        let hnsw_m = u32::try_from(hnsw_m)
            .map_err(|_| CoreError::invalid_state("hnsw_m stored negative value"))?;
        let hnsw_ef_construction = u32::try_from(hnsw_ef_construction)
            .map_err(|_| CoreError::invalid_state("hnsw_ef_construction stored negative value"))?;
        let max_doc_count = u64::try_from(max_doc_count)
            .map_err(|_| CoreError::invalid_state("max_doc_count stored negative value"))?;

        let created_at = DateTime::parse_from_rfc3339(&created_at)
            .map_err(|err| CoreError::internal(format!("invalid created_at: {err}")))?
            .with_timezone(&Utc);
        let updated_at = DateTime::parse_from_rfc3339(&updated_at)
            .map_err(|err| CoreError::internal(format!("invalid updated_at: {err}")))?
            .with_timezone(&Utc);

        Ok(CollectionDescriptor {
            collection_id,
            database_id,
            name,
            dimension,
            metric,
            embedding_model,
            hnsw_m,
            hnsw_ef_construction,
            max_doc_count,
            created_at,
            updated_at,
        })
    }
}

#[async_trait::async_trait]
impl akidb_core::CollectionRepository for SqliteCollectionRepository {
    async fn create(&self, collection: &CollectionDescriptor) -> CoreResult<()> {
        Self::create_with_executor(&self.pool, collection).await
    }

    async fn get(&self, collection_id: CollectionId) -> CoreResult<Option<CollectionDescriptor>> {
        let row = query(
            r#"
            SELECT collection_id,
                   database_id,
                   name,
                   dimension,
                   metric,
                   embedding_model,
                   hnsw_m,
                   hnsw_ef_construction,
                   max_doc_count,
                   created_at,
                   updated_at
              FROM collections
             WHERE collection_id = ?1
            "#,
        )
        .bind(collection_id.to_bytes().to_vec())
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| CoreError::internal(err.to_string()))?;

        match row {
            Some(row) => Ok(Some(Self::map_row(row)?)),
            None => Ok(None),
        }
    }

    async fn list_by_database(
        &self,
        database_id: DatabaseId,
    ) -> CoreResult<Vec<CollectionDescriptor>> {
        let rows = query(
            r#"
            SELECT collection_id,
                   database_id,
                   name,
                   dimension,
                   metric,
                   embedding_model,
                   hnsw_m,
                   hnsw_ef_construction,
                   max_doc_count,
                   created_at,
                   updated_at
              FROM collections
             WHERE database_id = ?1
          ORDER BY created_at ASC
            "#,
        )
        .bind(database_id.to_bytes().to_vec())
        .fetch_all(&self.pool)
        .await
        .map_err(|err| CoreError::internal(err.to_string()))?;

        rows.into_iter().map(Self::map_row).collect()
    }

    async fn list_all(&self) -> CoreResult<Vec<CollectionDescriptor>> {
        let rows = query(
            r#"
            SELECT collection_id,
                   database_id,
                   name,
                   dimension,
                   metric,
                   embedding_model,
                   hnsw_m,
                   hnsw_ef_construction,
                   max_doc_count,
                   created_at,
                   updated_at
              FROM collections
          ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|err| CoreError::internal(err.to_string()))?;

        rows.into_iter().map(Self::map_row).collect()
    }

    async fn update(&self, collection: &CollectionDescriptor) -> CoreResult<()> {
        Self::update_with_executor(&self.pool, collection).await
    }

    async fn delete(&self, collection_id: CollectionId) -> CoreResult<()> {
        Self::delete_with_executor(&self.pool, collection_id).await
    }
}

fn map_sqlx_error(entity: &'static str, id: String, err: sqlx::Error) -> CoreError {
    match err {
        sqlx::Error::Database(db_err) => {
            let message = db_err.message().to_string();
            if message.contains("UNIQUE constraint failed") {
                CoreError::already_exists(entity, id)
            } else if message.contains("FOREIGN KEY constraint failed") {
                CoreError::invalid_state("foreign key constraint failed".to_string())
            } else if message.contains("CHECK constraint failed") {
                CoreError::invalid_state(format!("check constraint failed: {message}"))
            } else {
                CoreError::internal(message)
            }
        }
        other => CoreError::internal(other.to_string()),
    }
}
