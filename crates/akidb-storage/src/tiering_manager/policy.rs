use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Tiering policy configuration
///
/// Controls when collections are promoted/demoted between tiers based on access patterns.
///
/// # Example
///
/// ```
/// use akidb_storage::tiering_manager::TieringPolicyConfig;
///
/// let policy = TieringPolicyConfig::default();
/// assert_eq!(policy.hot_tier_ttl_hours, 6);
/// assert_eq!(policy.warm_tier_ttl_days, 7);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TieringPolicyConfig {
    /// Hours without access before demoting hot → warm (default: 6)
    pub hot_tier_ttl_hours: i64,

    /// Days without access before demoting warm → cold (default: 7)
    pub warm_tier_ttl_days: i64,

    /// Access count threshold for promoting warm → hot (default: 10)
    pub hot_promotion_threshold: u32,

    /// Access window for promotion counting (default: 1 hour)
    pub access_window_hours: i64,

    /// Background worker interval in seconds (default: 300 = 5 minutes)
    pub worker_interval_secs: u64,
}

impl Default for TieringPolicyConfig {
    fn default() -> Self {
        Self {
            hot_tier_ttl_hours: 6,
            warm_tier_ttl_days: 7,
            hot_promotion_threshold: 10,
            access_window_hours: 1,
            worker_interval_secs: 300, // 5 minutes
        }
    }
}

impl TieringPolicyConfig {
    /// Validate policy configuration
    ///
    /// # Errors
    ///
    /// Returns error if any value is below minimum threshold
    pub fn validate(&self) -> Result<(), String> {
        if self.hot_tier_ttl_hours < 1 {
            return Err("hot_tier_ttl_hours must be >= 1".into());
        }
        if self.warm_tier_ttl_days < 1 {
            return Err("warm_tier_ttl_days must be >= 1".into());
        }
        if self.hot_promotion_threshold < 1 {
            return Err("hot_promotion_threshold must be >= 1".into());
        }
        if self.access_window_hours < 1 {
            return Err("access_window_hours must be >= 1".into());
        }
        if self.worker_interval_secs < 60 {
            return Err("worker_interval_secs must be >= 60".into());
        }
        Ok(())
    }

    /// Get worker interval as Duration
    pub fn worker_interval(&self) -> Duration {
        Duration::from_secs(self.worker_interval_secs)
    }

    /// Create policy optimized for testing (short intervals)
    #[cfg(test)]
    pub fn test_config() -> Self {
        Self {
            hot_tier_ttl_hours: 0, // Demote immediately
            warm_tier_ttl_days: 0,
            hot_promotion_threshold: 5,
            access_window_hours: 1,
            worker_interval_secs: 60,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy() {
        let policy = TieringPolicyConfig::default();
        assert_eq!(policy.hot_tier_ttl_hours, 6);
        assert_eq!(policy.warm_tier_ttl_days, 7);
        assert_eq!(policy.hot_promotion_threshold, 10);
        assert_eq!(policy.access_window_hours, 1);
        assert_eq!(policy.worker_interval_secs, 300);
        assert!(policy.validate().is_ok());
    }

    #[test]
    fn test_invalid_hot_ttl() {
        let mut policy = TieringPolicyConfig::default();
        policy.hot_tier_ttl_hours = 0;
        assert!(policy.validate().is_err());
    }

    #[test]
    fn test_invalid_warm_ttl() {
        let mut policy = TieringPolicyConfig::default();
        policy.warm_tier_ttl_days = 0;
        assert!(policy.validate().is_err());
    }

    #[test]
    fn test_invalid_threshold() {
        let mut policy = TieringPolicyConfig::default();
        policy.hot_promotion_threshold = 0;
        assert!(policy.validate().is_err());
    }

    #[test]
    fn test_invalid_worker_interval() {
        let mut policy = TieringPolicyConfig::default();
        policy.worker_interval_secs = 30;
        assert!(policy.validate().is_err());
    }

    #[test]
    fn test_worker_interval_conversion() {
        let policy = TieringPolicyConfig::default();
        assert_eq!(policy.worker_interval(), Duration::from_secs(300));
    }

    #[test]
    fn test_test_config() {
        let policy = TieringPolicyConfig::test_config();
        assert_eq!(policy.hot_tier_ttl_hours, 0);
        assert_eq!(policy.hot_promotion_threshold, 5);
    }
}
