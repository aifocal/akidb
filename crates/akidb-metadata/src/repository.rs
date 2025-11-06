use std::str::FromStr;

use akidb_core::{CoreError, CoreResult, DatabaseDescriptor, DatabaseId, DatabaseState, TenantId};
use chrono::{DateTime, SecondsFormat, Utc};
use sqlx::sqlite::SqliteRow;
use sqlx::{query, Executor, Row, Sqlite, SqlitePool};

/// SQLite-backed repository for database descriptors.
pub struct SqliteDatabaseRepository {
    pool: SqlitePool,
}

impl SqliteDatabaseRepository {
    /// Creates a new repository backed by the provided pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Returns the underlying pool (useful for composing with other services).
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Inserts a database descriptor via the supplied executor.
    pub async fn create_with_executor<'e, E>(
        executor: E,
        database: &DatabaseDescriptor,
    ) -> CoreResult<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let database_id = database.database_id.to_bytes().to_vec();
        let tenant_id = database.tenant_id.to_bytes().to_vec();
        let name = &database.name;
        let description = &database.description;
        let state = database.state.as_str();
        let schema_version = database.schema_version;
        let created_at = database
            .created_at
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        let updated_at = database
            .updated_at
            .to_rfc3339_opts(SecondsFormat::Millis, true);

        sqlx::query!(
            r#"
            INSERT INTO databases (
                database_id,
                tenant_id,
                name,
                description,
                state,
                schema_version,
                created_at,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            database_id,
            tenant_id,
            name,
            description,
            state,
            schema_version,
            created_at,
            updated_at
        )
        .execute(executor)
        .await
        .map(|_| ())
        .map_err(|err| map_sqlx_error("database", database.database_id.to_string(), err))
    }

    /// Updates a database descriptor via the supplied executor.
    pub async fn update_with_executor<'e, E>(
        executor: E,
        database: &DatabaseDescriptor,
    ) -> CoreResult<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let database_id = database.database_id.to_bytes().to_vec();
        let tenant_id = database.tenant_id.to_bytes().to_vec();
        let name = &database.name;
        let description = &database.description;
        let state = database.state.as_str();
        let schema_version = database.schema_version;
        let updated_at = database
            .updated_at
            .to_rfc3339_opts(SecondsFormat::Millis, true);

        let result = sqlx::query!(
            r#"
            UPDATE databases
               SET tenant_id = ?2,
                   name = ?3,
                   description = ?4,
                   state = ?5,
                   schema_version = ?6,
                   updated_at = ?7
             WHERE database_id = ?1
            "#,
            database_id,
            tenant_id,
            name,
            description,
            state,
            schema_version,
            updated_at
        )
        .execute(executor)
        .await
        .map_err(|err| map_sqlx_error("database", database.database_id.to_string(), err))?;

        if result.rows_affected() == 0 {
            return Err(CoreError::not_found(
                "database",
                database.database_id.to_string(),
            ));
        }
        Ok(())
    }

    /// Deletes a database descriptor via the supplied executor.
    pub async fn delete_with_executor<'e, E>(executor: E, database_id: DatabaseId) -> CoreResult<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let bytes = database_id.to_bytes().to_vec();
        let result = query(
            r#"
            DELETE FROM databases
             WHERE database_id = ?1
            "#,
        )
        .bind(bytes)
        .execute(executor)
        .await
        .map_err(|err| map_sqlx_error("database", database_id.to_string(), err))?;

        if result.rows_affected() == 0 {
            return Err(CoreError::not_found("database", database_id.to_string()));
        }
        Ok(())
    }

    fn map_row(row: SqliteRow) -> CoreResult<DatabaseDescriptor> {
        let db_bytes: Vec<u8> = row.get("database_id");
        let tenant_bytes: Vec<u8> = row.get("tenant_id");
        let database_id = DatabaseId::from_bytes(&db_bytes)
            .map_err(|err| CoreError::internal(err.to_string()))?;
        let tenant_id = TenantId::from_bytes(&tenant_bytes)
            .map_err(|err| CoreError::internal(err.to_string()))?;
        let name: String = row.get("name");
        let description: Option<String> = row.get("description");
        let state: String = row.get("state");
        let state = DatabaseState::from_str(&state)
            .map_err(|_| CoreError::invalid_state(format!("unknown database state `{state}`")))?;
        let schema_version: i64 = row.get("schema_version");
        let created_at: String = row.get("created_at");
        let updated_at: String = row.get("updated_at");

        let created_at = DateTime::parse_from_rfc3339(&created_at)
            .map_err(|err| CoreError::internal(format!("invalid created_at: {err}")))?
            .with_timezone(&Utc);
        let updated_at = DateTime::parse_from_rfc3339(&updated_at)
            .map_err(|err| CoreError::internal(format!("invalid updated_at: {err}")))?
            .with_timezone(&Utc);

        Ok(DatabaseDescriptor {
            database_id,
            tenant_id,
            name,
            description,
            state,
            schema_version,
            created_at,
            updated_at,
        })
    }
}

#[async_trait::async_trait]
impl akidb_core::DatabaseRepository for SqliteDatabaseRepository {
    async fn create(&self, database: &DatabaseDescriptor) -> CoreResult<()> {
        Self::create_with_executor(&self.pool, database).await
    }

    async fn get(&self, database_id: DatabaseId) -> CoreResult<Option<DatabaseDescriptor>> {
        let row = query(
            r#"
            SELECT database_id,
                   tenant_id,
                   name,
                   description,
                   state,
                   schema_version,
                   created_at,
                   updated_at
              FROM databases
             WHERE database_id = ?1
            "#,
        )
        .bind(database_id.to_bytes().to_vec())
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| CoreError::internal(err.to_string()))?;

        match row {
            Some(row) => Ok(Some(Self::map_row(row)?)),
            None => Ok(None),
        }
    }

    async fn list_by_tenant(&self, tenant_id: TenantId) -> CoreResult<Vec<DatabaseDescriptor>> {
        let rows = query(
            r#"
            SELECT database_id,
                   tenant_id,
                   name,
                   description,
                   state,
                   schema_version,
                   created_at,
                   updated_at
              FROM databases
             WHERE tenant_id = ?1
          ORDER BY created_at ASC
            "#,
        )
        .bind(tenant_id.to_bytes().to_vec())
        .fetch_all(&self.pool)
        .await
        .map_err(|err| CoreError::internal(err.to_string()))?;

        rows.into_iter().map(Self::map_row).collect()
    }

    async fn update(&self, database: &DatabaseDescriptor) -> CoreResult<()> {
        Self::update_with_executor(&self.pool, database).await
    }

    async fn delete(&self, database_id: DatabaseId) -> CoreResult<()> {
        Self::delete_with_executor(&self.pool, database_id).await
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
            } else {
                CoreError::internal(message)
            }
        }
        other => CoreError::internal(other.to_string()),
    }
}
