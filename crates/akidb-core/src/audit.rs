//! Audit log domain model for compliance and security monitoring.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::str::FromStr;

use crate::ids::{AuditLogId, TenantId, UserId};
use crate::user::Action;

/// Audit log entry for compliance and security monitoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub audit_log_id: AuditLogId,
    pub tenant_id: TenantId,
    pub user_id: Option<UserId>,
    pub action: Action,
    pub resource_type: String,
    pub resource_id: String,
    pub result: AuditResult,
    pub reason: Option<String>,
    pub metadata: Option<JsonValue>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Result of an authorization decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditResult {
    /// Action was allowed.
    Allowed,
    /// Action was denied.
    Denied,
}

impl AuditLogEntry {
    /// Create a new audit log entry.
    #[must_use]
    pub fn new(
        tenant_id: TenantId,
        user_id: Option<UserId>,
        action: Action,
        resource_type: impl Into<String>,
        resource_id: impl Into<String>,
        result: AuditResult,
    ) -> Self {
        Self {
            audit_log_id: AuditLogId::new(),
            tenant_id,
            user_id,
            action,
            resource_type: resource_type.into(),
            resource_id: resource_id.into(),
            result,
            reason: None,
            metadata: None,
            ip_address: None,
            user_agent: None,
            created_at: Utc::now(),
        }
    }

    /// Add a reason for the authorization decision.
    #[must_use]
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Add metadata (e.g., request details).
    #[must_use]
    pub fn with_metadata(mut self, metadata: JsonValue) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Add client IP address.
    #[must_use]
    pub fn with_ip(mut self, ip: impl Into<String>) -> Self {
        self.ip_address = Some(ip.into());
        self
    }

    /// Add user agent string.
    #[must_use]
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }
}

impl AuditResult {
    /// Convert result to string representation.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditResult::Allowed => "allowed",
            AuditResult::Denied => "denied",
        }
    }
}

impl FromStr for AuditResult {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "allowed" => Ok(AuditResult::Allowed),
            "denied" => Ok(AuditResult::Denied),
            _ => Err(format!("invalid audit result: {s}")),
        }
    }
}
