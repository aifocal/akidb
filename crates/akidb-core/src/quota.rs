use crate::{TenantError, TenantId, TenantQuota, TenantUsage};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::warn;

/// Quota tracker for tenant resource usage
#[derive(Clone)]
pub struct QuotaTracker {
    /// In-memory usage tracking (tenant_id -> usage)
    usage: Arc<RwLock<HashMap<TenantId, TenantUsage>>>,
    /// Quota limits (tenant_id -> quota)
    limits: Arc<RwLock<HashMap<TenantId, TenantQuota>>>,
}

impl QuotaTracker {
    /// Create a new quota tracker
    pub fn new() -> Self {
        Self {
            usage: Arc::new(RwLock::new(HashMap::new())),
            limits: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Set quota limits for a tenant
    pub fn set_quota(&self, tenant_id: TenantId, quota: TenantQuota) {
        self.limits.write().insert(tenant_id, quota);
    }

    /// Get quota limits for a tenant
    pub fn get_quota(&self, tenant_id: &TenantId) -> Option<TenantQuota> {
        self.limits.read().get(tenant_id).cloned()
    }

    /// Get current usage for a tenant
    pub fn get_usage(&self, tenant_id: &TenantId) -> TenantUsage {
        self.usage
            .read()
            .get(tenant_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Update storage usage
    pub fn update_storage(&self, tenant_id: TenantId, bytes: u64) {
        let mut usage_map = self.usage.write();
        let usage = usage_map.entry(tenant_id).or_default();
        usage.storage_bytes = bytes;
        usage.last_updated = Some(chrono::Utc::now());
    }

    /// Increment storage usage
    pub fn increment_storage(&self, tenant_id: TenantId, bytes: u64) {
        let mut usage_map = self.usage.write();
        let usage = usage_map.entry(tenant_id).or_default();
        usage.storage_bytes += bytes;
        usage.last_updated = Some(chrono::Utc::now());
    }

    /// Decrement storage usage
    pub fn decrement_storage(&self, tenant_id: TenantId, bytes: u64) {
        let mut usage_map = self.usage.write();
        let usage = usage_map.entry(tenant_id).or_default();
        usage.storage_bytes = usage.storage_bytes.saturating_sub(bytes);
        usage.last_updated = Some(chrono::Utc::now());
    }

    /// Update collection count
    pub fn update_collections(&self, tenant_id: TenantId, count: u32) {
        let mut usage_map = self.usage.write();
        let usage = usage_map.entry(tenant_id).or_default();
        usage.collection_count = count;
        usage.last_updated = Some(chrono::Utc::now());
    }

    /// Increment collection count
    pub fn increment_collections(&self, tenant_id: TenantId) {
        let mut usage_map = self.usage.write();
        let usage = usage_map.entry(tenant_id).or_default();
        usage.collection_count += 1;
        usage.last_updated = Some(chrono::Utc::now());
    }

    /// Decrement collection count
    pub fn decrement_collections(&self, tenant_id: TenantId) {
        let mut usage_map = self.usage.write();
        let usage = usage_map.entry(tenant_id).or_default();
        usage.collection_count = usage.collection_count.saturating_sub(1);
        usage.last_updated = Some(chrono::Utc::now());
    }

    /// Update vector count
    pub fn update_vectors(&self, tenant_id: TenantId, count: u64) {
        let mut usage_map = self.usage.write();
        let usage = usage_map.entry(tenant_id).or_default();
        usage.total_vectors = count;
        usage.last_updated = Some(chrono::Utc::now());
    }

    /// Increment vector count
    pub fn increment_vectors(&self, tenant_id: TenantId, count: u64) {
        let mut usage_map = self.usage.write();
        let usage = usage_map.entry(tenant_id).or_default();
        usage.total_vectors += count;
        usage.last_updated = Some(chrono::Utc::now());
    }

    /// Decrement vector count
    pub fn decrement_vectors(&self, tenant_id: TenantId, count: u64) {
        let mut usage_map = self.usage.write();
        let usage = usage_map.entry(tenant_id).or_default();
        usage.total_vectors = usage.total_vectors.saturating_sub(count);
        usage.last_updated = Some(chrono::Utc::now());
    }

    /// Increment API request counter
    pub fn increment_api_requests(&self, tenant_id: TenantId) {
        let mut usage_map = self.usage.write();
        let usage = usage_map.entry(tenant_id).or_default();
        usage.api_requests_last_minute += 1;
        usage.last_updated = Some(chrono::Utc::now());
    }

    /// Reset API request counter (called every minute)
    pub fn reset_api_requests(&self, tenant_id: &TenantId) {
        let mut usage_map = self.usage.write();
        if let Some(usage) = usage_map.get_mut(tenant_id) {
            usage.api_requests_last_minute = 0;
        }
    }

    /// Check if storage quota would be exceeded
    pub fn check_storage_quota(
        &self,
        tenant_id: &TenantId,
        additional_bytes: u64,
    ) -> Result<(), TenantError> {
        let usage = self.get_usage(tenant_id);
        let quota = self
            .get_quota(tenant_id)
            .unwrap_or_else(TenantQuota::default);

        // Check if unlimited
        if quota.max_storage_bytes == 0 {
            return Ok(());
        }

        let new_usage = usage.storage_bytes + additional_bytes;
        if new_usage > quota.max_storage_bytes {
            warn!(
                "Storage quota exceeded for tenant {}: {} + {} > {}",
                tenant_id, usage.storage_bytes, additional_bytes, quota.max_storage_bytes
            );
            return Err(TenantError::QuotaExceeded {
                quota_type: "storage".to_string(),
            });
        }

        Ok(())
    }

    /// Check if collection quota would be exceeded
    pub fn check_collection_quota(&self, tenant_id: &TenantId) -> Result<(), TenantError> {
        let usage = self.get_usage(tenant_id);
        let quota = self
            .get_quota(tenant_id)
            .unwrap_or_else(TenantQuota::default);

        // Check if unlimited
        if quota.max_collections == 0 {
            return Ok(());
        }

        if usage.collection_count >= quota.max_collections {
            warn!(
                "Collection quota exceeded for tenant {}: {} >= {}",
                tenant_id, usage.collection_count, quota.max_collections
            );
            return Err(TenantError::QuotaExceeded {
                quota_type: "collections".to_string(),
            });
        }

        Ok(())
    }

    /// Check if vector quota would be exceeded for a collection
    pub fn check_vector_quota(
        &self,
        tenant_id: &TenantId,
        collection_vectors: u64,
        additional_vectors: u64,
    ) -> Result<(), TenantError> {
        let quota = self
            .get_quota(tenant_id)
            .unwrap_or_else(TenantQuota::default);

        // Check if unlimited
        if quota.max_vectors_per_collection == 0 {
            return Ok(());
        }

        let new_count = collection_vectors + additional_vectors;
        if new_count > quota.max_vectors_per_collection {
            warn!(
                "Vector quota exceeded for tenant {}: {} + {} > {}",
                tenant_id, collection_vectors, additional_vectors, quota.max_vectors_per_collection
            );
            return Err(TenantError::QuotaExceeded {
                quota_type: "vectors".to_string(),
            });
        }

        Ok(())
    }

    /// Check if API rate limit would be exceeded
    pub fn check_rate_limit(&self, tenant_id: &TenantId) -> Result<(), TenantError> {
        let usage = self.get_usage(tenant_id);
        let quota = self
            .get_quota(tenant_id)
            .unwrap_or_else(TenantQuota::default);

        // Check if unlimited
        if quota.api_rate_limit_per_second == 0 {
            return Ok(());
        }

        // Convert per-second limit to per-minute for tracking
        let per_minute_limit = quota.api_rate_limit_per_second as u64 * 60;

        if usage.api_requests_last_minute >= per_minute_limit {
            warn!(
                "Rate limit exceeded for tenant {}: {} >= {}",
                tenant_id, usage.api_requests_last_minute, per_minute_limit
            );
            return Err(TenantError::QuotaExceeded {
                quota_type: "rate_limit".to_string(),
            });
        }

        Ok(())
    }

    /// Get quota utilization percentage for a tenant
    pub fn get_quota_utilization(&self, tenant_id: &TenantId) -> QuotaUtilization {
        let usage = self.get_usage(tenant_id);
        let quota = self
            .get_quota(tenant_id)
            .unwrap_or_else(TenantQuota::default);

        let storage_pct = if quota.max_storage_bytes > 0 {
            (usage.storage_bytes as f64 / quota.max_storage_bytes as f64 * 100.0) as u32
        } else {
            0
        };

        let collections_pct = if quota.max_collections > 0 {
            (usage.collection_count as f64 / quota.max_collections as f64 * 100.0) as u32
        } else {
            0
        };

        // Note: vectors_percent is not calculated because total_vectors is across all collections
        // while max_vectors_per_collection is a per-collection limit. These cannot be meaningfully
        // compared. To get accurate vector utilization, check each collection individually.
        let vectors_pct = 0;

        QuotaUtilization {
            storage_percent: storage_pct,
            collections_percent: collections_pct,
            vectors_percent: vectors_pct,
        }
    }
}

impl Default for QuotaTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Quota utilization percentages
#[derive(Debug, Clone)]
pub struct QuotaUtilization {
    pub storage_percent: u32,
    pub collections_percent: u32,
    pub vectors_percent: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quota_tracker_basic() {
        let tracker = QuotaTracker::new();
        let tenant_id = "tenant_test".to_string();

        // Set quota
        let quota = TenantQuota::default();
        tracker.set_quota(tenant_id.clone(), quota.clone());

        // Verify quota
        assert_eq!(tracker.get_quota(&tenant_id).unwrap(), quota);
    }

    #[test]
    fn test_storage_tracking() {
        let tracker = QuotaTracker::new();
        let tenant_id = "tenant_test".to_string();

        // Increment storage
        tracker.increment_storage(tenant_id.clone(), 1000);
        tracker.increment_storage(tenant_id.clone(), 500);

        let usage = tracker.get_usage(&tenant_id);
        assert_eq!(usage.storage_bytes, 1500);

        // Decrement storage
        tracker.decrement_storage(tenant_id.clone(), 300);
        let usage = tracker.get_usage(&tenant_id);
        assert_eq!(usage.storage_bytes, 1200);
    }

    #[test]
    fn test_collection_tracking() {
        let tracker = QuotaTracker::new();
        let tenant_id = "tenant_test".to_string();

        tracker.increment_collections(tenant_id.clone());
        tracker.increment_collections(tenant_id.clone());
        tracker.increment_collections(tenant_id.clone());

        let usage = tracker.get_usage(&tenant_id);
        assert_eq!(usage.collection_count, 3);

        tracker.decrement_collections(tenant_id.clone());
        let usage = tracker.get_usage(&tenant_id);
        assert_eq!(usage.collection_count, 2);
    }

    #[test]
    fn test_vector_tracking() {
        let tracker = QuotaTracker::new();
        let tenant_id = "tenant_test".to_string();

        tracker.increment_vectors(tenant_id.clone(), 1000);
        tracker.increment_vectors(tenant_id.clone(), 2000);

        let usage = tracker.get_usage(&tenant_id);
        assert_eq!(usage.total_vectors, 3000);
    }

    #[test]
    fn test_storage_quota_check() {
        let tracker = QuotaTracker::new();
        let tenant_id = "tenant_test".to_string();

        let mut quota = TenantQuota::default();
        quota.max_storage_bytes = 1000;
        tracker.set_quota(tenant_id.clone(), quota);

        // Should pass
        assert!(tracker.check_storage_quota(&tenant_id, 500).is_ok());

        // Update usage
        tracker.update_storage(tenant_id.clone(), 800);

        // Should fail (800 + 300 > 1000)
        assert!(tracker.check_storage_quota(&tenant_id, 300).is_err());
    }

    #[test]
    fn test_collection_quota_check() {
        let tracker = QuotaTracker::new();
        let tenant_id = "tenant_test".to_string();

        let mut quota = TenantQuota::default();
        quota.max_collections = 5;
        tracker.set_quota(tenant_id.clone(), quota);

        // Should pass
        tracker.update_collections(tenant_id.clone(), 3);
        assert!(tracker.check_collection_quota(&tenant_id).is_ok());

        // Should fail
        tracker.update_collections(tenant_id.clone(), 5);
        assert!(tracker.check_collection_quota(&tenant_id).is_err());
    }

    #[test]
    fn test_unlimited_quota() {
        let tracker = QuotaTracker::new();
        let tenant_id = "tenant_test".to_string();

        // Set unlimited quota
        tracker.set_quota(tenant_id.clone(), TenantQuota::unlimited());

        // All checks should pass regardless of usage
        tracker.update_storage(tenant_id.clone(), u64::MAX / 2);
        assert!(tracker
            .check_storage_quota(&tenant_id, u64::MAX / 2)
            .is_ok());

        tracker.update_collections(tenant_id.clone(), u32::MAX / 2);
        assert!(tracker.check_collection_quota(&tenant_id).is_ok());
    }

    #[test]
    fn test_quota_utilization() {
        let tracker = QuotaTracker::new();
        let tenant_id = "tenant_test".to_string();

        let mut quota = TenantQuota::default();
        quota.max_storage_bytes = 1000;
        quota.max_collections = 10;
        tracker.set_quota(tenant_id.clone(), quota);

        tracker.update_storage(tenant_id.clone(), 500); // 50%
        tracker.update_collections(tenant_id.clone(), 3); // 30%

        let util = tracker.get_quota_utilization(&tenant_id);
        assert_eq!(util.storage_percent, 50);
        assert_eq!(util.collections_percent, 30);
    }
}
