use std::str::FromStr;

use akidb_core::{CoreError, CoreResult, TenantDescriptor, TenantId, TenantQuota, TenantStatus};
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::Value;
use sqlx::sqlite::SqliteRow;
use sqlx::{query, Executor, Row, Sqlite, SqlitePool};

/// SQLite-backed implementation of the tenant catalog.
pub struct SqliteTenantCatalog {
    pool: SqlitePool,
}

impl SqliteTenantCatalog {
    /// Creates a new catalog backed by the provided pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Provides access to the underlying pool, primarily for testing.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Inserts a tenant using the supplied executor (pool or transaction).
    pub async fn create_with_executor<'e, E>(
        executor: E,
        tenant: &TenantDescriptor,
    ) -> CoreResult<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let tenant_id = tenant.tenant_id.to_bytes().to_vec();
        let name = &tenant.name;
        let slug = &tenant.slug;
        let status = tenant.status.as_str();
        let memory_quota_bytes = i64::try_from(tenant.quotas.memory_quota_bytes)
            .map_err(|_| CoreError::invalid_state("memory_quota_bytes exceeds 63-bit range"))?;
        let storage_quota_bytes = i64::try_from(tenant.quotas.storage_quota_bytes)
            .map_err(|_| CoreError::invalid_state("storage_quota_bytes exceeds 63-bit range"))?;
        let qps_quota = i64::from(tenant.quotas.qps_quota);
        let metadata = serde_json::to_string(&tenant.metadata)
            .map_err(|err| CoreError::internal(err.to_string()))?;
        let created_at = tenant
            .created_at
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        let updated_at = tenant
            .updated_at
            .to_rfc3339_opts(SecondsFormat::Millis, true);

        sqlx::query!(
            r#"
            INSERT INTO tenants (
                tenant_id,
                name,
                slug,
                status,
                memory_quota_bytes,
                storage_quota_bytes,
                qps_quota,
                metadata,
                created_at,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            tenant_id,
            name,
            slug,
            status,
            memory_quota_bytes,
            storage_quota_bytes,
            qps_quota,
            metadata,
            created_at,
            updated_at
        )
        .execute(executor)
        .await
        .map(|_| ())
        .map_err(|err| map_sqlx_error("tenant", tenant.tenant_id.to_string(), err))
    }

    /// Updates a tenant using the supplied executor (pool or transaction).
    pub async fn update_with_executor<'e, E>(
        executor: E,
        tenant: &TenantDescriptor,
    ) -> CoreResult<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let tenant_id = tenant.tenant_id.to_bytes().to_vec();
        let name = &tenant.name;
        let slug = &tenant.slug;
        let status = tenant.status.as_str();
        let memory_quota_bytes = i64::try_from(tenant.quotas.memory_quota_bytes)
            .map_err(|_| CoreError::invalid_state("memory_quota_bytes exceeds 63-bit range"))?;
        let storage_quota_bytes = i64::try_from(tenant.quotas.storage_quota_bytes)
            .map_err(|_| CoreError::invalid_state("storage_quota_bytes exceeds 63-bit range"))?;
        let qps_quota = i64::from(tenant.quotas.qps_quota);
        let metadata = serde_json::to_string(&tenant.metadata)
            .map_err(|err| CoreError::internal(err.to_string()))?;
        let updated_at = tenant
            .updated_at
            .to_rfc3339_opts(SecondsFormat::Millis, true);

        let rows = sqlx::query!(
            r#"
            UPDATE tenants
               SET name = ?2,
                   slug = ?3,
                   status = ?4,
                   memory_quota_bytes = ?5,
                   storage_quota_bytes = ?6,
                   qps_quota = ?7,
                   metadata = ?8,
                   updated_at = ?9
             WHERE tenant_id = ?1
            "#,
            tenant_id,
            name,
            slug,
            status,
            memory_quota_bytes,
            storage_quota_bytes,
            qps_quota,
            metadata,
            updated_at
        )
        .execute(executor)
        .await
        .map_err(|err| map_sqlx_error("tenant", tenant.tenant_id.to_string(), err))?;

        if rows.rows_affected() == 0 {
            return Err(CoreError::not_found("tenant", tenant.tenant_id.to_string()));
        }
        Ok(())
    }

    /// Deletes a tenant using the supplied executor (pool or transaction).
    pub async fn delete_with_executor<'e, E>(executor: E, tenant_id: TenantId) -> CoreResult<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let tenant_id_bytes = tenant_id.to_bytes().to_vec();
        let rows = query!(
            r#"
            DELETE FROM tenants
             WHERE tenant_id = ?1
            "#,
            tenant_id_bytes
        )
        .execute(executor)
        .await
        .map_err(|err| map_sqlx_error("tenant", tenant_id.to_string(), err))?;

        if rows.rows_affected() == 0 {
            return Err(CoreError::not_found("tenant", tenant_id.to_string()));
        }
        Ok(())
    }

    fn map_row(row: SqliteRow) -> CoreResult<TenantDescriptor> {
        let bytes: Vec<u8> = row.get("tenant_id");
        let tenant_id =
            TenantId::from_bytes(&bytes).map_err(|err| CoreError::internal(err.to_string()))?;
        let name: String = row.get("name");
        let slug: String = row.get("slug");
        let status: String = row.get("status");
        let status = TenantStatus::from_str(&status)
            .map_err(|_| CoreError::invalid_state(format!("unknown tenant status `{status}`")))?;

        let memory_quota_bytes: i64 = row.get("memory_quota_bytes");
        let storage_quota_bytes: i64 = row.get("storage_quota_bytes");
        let qps_quota: i64 = row.get("qps_quota");

        let quotas = TenantQuota {
            memory_quota_bytes: u64::try_from(memory_quota_bytes).map_err(|_| {
                CoreError::invalid_state("memory_quota_bytes stored negative value")
            })?,
            storage_quota_bytes: u64::try_from(storage_quota_bytes).map_err(|_| {
                CoreError::invalid_state("storage_quota_bytes stored negative value")
            })?,
            qps_quota: u32::try_from(qps_quota)
                .map_err(|_| CoreError::invalid_state("qps_quota stored negative value"))?,
        };

        let metadata: Option<String> = row.get("metadata");
        let metadata = metadata
            .map(|json| serde_json::from_str(&json))
            .transpose()
            .map_err(|err| CoreError::internal(err.to_string()))?
            .unwrap_or_else(|| Value::Object(serde_json::Map::new()));

        let created_at: String = row.get("created_at");
        let updated_at: String = row.get("updated_at");
        let created_at = DateTime::parse_from_rfc3339(&created_at)
            .map_err(|err| CoreError::internal(format!("invalid created_at: {err}")))?
            .with_timezone(&Utc);
        let updated_at = DateTime::parse_from_rfc3339(&updated_at)
            .map_err(|err| CoreError::internal(format!("invalid updated_at: {err}")))?
            .with_timezone(&Utc);

        Ok(TenantDescriptor {
            tenant_id,
            name,
            slug,
            status,
            quotas,
            metadata,
            created_at,
            updated_at,
        })
    }
}

#[async_trait::async_trait]
impl akidb_core::TenantCatalog for SqliteTenantCatalog {
    async fn list(&self) -> CoreResult<Vec<TenantDescriptor>> {
        let rows = query(
            r#"
            SELECT tenant_id,
                   name,
                   slug,
                   status,
                   memory_quota_bytes,
                   storage_quota_bytes,
                   qps_quota,
                   metadata,
                   created_at,
                   updated_at
              FROM tenants
          ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|err| CoreError::internal(err.to_string()))?;

        rows.into_iter().map(SqliteTenantCatalog::map_row).collect()
    }

    async fn get(&self, tenant_id: TenantId) -> CoreResult<Option<TenantDescriptor>> {
        let row = query(
            r#"
            SELECT tenant_id,
                   name,
                   slug,
                   status,
                   memory_quota_bytes,
                   storage_quota_bytes,
                   qps_quota,
                   metadata,
                   created_at,
                   updated_at
              FROM tenants
             WHERE tenant_id = ?1
            "#,
        )
        .bind(tenant_id.to_bytes().to_vec())
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| CoreError::internal(err.to_string()))?;

        match row {
            Some(row) => Ok(Some(Self::map_row(row)?)),
            None => Ok(None),
        }
    }

    async fn create(&self, tenant: &TenantDescriptor) -> CoreResult<()> {
        Self::create_with_executor(&self.pool, tenant).await
    }

    async fn update(&self, tenant: &TenantDescriptor) -> CoreResult<()> {
        Self::update_with_executor(&self.pool, tenant).await
    }

    async fn delete(&self, tenant_id: TenantId) -> CoreResult<()> {
        Self::delete_with_executor(&self.pool, tenant_id).await
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
