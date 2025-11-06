//! User domain model for multi-tenant user management and RBAC.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::ids::{TenantId, UserId};

/// User within a tenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDescriptor {
    pub user_id: UserId,
    pub tenant_id: TenantId,
    pub email: String,
    pub password_hash: String, // Argon2 hash
    pub role: Role,
    pub status: UserStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
}

/// User status lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserStatus {
    /// User is active and can authenticate.
    Active,
    /// User is temporarily suspended (can be reactivated).
    Suspended,
    /// User is deactivated (requires admin intervention).
    Deactivated,
}

/// User roles with predefined permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    /// Full access to tenant resources (create users, manage databases, etc.).
    Admin,
    /// Can create/read/update/delete collections and documents.
    Developer,
    /// Read-only access to collections and documents.
    Viewer,
    /// Read-only access to audit logs and metrics.
    Auditor,
}

/// Actions that can be performed on resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    // User management
    UserCreate,
    UserRead,
    UserUpdate,
    UserDelete,

    // Database management
    DatabaseCreate,
    DatabaseRead,
    DatabaseUpdate,
    DatabaseDelete,

    // Collection management
    CollectionCreate,
    CollectionRead,
    CollectionUpdate,
    CollectionDelete,

    // Document operations
    DocumentInsert,
    DocumentSearch,
    DocumentUpdate,
    DocumentDelete,

    // Audit logs
    AuditRead,
}

impl UserDescriptor {
    /// Create a new user descriptor.
    #[must_use]
    pub fn new(tenant_id: TenantId, email: impl Into<String>, role: Role) -> Self {
        let now = Utc::now();
        Self {
            user_id: UserId::new(),
            tenant_id,
            email: email.into(),
            password_hash: String::new(), // Set via set_password()
            role,
            status: UserStatus::Active,
            created_at: now,
            updated_at: now,
            last_login_at: None,
        }
    }

    /// Update the last login timestamp.
    pub fn record_login(&mut self) {
        self.last_login_at = Some(Utc::now());
        self.touch();
    }

    /// Transition to a new status.
    pub fn transition_to(&mut self, status: UserStatus) {
        self.status = status;
        self.touch();
    }

    /// Update the updated_at timestamp.
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }

    /// Check if user has permission to perform an action.
    #[must_use]
    pub fn has_permission(&self, action: Action) -> bool {
        if self.status != UserStatus::Active {
            return false;
        }

        match self.role {
            Role::Admin => true, // Admin has all permissions
            Role::Developer => matches!(
                action,
                Action::DatabaseRead
                    | Action::CollectionCreate
                    | Action::CollectionRead
                    | Action::CollectionUpdate
                    | Action::CollectionDelete
                    | Action::DocumentInsert
                    | Action::DocumentSearch
                    | Action::DocumentUpdate
                    | Action::DocumentDelete
            ),
            Role::Viewer => matches!(
                action,
                Action::DatabaseRead | Action::CollectionRead | Action::DocumentSearch
            ),
            Role::Auditor => matches!(
                action,
                Action::DatabaseRead | Action::CollectionRead | Action::AuditRead
            ),
        }
    }
}

impl Role {
    /// Convert role to string representation.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Developer => "developer",
            Role::Viewer => "viewer",
            Role::Auditor => "auditor",
        }
    }
}

impl FromStr for Role {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "admin" => Ok(Role::Admin),
            "developer" => Ok(Role::Developer),
            "viewer" => Ok(Role::Viewer),
            "auditor" => Ok(Role::Auditor),
            _ => Err(format!("invalid role: {s}")),
        }
    }
}

impl UserStatus {
    /// Convert status to string representation.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            UserStatus::Active => "active",
            UserStatus::Suspended => "suspended",
            UserStatus::Deactivated => "deactivated",
        }
    }
}

impl FromStr for UserStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(UserStatus::Active),
            "suspended" => Ok(UserStatus::Suspended),
            "deactivated" => Ok(UserStatus::Deactivated),
            _ => Err(format!("invalid user status: {s}")),
        }
    }
}

impl Action {
    /// Convert action to string representation.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Action::UserCreate => "user::create",
            Action::UserRead => "user::read",
            Action::UserUpdate => "user::update",
            Action::UserDelete => "user::delete",
            Action::DatabaseCreate => "database::create",
            Action::DatabaseRead => "database::read",
            Action::DatabaseUpdate => "database::update",
            Action::DatabaseDelete => "database::delete",
            Action::CollectionCreate => "collection::create",
            Action::CollectionRead => "collection::read",
            Action::CollectionUpdate => "collection::update",
            Action::CollectionDelete => "collection::delete",
            Action::DocumentInsert => "document::insert",
            Action::DocumentSearch => "document::search",
            Action::DocumentUpdate => "document::update",
            Action::DocumentDelete => "document::delete",
            Action::AuditRead => "audit::read",
        }
    }
}

impl FromStr for Action {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "user::create" => Ok(Action::UserCreate),
            "user::read" => Ok(Action::UserRead),
            "user::update" => Ok(Action::UserUpdate),
            "user::delete" => Ok(Action::UserDelete),
            "database::create" => Ok(Action::DatabaseCreate),
            "database::read" => Ok(Action::DatabaseRead),
            "database::update" => Ok(Action::DatabaseUpdate),
            "database::delete" => Ok(Action::DatabaseDelete),
            "collection::create" => Ok(Action::CollectionCreate),
            "collection::read" => Ok(Action::CollectionRead),
            "collection::update" => Ok(Action::CollectionUpdate),
            "collection::delete" => Ok(Action::CollectionDelete),
            "document::insert" => Ok(Action::DocumentInsert),
            "document::search" => Ok(Action::DocumentSearch),
            "document::update" => Ok(Action::DocumentUpdate),
            "document::delete" => Ok(Action::DocumentDelete),
            "audit::read" => Ok(Action::AuditRead),
            _ => Err(format!("invalid action: {s}")),
        }
    }
}
