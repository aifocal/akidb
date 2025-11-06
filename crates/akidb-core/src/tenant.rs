use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;

use crate::ids::TenantId;

/// Provisioning state of a tenant account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TenantStatus {
    /// Tenant is being created and not yet ready for workloads.
    Provisioning,
    /// Tenant is active and fully provisioned.
    Active,
    /// Tenant is suspended due to quota enforcement or policy violation.
    Suspended,
    /// Tenant has been decommissioned and is pending archival.
    Decommissioned,
}

impl TenantStatus {
    /// Returns the canonical lowercase string stored in SQLite.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Provisioning => "provisioning",
            Self::Active => "active",
            Self::Suspended => "suspended",
            Self::Decommissioned => "decommissioned",
        }
    }
}

impl Default for TenantStatus {
    fn default() -> Self {
        Self::Provisioning
    }
}

impl FromStr for TenantStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "provisioning" => Ok(Self::Provisioning),
            "active" => Ok(Self::Active),
            "suspended" => Ok(Self::Suspended),
            "decommissioned" => Ok(Self::Decommissioned),
            _ => Err(()),
        }
    }
}

/// Resource quota allocations enforced per tenant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenantQuota {
    /// Maximum in-memory footprint allowed for the tenant in bytes.
    pub memory_quota_bytes: u64,
    /// Maximum durable storage footprint allotted to the tenant in bytes.
    pub storage_quota_bytes: u64,
    /// Steady-state permitted queries per second for the tenant.
    pub qps_quota: u32,
}

impl TenantQuota {
    /// Default memory allocation (32 GiB).
    pub const DEFAULT_MEMORY_BYTES: u64 = 34_359_738_368;
    /// Default storage allocation (1 TiB).
    pub const DEFAULT_STORAGE_BYTES: u64 = 1_099_511_627_776;
    /// Default throughput allocation (200 QPS).
    pub const DEFAULT_QPS: u32 = 200;

    /// Creates a quota configuration using platform defaults.
    #[must_use]
    pub fn standard() -> Self {
        Self::default()
    }

    /// Returns `true` when memory quota is unlimited.
    #[must_use]
    pub fn is_memory_unbounded(&self) -> bool {
        self.memory_quota_bytes == 0
    }

    /// Returns `true` when storage quota is unlimited.
    #[must_use]
    pub fn is_storage_unbounded(&self) -> bool {
        self.storage_quota_bytes == 0
    }

    /// Returns `true` when QPS quota is unlimited.
    #[must_use]
    pub fn is_qps_unbounded(&self) -> bool {
        self.qps_quota == 0
    }
}

impl Default for TenantQuota {
    fn default() -> Self {
        Self {
            memory_quota_bytes: Self::DEFAULT_MEMORY_BYTES,
            storage_quota_bytes: Self::DEFAULT_STORAGE_BYTES,
            qps_quota: Self::DEFAULT_QPS,
        }
    }
}

/// Immutable tenant metadata persisted by the metadata service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantDescriptor {
    /// Stable tenant identifier.
    pub tenant_id: TenantId,
    /// Human-readable tenant name.
    pub name: String,
    /// URL-safe slug that uniquely identifies the tenant.
    pub slug: String,
    /// Lifecycle status of the tenant.
    pub status: TenantStatus,
    /// Resource quotas allocated to the tenant.
    pub quotas: TenantQuota,
    /// Arbitrary metadata stored as JSON.
    pub metadata: Value,
    /// Creation timestamp in UTC.
    pub created_at: DateTime<Utc>,
    /// Update timestamp in UTC.
    pub updated_at: DateTime<Utc>,
}

impl TenantDescriptor {
    /// Creates a provisioned tenant descriptor using defaults.
    #[must_use]
    pub fn new(name: impl Into<String>, slug: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            tenant_id: TenantId::new(),
            name: name.into(),
            slug: slug.into(),
            status: TenantStatus::default(),
            quotas: TenantQuota::default(),
            metadata: Value::Object(serde_json::Map::new()),
            created_at: now,
            updated_at: now,
        }
    }

    /// Returns a mutable reference to the metadata object map, creating it when missing.
    #[must_use]
    pub fn metadata_object(&mut self) -> &mut serde_json::Map<String, Value> {
        if !self.metadata.is_object() {
            self.metadata = Value::Object(serde_json::Map::new());
        }
        self.metadata
            .as_object_mut()
            .expect("metadata must be an object")
    }

    /// Sets the tenant status and updates the modification timestamp.
    pub fn transition_to(&mut self, status: TenantStatus) {
        self.status = status;
        self.touch();
    }

    /// Updates the `updated_at` timestamp to the current time.
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}
