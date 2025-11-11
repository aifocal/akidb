//! SQLite implementation of user repository for user management and authentication.

use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{query, Executor, Row, Sqlite, SqlitePool};

use akidb_core::{
    CoreError, CoreResult, Role, TenantId, UserDescriptor, UserId, UserRepository, UserStatus,
};

/// SQLite implementation of the user repository.
pub struct SqliteUserRepository {
    pool: SqlitePool,
}

impl SqliteUserRepository {
    /// Creates a new SQLite user repository.
    #[must_use]
    pub const fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a user using the provided executor (for transaction support).
    async fn create_with_executor<'e, E>(
        &self,
        user: &UserDescriptor,
        executor: E,
    ) -> CoreResult<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let user_id = user.user_id.to_bytes().to_vec();
        let tenant_id = user.tenant_id.to_bytes().to_vec();
        let email = &user.email;
        let password_hash = &user.password_hash;
        let role = user.role.as_str();
        let status = user.status.as_str();
        let created_at = user.created_at.to_rfc3339();
        let updated_at = user.updated_at.to_rfc3339();
        let last_login_at = user.last_login_at.map(|t| t.to_rfc3339());

        query(
            "INSERT INTO users (user_id, tenant_id, email, password_hash, role, status, created_at, updated_at, last_login_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
        )
        .bind(user_id)
        .bind(tenant_id)
        .bind(email)
        .bind(password_hash)
        .bind(role)
        .bind(status)
        .bind(created_at)
        .bind(updated_at)
        .bind(last_login_at)
        .execute(executor)
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                CoreError::already_exists("user", email)
            } else if is_foreign_key_violation(&e) {
                CoreError::invalid_state(format!("tenant {} does not exist", user.tenant_id))
            } else {
                CoreError::internal(e.to_string())
            }
        })?;

        Ok(())
    }

    /// Get a user using the provided executor.
    async fn get_with_executor<'e, E>(
        &self,
        user_id: UserId,
        executor: E,
    ) -> CoreResult<Option<UserDescriptor>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let user_id_bytes = user_id.to_bytes().to_vec();

        let row = query(
            "SELECT user_id, tenant_id, email, password_hash, role, status, created_at, updated_at, last_login_at
             FROM users WHERE user_id = ?1"
        )
        .bind(user_id_bytes)
        .fetch_optional(executor)
        .await
        .map_err(|e| CoreError::internal(e.to_string()))?;

        row.map(|r| parse_user_row(&r)).transpose()
    }

    /// Update a user using the provided executor.
    async fn update_with_executor<'e, E>(
        &self,
        user: &UserDescriptor,
        executor: E,
    ) -> CoreResult<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let user_id = user.user_id.to_bytes().to_vec();
        let email = &user.email;
        let password_hash = &user.password_hash;
        let role = user.role.as_str();
        let status = user.status.as_str();
        let updated_at = user.updated_at.to_rfc3339();
        let last_login_at = user.last_login_at.map(|t| t.to_rfc3339());

        query(
            "UPDATE users SET email = ?1, password_hash = ?2, role = ?3, status = ?4, updated_at = ?5, last_login_at = ?6
             WHERE user_id = ?7"
        )
        .bind(email)
        .bind(password_hash)
        .bind(role)
        .bind(status)
        .bind(updated_at)
        .bind(last_login_at)
        .bind(user_id)
        .execute(executor)
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                CoreError::already_exists("user", email)
            } else {
                CoreError::internal(e.to_string())
            }
        })?;

        Ok(())
    }

    /// Delete a user using the provided executor.
    async fn delete_with_executor<'e, E>(&self, user_id: UserId, executor: E) -> CoreResult<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let user_id_bytes = user_id.to_bytes().to_vec();

        query("DELETE FROM users WHERE user_id = ?1")
            .bind(user_id_bytes)
            .execute(executor)
            .await
            .map_err(|e| CoreError::internal(e.to_string()))?;

        Ok(())
    }
}

#[async_trait]
impl UserRepository for SqliteUserRepository {
    async fn create(&self, user: &UserDescriptor) -> CoreResult<()> {
        self.create_with_executor(user, &self.pool).await
    }

    async fn get(&self, user_id: UserId) -> CoreResult<Option<UserDescriptor>> {
        self.get_with_executor(user_id, &self.pool).await
    }

    async fn get_by_email(
        &self,
        tenant_id: TenantId,
        email: &str,
    ) -> CoreResult<Option<UserDescriptor>> {
        let tenant_id_bytes = tenant_id.to_bytes().to_vec();

        let row = query(
            "SELECT user_id, tenant_id, email, password_hash, role, status, created_at, updated_at, last_login_at
             FROM users WHERE tenant_id = ?1 AND email = ?2"
        )
        .bind(tenant_id_bytes)
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CoreError::internal(e.to_string()))?;

        row.map(|r| parse_user_row(&r)).transpose()
    }

    async fn list_by_tenant(&self, tenant_id: TenantId) -> CoreResult<Vec<UserDescriptor>> {
        let tenant_id_bytes = tenant_id.to_bytes().to_vec();

        let rows = query(
            "SELECT user_id, tenant_id, email, password_hash, role, status, created_at, updated_at, last_login_at
             FROM users WHERE tenant_id = ?1 ORDER BY created_at ASC"
        )
        .bind(tenant_id_bytes)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::internal(e.to_string()))?;

        rows.iter().map(parse_user_row).collect()
    }

    async fn update(&self, user: &UserDescriptor) -> CoreResult<()> {
        self.update_with_executor(user, &self.pool).await
    }

    async fn delete(&self, user_id: UserId) -> CoreResult<()> {
        self.delete_with_executor(user_id, &self.pool).await
    }
}

/// Parse a user row from SQLite.
fn parse_user_row(row: &sqlx::sqlite::SqliteRow) -> CoreResult<UserDescriptor> {
    let user_id_bytes: Vec<u8> = row
        .try_get("user_id")
        .map_err(|e| CoreError::internal(e.to_string()))?;
    let tenant_id_bytes: Vec<u8> = row
        .try_get("tenant_id")
        .map_err(|e| CoreError::internal(e.to_string()))?;
    let email: String = row
        .try_get("email")
        .map_err(|e| CoreError::internal(e.to_string()))?;
    let password_hash: String = row
        .try_get("password_hash")
        .map_err(|e| CoreError::internal(e.to_string()))?;
    let role_str: String = row
        .try_get("role")
        .map_err(|e| CoreError::internal(e.to_string()))?;
    let status_str: String = row
        .try_get("status")
        .map_err(|e| CoreError::internal(e.to_string()))?;
    let created_at_str: String = row
        .try_get("created_at")
        .map_err(|e| CoreError::internal(e.to_string()))?;
    let updated_at_str: String = row
        .try_get("updated_at")
        .map_err(|e| CoreError::internal(e.to_string()))?;
    let last_login_at_str: Option<String> = row
        .try_get("last_login_at")
        .map_err(|e| CoreError::internal(e.to_string()))?;

    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map_err(|e| CoreError::internal(e.to_string()))?
        .with_timezone(&Utc);
    let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
        .map_err(|e| CoreError::internal(e.to_string()))?
        .with_timezone(&Utc);
    let last_login_at = last_login_at_str
        .map(|s| DateTime::parse_from_rfc3339(&s))
        .transpose()
        .map_err(|e| CoreError::internal(e.to_string()))?
        .map(|dt| dt.with_timezone(&Utc));

    let user_id =
        UserId::from_bytes(&user_id_bytes).map_err(|e| CoreError::internal(e.to_string()))?;
    let tenant_id =
        TenantId::from_bytes(&tenant_id_bytes).map_err(|e| CoreError::internal(e.to_string()))?;
    let role = Role::from_str(&role_str).map_err(CoreError::invalid_state)?;
    let status = UserStatus::from_str(&status_str).map_err(CoreError::invalid_state)?;

    Ok(UserDescriptor {
        user_id,
        tenant_id,
        email,
        password_hash,
        role,
        status,
        created_at,
        updated_at,
        last_login_at,
    })
}

/// Check if an error is a unique constraint violation.
fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(
        err,
        sqlx::Error::Database(ref db_err) if db_err.code().as_deref() == Some("2067")  // SQLITE_CONSTRAINT_UNIQUE
    )
}

/// Check if an error is a foreign key constraint violation.
fn is_foreign_key_violation(err: &sqlx::Error) -> bool {
    matches!(
        err,
        sqlx::Error::Database(ref db_err) if db_err.code().as_deref() == Some("787")  // SQLITE_CONSTRAINT_FOREIGNKEY
    )
}
