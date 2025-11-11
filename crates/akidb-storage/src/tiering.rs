//! Tiering policies for vector storage
//!
//! Provides three storage tiers with different performance/cost trade-offs:
//! - Memory: Fastest, ephemeral (dev/test)
//! - MemoryS3: Fast + durable (production)
//! - S3Only: Cost-optimized (cold storage)

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Tiering policy determines where vectors are stored
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TieringPolicy {
    /// Vectors in memory only (no S3 sync)
    ///
    /// - **Performance:** <1ms insert/query P95
    /// - **Durability:** Local WAL only (ephemeral on disk loss)
    /// - **Use case:** Dev/test, hot cache, <10GB datasets
    Memory,

    /// Vectors in memory + async S3 backup
    ///
    /// - **Performance:** <2ms insert/query P95
    /// - **Durability:** S3 backup (11-nines)
    /// - **Use case:** Production RAG, semantic search
    MemoryS3,

    /// Vectors on S3, loaded on-demand to memory cache
    ///
    /// - **Performance:** <10ms query P95 (cache hit), <50ms (cache miss)
    /// - **Durability:** S3 primary storage
    /// - **Use case:** Archival, cold storage, cost-sensitive
    S3Only,
}

impl TieringPolicy {
    /// Get string representation
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::MemoryS3 => "memory_s3",
            Self::S3Only => "s3_only",
        }
    }

    /// Check if policy requires S3
    #[must_use]
    pub fn requires_s3(&self) -> bool {
        matches!(self, Self::MemoryS3 | Self::S3Only)
    }

    /// Check if policy keeps vectors in memory
    #[must_use]
    pub fn keeps_memory(&self) -> bool {
        matches!(self, Self::Memory | Self::MemoryS3)
    }

    /// Check if policy requires WAL
    #[must_use]
    pub fn requires_wal(&self) -> bool {
        // All policies use WAL for local durability
        true
    }
}

impl std::fmt::Display for TieringPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Compression type for snapshots
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionType {
    /// No compression
    None,
    /// Snappy compression (fast, moderate ratio)
    Snappy,
    /// Zstd compression (balanced)
    Zstd,
    /// LZ4 compression (fastest)
    Lz4,
}

impl Default for CompressionType {
    fn default() -> Self {
        Self::None
    }
}

/// Compaction configuration
#[derive(Debug, Clone)]
pub struct CompactionConfig {
    /// Compaction threshold (WAL size in bytes)
    pub threshold_bytes: u64,
    /// Compaction threshold (WAL operation count)
    pub threshold_ops: u64,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            threshold_bytes: 100 * 1024 * 1024, // 100MB
            threshold_ops: 10_000,
        }
    }
}

/// Storage configuration
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// FIX BUG #16: Collection ID for S3 key paths and WAL entries
    /// Without this, every operation generates a random CollectionId, making S3 backups unusable
    pub collection_id: akidb_core::CollectionId,

    /// Tiering policy
    pub tiering_policy: TieringPolicy,

    /// WAL path
    pub wal_path: PathBuf,

    /// Snapshot directory (for periodic snapshots)
    pub snapshot_dir: PathBuf,

    /// S3 bucket name (required for MemoryS3 and S3Only)
    pub s3_bucket: Option<String>,

    /// S3 region
    pub s3_region: String,

    /// S3 endpoint (for MinIO compatibility)
    pub s3_endpoint: Option<String>,

    /// S3 access key (for MinIO compatibility)
    pub s3_access_key: Option<String>,

    /// S3 secret key (for MinIO compatibility)
    pub s3_secret_key: Option<String>,

    /// Sync interval for MemoryS3 (periodic snapshot + upload)
    pub sync_interval: Duration,

    /// Compaction threshold (WAL size in bytes)
    pub compaction_threshold_bytes: u64,

    /// Compaction threshold (WAL operation count)
    pub compaction_threshold_ops: u64,

    /// Cache size for S3Only (number of vectors)
    pub cache_size: usize,

    /// Compression type for snapshots
    pub compression: CompressionType,

    /// Enable background compaction worker (default: true)
    pub enable_background_compaction: bool,

    /// Compaction configuration
    pub compaction_config: CompactionConfig,

    /// Retry configuration for S3 uploads (Day 4)
    pub retry_config: Option<crate::storage_backend::RetryConfig>,

    /// Circuit breaker enabled (Phase 7 Week 1)
    pub circuit_breaker_enabled: bool,

    /// Circuit breaker configuration (Phase 7 Week 1)
    pub circuit_breaker_config: Option<crate::circuit_breaker::CircuitBreakerConfig>,

    /// DLQ configuration (Phase 7 Week 1 Days 3-4)
    pub dlq_config: crate::dlq::DLQConfig,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            // FIX BUG #16: Generate a default collection_id for testing
            // In production, this should be provided by the caller
            collection_id: akidb_core::CollectionId::new(),
            tiering_policy: TieringPolicy::Memory,
            wal_path: PathBuf::from("./akidb.wal"),
            snapshot_dir: PathBuf::from("./snapshots"),
            s3_bucket: None,
            s3_region: "us-east-1".to_string(),
            s3_endpoint: None,
            s3_access_key: None,
            s3_secret_key: None,
            sync_interval: Duration::from_secs(300), // 5 minutes
            compaction_threshold_bytes: 100 * 1024 * 1024, // 100MB
            compaction_threshold_ops: 10_000,
            cache_size: 10_000, // 10k vectors
            compression: CompressionType::None,
            enable_background_compaction: true,
            compaction_config: CompactionConfig::default(),
            retry_config: None, // Use RetryConfig::default() when needed
            circuit_breaker_enabled: true,
            circuit_breaker_config: Some(crate::circuit_breaker::CircuitBreakerConfig::default()),
            dlq_config: crate::dlq::DLQConfig::default(),
        }
    }
}

impl StorageConfig {
    /// Validate configuration
    ///
    /// # Errors
    ///
    /// Returns `CoreError::ValidationError` if:
    /// - S3 bucket missing for MemoryS3/S3Only policies
    /// - Snapshot directory doesn't exist
    pub fn validate(&self) -> akidb_core::CoreResult<()> {
        // S3 bucket required for MemoryS3 and S3Only
        if self.tiering_policy.requires_s3() && self.s3_bucket.is_none() {
            return Err(akidb_core::CoreError::ValidationError(format!(
                "S3 bucket required for {} policy",
                self.tiering_policy
            )));
        }

        // Snapshot directory must exist for policies that use it
        if self.tiering_policy.requires_s3() && !self.snapshot_dir.exists() {
            return Err(akidb_core::CoreError::ValidationError(format!(
                "Snapshot directory does not exist: {}",
                self.snapshot_dir.display()
            )));
        }

        Ok(())
    }

    /// Create config for memory policy
    pub fn memory(wal_path: impl AsRef<Path>) -> Self {
        Self {
            tiering_policy: TieringPolicy::Memory,
            wal_path: wal_path.as_ref().to_path_buf(),
            ..Default::default()
        }
    }

    /// Create config for memory_s3 policy
    pub fn memory_s3(
        wal_path: impl AsRef<Path>,
        snapshot_dir: impl AsRef<Path>,
        s3_bucket: String,
    ) -> Self {
        Self {
            tiering_policy: TieringPolicy::MemoryS3,
            wal_path: wal_path.as_ref().to_path_buf(),
            snapshot_dir: snapshot_dir.as_ref().to_path_buf(),
            s3_bucket: Some(s3_bucket),
            ..Default::default()
        }
    }

    /// Create config for s3_only policy
    pub fn s3_only(
        wal_path: impl AsRef<Path>,
        snapshot_dir: impl AsRef<Path>,
        s3_bucket: String,
        cache_size: usize,
    ) -> Self {
        Self {
            tiering_policy: TieringPolicy::S3Only,
            wal_path: wal_path.as_ref().to_path_buf(),
            snapshot_dir: snapshot_dir.as_ref().to_path_buf(),
            s3_bucket: Some(s3_bucket),
            cache_size,
            ..Default::default()
        }
    }

    /// Set S3 endpoint (for MinIO compatibility)
    pub fn with_s3_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.s3_endpoint = Some(endpoint.into());
        self
    }

    /// Set S3 credentials (for MinIO compatibility)
    pub fn with_s3_credentials(
        mut self,
        access_key: impl Into<String>,
        secret_key: impl Into<String>,
    ) -> Self {
        self.s3_access_key = Some(access_key.into());
        self.s3_secret_key = Some(secret_key.into());
        self
    }

    /// Set compression type
    pub fn with_compression(mut self, compression: CompressionType) -> Self {
        self.compression = compression;
        self
    }

    /// Set sync interval
    pub fn with_sync_interval(mut self, interval: Duration) -> Self {
        self.sync_interval = interval;
        self
    }

    /// Set compaction thresholds
    pub fn with_compaction_thresholds(mut self, bytes: u64, ops: u64) -> Self {
        self.compaction_threshold_bytes = bytes;
        self.compaction_threshold_ops = ops;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tiering_policy_requires_s3() {
        assert!(!TieringPolicy::Memory.requires_s3());
        assert!(TieringPolicy::MemoryS3.requires_s3());
        assert!(TieringPolicy::S3Only.requires_s3());
    }

    #[test]
    fn test_tiering_policy_keeps_memory() {
        assert!(TieringPolicy::Memory.keeps_memory());
        assert!(TieringPolicy::MemoryS3.keeps_memory());
        assert!(!TieringPolicy::S3Only.keeps_memory());
    }

    #[test]
    fn test_tiering_policy_requires_wal() {
        assert!(TieringPolicy::Memory.requires_wal());
        assert!(TieringPolicy::MemoryS3.requires_wal());
        assert!(TieringPolicy::S3Only.requires_wal());
    }

    #[test]
    fn test_tiering_policy_display() {
        assert_eq!(TieringPolicy::Memory.to_string(), "memory");
        assert_eq!(TieringPolicy::MemoryS3.to_string(), "memory_s3");
        assert_eq!(TieringPolicy::S3Only.to_string(), "s3_only");
    }

    #[test]
    fn test_storage_config_memory() {
        let config = StorageConfig::memory("/tmp/test.wal");
        assert_eq!(config.tiering_policy, TieringPolicy::Memory);
        assert_eq!(config.wal_path, PathBuf::from("/tmp/test.wal"));
        assert!(config.s3_bucket.is_none());
    }

    #[test]
    fn test_storage_config_memory_s3() {
        let config =
            StorageConfig::memory_s3("/tmp/test.wal", "/tmp/snapshots", "akidb-test".to_string());
        assert_eq!(config.tiering_policy, TieringPolicy::MemoryS3);
        assert_eq!(config.s3_bucket, Some("akidb-test".to_string()));
    }

    #[test]
    fn test_storage_config_s3_only() {
        let config = StorageConfig::s3_only(
            "/tmp/test.wal",
            "/tmp/snapshots",
            "akidb-test".to_string(),
            5000,
        );
        assert_eq!(config.tiering_policy, TieringPolicy::S3Only);
        assert_eq!(config.cache_size, 5000);
    }

    #[test]
    fn test_storage_config_validation_memory() {
        let config = StorageConfig::memory("/tmp/test.wal");
        // Memory policy doesn't require S3, so validation should pass even without bucket
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_storage_config_validation_memory_s3_no_bucket() {
        let mut config = StorageConfig::default();
        config.tiering_policy = TieringPolicy::MemoryS3;
        config.s3_bucket = None;

        // Should fail - bucket required
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("S3 bucket required"));
    }

    #[test]
    fn test_storage_config_with_builders() {
        let config = StorageConfig::memory("/tmp/test.wal")
            .with_compression(CompressionType::Zstd)
            .with_sync_interval(Duration::from_secs(60))
            .with_compaction_thresholds(50_000_000, 5000);

        assert_eq!(config.compression, CompressionType::Zstd);
        assert_eq!(config.sync_interval, Duration::from_secs(60));
        assert_eq!(config.compaction_threshold_bytes, 50_000_000);
        assert_eq!(config.compaction_threshold_ops, 5000);
    }

    #[test]
    fn test_storage_config_with_s3_endpoint() {
        let config =
            StorageConfig::memory_s3("/tmp/test.wal", "/tmp/snapshots", "akidb-test".to_string())
                .with_s3_endpoint("http://localhost:9000")
                .with_s3_credentials("minioadmin", "minioadmin");

        assert_eq!(
            config.s3_endpoint,
            Some("http://localhost:9000".to_string())
        );
        assert_eq!(config.s3_access_key, Some("minioadmin".to_string()));
        assert_eq!(config.s3_secret_key, Some("minioadmin".to_string()));
    }
}
