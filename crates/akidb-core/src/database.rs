use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::ids::{DatabaseId, TenantId};

/// Provisioning lifecycle state for a logical database.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseState {
    /// Database metadata is being created and not yet accessible.
    Provisioning,
    /// Database is ready to serve read/write traffic.
    Ready,
    /// Database schema is undergoing migration.
    Migrating,
    /// Database is being decommissioned and may reject new writes.
    Deleting,
}

impl DatabaseState {
    /// Returns the canonical lowercase string persisted in SQLite.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Provisioning => "provisioning",
            Self::Ready => "ready",
            Self::Migrating => "migrating",
            Self::Deleting => "deleting",
        }
    }
}

impl Default for DatabaseState {
    fn default() -> Self {
        Self::Provisioning
    }
}

impl FromStr for DatabaseState {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "provisioning" => Ok(Self::Provisioning),
            "ready" => Ok(Self::Ready),
            "migrating" => Ok(Self::Migrating),
            "deleting" => Ok(Self::Deleting),
            _ => Err(()),
        }
    }
}

/// Metadata describing a logical database within a tenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseDescriptor {
    /// Stable database identifier.
    pub database_id: DatabaseId,
    /// Owning tenant identifier.
    pub tenant_id: TenantId,
    /// Human-readable name for the database.
    pub name: String,
    /// Optional description supplied by operators.
    pub description: Option<String>,
    /// Lifecycle state of the database.
    pub state: DatabaseState,
    /// Schema version of the collection topology.
    pub schema_version: i64,
    /// Creation timestamp in UTC.
    pub created_at: DateTime<Utc>,
    /// Update timestamp in UTC.
    pub updated_at: DateTime<Utc>,
}

impl DatabaseDescriptor {
    /// Constructs a new database descriptor with default provisioning state.
    #[must_use]
    pub fn new(tenant_id: TenantId, name: impl Into<String>, description: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            database_id: DatabaseId::new(),
            tenant_id,
            name: name.into(),
            description,
            state: DatabaseState::default(),
            schema_version: 1,
            created_at: now,
            updated_at: now,
        }
    }

    /// Updates the state and refreshes the `updated_at` timestamp.
    pub fn transition_to(&mut self, state: DatabaseState) {
        self.state = state;
        self.touch();
    }

    /// Updates the `updated_at` timestamp to the current time.
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}
