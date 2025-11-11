//! SQLite implementation of API key repository for authentication.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{query, Executor, Row, Sqlite, SqlitePool};

use akidb_core::{
    ApiKeyDescriptor, ApiKeyId, ApiKeyRepository, CoreError, CoreResult, TenantId, UserId,
};

/// SQLite implementation of the API key repository.
pub struct SqliteApiKeyRepository {
    pool: SqlitePool,
}

impl SqliteApiKeyRepository {
    /// Creates a new SQLite API key repository.
    #[must_use]
    pub const fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create an API key using the provided executor (for transaction support).
    async fn create_with_executor<'e, E>(
        &self,
        api_key: &ApiKeyDescriptor,
        key_hash: &str,
        executor: E,
    ) -> CoreResult<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let key_id = api_key.key_id.to_bytes().to_vec();
        let tenant_id = api_key.tenant_id.to_bytes().to_vec();
        let name = &api_key.name;
        let permissions = serde_json::to_string(&api_key.permissions)
            .map_err(|e| CoreError::internal(format!("Failed to serialize permissions: {e}")))?;
        let created_at = api_key.created_at.to_rfc3339();
        let expires_at = api_key.expires_at.map(|t| t.to_rfc3339());
        let last_used_at = api_key.last_used_at.map(|t| t.to_rfc3339());
        let created_by = api_key.created_by.map(|id| id.to_bytes().to_vec());

        query(
            "INSERT INTO api_keys (key_id, tenant_id, key_hash, name, permissions, created_at, expires_at, last_used_at, created_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
        )
        .bind(key_id)
        .bind(tenant_id)
        .bind(key_hash)
        .bind(name)
        .bind(permissions)
        .bind(created_at)
        .bind(expires_at)
        .bind(last_used_at)
        .bind(created_by)
        .execute(executor)
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                CoreError::already_exists("api_key", key_hash)
            } else if is_foreign_key_violation(&e) {
                CoreError::invalid_state(format!("tenant {} does not exist", api_key.tenant_id))
            } else {
                CoreError::internal(e.to_string())
            }
        })?;

        Ok(())
    }

    /// Get an API key using the provided executor.
    async fn get_with_executor<'e, E>(
        &self,
        key_id: ApiKeyId,
        executor: E,
    ) -> CoreResult<Option<ApiKeyDescriptor>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let key_id_bytes = key_id.to_bytes().to_vec();

        let row = query(
            "SELECT key_id, tenant_id, name, permissions, created_at, expires_at, last_used_at, created_by
             FROM api_keys WHERE key_id = ?1"
        )
        .bind(key_id_bytes)
        .fetch_optional(executor)
        .await
        .map_err(|e| CoreError::internal(e.to_string()))?;

        row.map(|r| parse_api_key_row(&r)).transpose()
    }

    /// Get an API key by hash using the provided executor.
    async fn get_by_hash_with_executor<'e, E>(
        &self,
        key_hash: &str,
        executor: E,
    ) -> CoreResult<Option<ApiKeyDescriptor>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let row = query(
            "SELECT key_id, tenant_id, name, permissions, created_at, expires_at, last_used_at, created_by
             FROM api_keys WHERE key_hash = ?1"
        )
        .bind(key_hash)
        .fetch_optional(executor)
        .await
        .map_err(|e| CoreError::internal(e.to_string()))?;

        row.map(|r| parse_api_key_row(&r)).transpose()
    }

    /// Delete an API key using the provided executor.
    async fn delete_with_executor<'e, E>(&self, key_id: ApiKeyId, executor: E) -> CoreResult<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let key_id_bytes = key_id.to_bytes().to_vec();

        let result = query("DELETE FROM api_keys WHERE key_id = ?1")
            .bind(key_id_bytes)
            .execute(executor)
            .await
            .map_err(|e| CoreError::internal(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(CoreError::not_found("api_key", key_id.to_string()));
        }

        Ok(())
    }
}

#[async_trait]
impl ApiKeyRepository for SqliteApiKeyRepository {
    async fn create(&self, api_key: &ApiKeyDescriptor, key_hash: &str) -> CoreResult<()> {
        self.create_with_executor(api_key, key_hash, &self.pool)
            .await
    }

    async fn get(&self, key_id: ApiKeyId) -> CoreResult<Option<ApiKeyDescriptor>> {
        self.get_with_executor(key_id, &self.pool).await
    }

    async fn get_by_hash(&self, key_hash: &str) -> CoreResult<Option<ApiKeyDescriptor>> {
        self.get_by_hash_with_executor(key_hash, &self.pool).await
    }

    async fn list_by_tenant(&self, tenant_id: TenantId) -> CoreResult<Vec<ApiKeyDescriptor>> {
        let tenant_id_bytes = tenant_id.to_bytes().to_vec();

        let rows = query(
            "SELECT key_id, tenant_id, name, permissions, created_at, expires_at, last_used_at, created_by
             FROM api_keys WHERE tenant_id = ?1 ORDER BY created_at DESC"
        )
        .bind(tenant_id_bytes)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::internal(e.to_string()))?;

        rows.iter().map(parse_api_key_row).collect()
    }

    async fn delete(&self, key_id: ApiKeyId) -> CoreResult<()> {
        self.delete_with_executor(key_id, &self.pool).await
    }

    async fn update_last_used(&self, key_id: ApiKeyId) -> CoreResult<()> {
        let key_id_bytes = key_id.to_bytes().to_vec();
        let now = Utc::now().to_rfc3339();

        query("UPDATE api_keys SET last_used_at = ?1 WHERE key_id = ?2")
            .bind(now)
            .bind(key_id_bytes)
            .execute(&self.pool)
            .await
            .map_err(|e| CoreError::internal(e.to_string()))?;

        Ok(())
    }
}

/// Parses a SQLite row into an `ApiKeyDescriptor`.
fn parse_api_key_row(row: &sqlx::sqlite::SqliteRow) -> CoreResult<ApiKeyDescriptor> {
    let key_id_bytes: Vec<u8> = row
        .try_get("key_id")
        .map_err(|e| CoreError::internal(format!("Failed to get key_id: {e}")))?;
    let key_id = ApiKeyId::from_bytes(&key_id_bytes)
        .map_err(|e| CoreError::internal(format!("Invalid key_id: {e}")))?;

    let tenant_id_bytes: Vec<u8> = row
        .try_get("tenant_id")
        .map_err(|e| CoreError::internal(format!("Failed to get tenant_id: {e}")))?;
    let tenant_id = TenantId::from_bytes(&tenant_id_bytes)
        .map_err(|e| CoreError::internal(format!("Invalid tenant_id: {e}")))?;

    let created_by: Option<Vec<u8>> = row
        .try_get("created_by")
        .map_err(|e| CoreError::internal(format!("Failed to get created_by: {e}")))?;
    let created_by = created_by
        .map(|bytes| UserId::from_bytes(&bytes))
        .transpose()
        .map_err(|e| CoreError::internal(format!("Invalid created_by: {e}")))?;

    let permissions_json: String = row
        .try_get("permissions")
        .map_err(|e| CoreError::internal(format!("Failed to get permissions: {e}")))?;
    let permissions: Vec<String> = serde_json::from_str(&permissions_json)
        .map_err(|e| CoreError::internal(format!("Failed to parse permissions: {e}")))?;

    let created_at_str: String = row
        .try_get("created_at")
        .map_err(|e| CoreError::internal(format!("Failed to get created_at: {e}")))?;
    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map_err(|e| CoreError::internal(format!("Invalid created_at: {e}")))?
        .with_timezone(&Utc);

    let expires_at_str: Option<String> = row
        .try_get("expires_at")
        .map_err(|e| CoreError::internal(format!("Failed to get expires_at: {e}")))?;
    let expires_at = expires_at_str
        .map(|s| DateTime::parse_from_rfc3339(&s))
        .transpose()
        .map_err(|e| CoreError::internal(format!("Invalid expires_at: {e}")))?
        .map(|dt| dt.with_timezone(&Utc));

    let last_used_at_str: Option<String> = row
        .try_get("last_used_at")
        .map_err(|e| CoreError::internal(format!("Failed to get last_used_at: {e}")))?;
    let last_used_at = last_used_at_str
        .map(|s| DateTime::parse_from_rfc3339(&s))
        .transpose()
        .map_err(|e| CoreError::internal(format!("Invalid last_used_at: {e}")))?
        .map(|dt| dt.with_timezone(&Utc));

    let name: String = row
        .try_get("name")
        .map_err(|e| CoreError::internal(format!("Failed to get name: {e}")))?;

    Ok(ApiKeyDescriptor {
        key_id,
        tenant_id,
        name,
        permissions,
        created_at,
        expires_at,
        last_used_at,
        created_by,
    })
}

/// Checks if the error is a unique constraint violation.
fn is_unique_violation(err: &sqlx::Error) -> bool {
    if let sqlx::Error::Database(db_err) = err {
        db_err.message().contains("UNIQUE constraint failed")
    } else {
        false
    }
}

/// Checks if the error is a foreign key constraint violation.
fn is_foreign_key_violation(err: &sqlx::Error) -> bool {
    if let sqlx::Error::Database(db_err) = err {
        db_err.message().contains("FOREIGN KEY constraint failed")
    } else {
        false
    }
}
