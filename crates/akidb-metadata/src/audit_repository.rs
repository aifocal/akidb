//! SQLite implementation of audit log repository for compliance and security monitoring.

use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{query, Row, SqlitePool};

use akidb_core::{
    Action, AuditLogEntry, AuditLogId, AuditLogRepository, AuditResult, CoreError, CoreResult,
    TenantId, UserId,
};

/// SQLite implementation of the audit log repository.
pub struct SqliteAuditLogRepository {
    pool: SqlitePool,
}

impl SqliteAuditLogRepository {
    /// Creates a new SQLite audit log repository.
    #[must_use]
    pub const fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuditLogRepository for SqliteAuditLogRepository {
    async fn create(&self, entry: &AuditLogEntry) -> CoreResult<()> {
        let audit_log_id = entry.audit_log_id.to_bytes().to_vec();
        let tenant_id = entry.tenant_id.to_bytes().to_vec();
        let user_id = entry.user_id.map(|id| id.to_bytes().to_vec());
        let action = entry.action.as_str();
        let resource_type = &entry.resource_type;
        let resource_id = &entry.resource_id;
        let result = entry.result.as_str();
        let reason = &entry.reason;
        let metadata = entry.metadata.as_ref().map(|m| m.to_string());
        let ip_address = &entry.ip_address;
        let user_agent = &entry.user_agent;
        let created_at = entry.created_at.to_rfc3339();

        query(
            "INSERT INTO audit_logs (audit_log_id, tenant_id, user_id, action, resource_type, resource_id, result, reason, metadata, ip_address, user_agent, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"
        )
        .bind(audit_log_id)
        .bind(tenant_id)
        .bind(user_id)
        .bind(action)
        .bind(resource_type)
        .bind(resource_id)
        .bind(result)
        .bind(reason)
        .bind(metadata)
        .bind(ip_address)
        .bind(user_agent)
        .bind(created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::internal(e.to_string()))?;

        Ok(())
    }

    async fn list_by_tenant(
        &self,
        tenant_id: TenantId,
        limit: usize,
        offset: usize,
    ) -> CoreResult<Vec<AuditLogEntry>> {
        let tenant_id_bytes = tenant_id.to_bytes().to_vec();

        let rows = query(
            "SELECT audit_log_id, tenant_id, user_id, action, resource_type, resource_id, result, reason, metadata, ip_address, user_agent, created_at
             FROM audit_logs WHERE tenant_id = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3"
        )
        .bind(tenant_id_bytes)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::internal(e.to_string()))?;

        rows.iter().map(parse_audit_log_row).collect()
    }

    async fn list_by_user(
        &self,
        user_id: UserId,
        limit: usize,
        offset: usize,
    ) -> CoreResult<Vec<AuditLogEntry>> {
        let user_id_bytes = user_id.to_bytes().to_vec();

        let rows = query(
            "SELECT audit_log_id, tenant_id, user_id, action, resource_type, resource_id, result, reason, metadata, ip_address, user_agent, created_at
             FROM audit_logs WHERE user_id = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3"
        )
        .bind(user_id_bytes)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::internal(e.to_string()))?;

        rows.iter().map(parse_audit_log_row).collect()
    }
}

/// Parse an audit log row from SQLite.
fn parse_audit_log_row(row: &sqlx::sqlite::SqliteRow) -> CoreResult<AuditLogEntry> {
    let audit_log_id_bytes: Vec<u8> = row.try_get("audit_log_id").map_err(|e| CoreError::internal(e.to_string()))?;
    let tenant_id_bytes: Vec<u8> = row.try_get("tenant_id").map_err(|e| CoreError::internal(e.to_string()))?;
    let user_id_bytes: Option<Vec<u8>> = row.try_get("user_id").map_err(|e| CoreError::internal(e.to_string()))?;
    let action_str: String = row.try_get("action").map_err(|e| CoreError::internal(e.to_string()))?;
    let resource_type: String = row.try_get("resource_type").map_err(|e| CoreError::internal(e.to_string()))?;
    let resource_id: String = row.try_get("resource_id").map_err(|e| CoreError::internal(e.to_string()))?;
    let result_str: String = row.try_get("result").map_err(|e| CoreError::internal(e.to_string()))?;
    let reason: Option<String> = row.try_get("reason").map_err(|e| CoreError::internal(e.to_string()))?;
    let metadata_str: Option<String> = row.try_get("metadata").map_err(|e| CoreError::internal(e.to_string()))?;
    let ip_address: Option<String> = row.try_get("ip_address").map_err(|e| CoreError::internal(e.to_string()))?;
    let user_agent: Option<String> = row.try_get("user_agent").map_err(|e| CoreError::internal(e.to_string()))?;
    let created_at_str: String = row.try_get("created_at").map_err(|e| CoreError::internal(e.to_string()))?;

    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map_err(|e| CoreError::internal(e.to_string()))?
        .with_timezone(&Utc);

    let audit_log_id = AuditLogId::from_bytes(&audit_log_id_bytes).map_err(|e| CoreError::internal(e.to_string()))?;
    let tenant_id = TenantId::from_bytes(&tenant_id_bytes).map_err(|e| CoreError::internal(e.to_string()))?;
    let user_id = user_id_bytes
        .map(|bytes| UserId::from_bytes(&bytes))
        .transpose()
        .map_err(|e| CoreError::internal(e.to_string()))?;
    let action = Action::from_str(&action_str).map_err(CoreError::invalid_state)?;
    let result = AuditResult::from_str(&result_str).map_err(CoreError::invalid_state)?;
    let metadata = metadata_str
        .map(|s| serde_json::from_str(&s))
        .transpose()
        .map_err(|e| CoreError::internal(e.to_string()))?;

    Ok(AuditLogEntry {
        audit_log_id,
        tenant_id,
        user_id,
        action,
        resource_type,
        resource_id,
        result,
        reason,
        metadata,
        ip_address,
        user_agent,
        created_at,
    })
}
