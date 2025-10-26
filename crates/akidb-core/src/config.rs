//! Configuration management for AkiDB
//!
//! This module provides a centralized configuration system that supports:
//! - YAML configuration files
//! - Environment variable overrides
//! - Reasonable defaults
//! - Configuration validation

use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

/// Root configuration structure for AkiDB
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AkidbConfig {
    #[serde(default)]
    pub storage: StorageConfig,

    #[serde(default)]
    pub index: IndexConfig,

    #[serde(default)]
    pub api: ApiConfig,

    #[serde(default)]
    pub query: QueryConfig,
}

impl AkidbConfig {
    /// Load configuration from multiple sources with precedence:
    /// 1. Environment variables (highest priority)
    /// 2. Config file specified by AKIDB_CONFIG env var
    /// 3. ./config/akidb.yaml
    /// 4. /etc/akidb/akidb.yaml
    /// 5. Hardcoded defaults (lowest priority)
    pub fn load() -> Result<Self, ConfigError> {
        let mut builder = Config::builder();

        // Set defaults
        builder = Self::set_defaults(builder)?;

        // Load from files (in order of precedence)
        if let Ok(config_path) = std::env::var("AKIDB_CONFIG") {
            builder = builder.add_source(File::with_name(&config_path).required(false));
        }

        builder = builder
            .add_source(File::with_name("./config/akidb").required(false))
            .add_source(File::with_name("/etc/akidb/akidb").required(false));

        // Override with environment variables
        // Example: AKIDB_STORAGE__RETRY__MAX_ATTEMPTS=20
        builder = builder.add_source(
            Environment::with_prefix("AKIDB")
                .separator("__")
                .try_parsing(true),
        );

        let config: AkidbConfig = builder.build()?.try_deserialize()?;

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Set default values for all configuration options
    fn set_defaults(
        builder: config::ConfigBuilder<config::builder::DefaultState>,
    ) -> Result<config::ConfigBuilder<config::builder::DefaultState>, ConfigError> {
        builder
            // Storage: Circuit Breaker
            .set_default("storage.circuit_breaker.failure_threshold", 5)?
            .set_default("storage.circuit_breaker.recovery_timeout_secs", 30)?
            // Storage: Retry
            .set_default("storage.retry.max_attempts", 10)?
            .set_default("storage.retry.initial_backoff_ms", 100)?
            .set_default("storage.retry.max_backoff_ms", 5000)?
            .set_default("storage.retry.backoff_multiplier", 2.0)?
            // Storage: Manifest Retry (critical operation)
            .set_default("storage.manifest_retry.max_attempts", 20)?
            .set_default("storage.manifest_retry.initial_backoff_ms", 50)?
            .set_default("storage.manifest_retry.max_backoff_ms", 2000)?
            // Index: HNSW
            .set_default("index.hnsw.m", 16)?
            .set_default("index.hnsw.ef_construction", 400)?
            .set_default("index.hnsw.ef_search", 200)?
            .set_default("index.hnsw.min_vectors_threshold", 100)?
            // Index: Native
            .set_default("index.native.max_vectors", 10000)?
            // API: Validation
            .set_default("api.validation.collection_name_max_length", 255)?
            .set_default("api.validation.vector_dimension_max", 4096)?
            .set_default("api.validation.vector_dimension_min", 1)?
            .set_default("api.validation.top_k_max", 1000)?
            .set_default("api.validation.top_k_min", 1)?
            .set_default("api.validation.batch_size_max", 1000)?
            // Query
            .set_default("query.max_filter_depth", 32)?
            .set_default("query.max_filter_clauses", 100)?
            .set_default("query.parallel_segments", true)?
            .set_default("query.max_parallel_segments", 8)
    }

    /// Validate configuration values
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate storage configuration
        if self.storage.circuit_breaker.failure_threshold == 0 {
            return Err(ConfigError::Message(
                "storage.circuit_breaker.failure_threshold must be > 0".to_string(),
            ));
        }

        if self.storage.retry.max_attempts == 0 {
            return Err(ConfigError::Message(
                "storage.retry.max_attempts must be > 0".to_string(),
            ));
        }

        // Validate index configuration
        if self.index.hnsw.m == 0 {
            return Err(ConfigError::Message("index.hnsw.m must be > 0".to_string()));
        }

        if self.index.hnsw.ef_construction == 0 {
            return Err(ConfigError::Message(
                "index.hnsw.ef_construction must be > 0".to_string(),
            ));
        }

        // Validate API configuration
        if self.api.validation.vector_dimension_max < self.api.validation.vector_dimension_min {
            return Err(ConfigError::Message(
                "api.validation.vector_dimension_max must be >= vector_dimension_min".to_string(),
            ));
        }

        if self.api.validation.top_k_max < self.api.validation.top_k_min {
            return Err(ConfigError::Message(
                "api.validation.top_k_max must be >= top_k_min".to_string(),
            ));
        }

        Ok(())
    }

    /// Load configuration from a specific file path
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let config = Config::builder()
            .add_source(File::from(path.as_ref()))
            .build()?
            .try_deserialize()?;

        Ok(config)
    }
}

/// Storage backend configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    #[serde(default)]
    pub circuit_breaker: CircuitBreakerConfig,

    #[serde(default)]
    pub retry: RetryConfig,

    #[serde(default)]
    pub manifest_retry: RetryConfig,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            circuit_breaker: CircuitBreakerConfig::default(),
            retry: RetryConfig::default(),
            manifest_retry: RetryConfig {
                max_attempts: 20,
                initial_backoff_ms: 50,
                max_backoff_ms: 2000,
                backoff_multiplier: 2.0,
            },
        }
    }
}

/// Circuit breaker configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening circuit
    pub failure_threshold: u32,

    /// Seconds to wait before attempting recovery
    pub recovery_timeout_secs: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout_secs: 30,
        }
    }
}

impl CircuitBreakerConfig {
    /// Convert recovery timeout to Duration
    pub fn recovery_timeout(&self) -> Duration {
        Duration::from_secs(self.recovery_timeout_secs)
    }
}

/// Retry configuration for operations
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,

    /// Initial backoff delay in milliseconds
    pub initial_backoff_ms: u64,

    /// Maximum backoff delay in milliseconds
    pub max_backoff_ms: u64,

    /// Backoff multiplier for exponential backoff
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 10,
            initial_backoff_ms: 100,
            max_backoff_ms: 5000,
            backoff_multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    /// Get initial backoff duration
    pub fn initial_backoff(&self) -> Duration {
        Duration::from_millis(self.initial_backoff_ms)
    }

    /// Get maximum backoff duration
    pub fn max_backoff(&self) -> Duration {
        Duration::from_millis(self.max_backoff_ms)
    }

    /// Calculate backoff delay for a given retry attempt
    pub fn backoff_for_attempt(&self, attempt: u32) -> Duration {
        let delay_ms = (self.initial_backoff_ms as f64
            * self.backoff_multiplier.powi(attempt as i32))
        .min(self.max_backoff_ms as f64);
        Duration::from_millis(delay_ms as u64)
    }
}

/// Index provider configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct IndexConfig {
    #[serde(default)]
    pub hnsw: HnswIndexConfig,

    #[serde(default)]
    pub native: NativeIndexConfig,
}

/// HNSW index configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HnswIndexConfig {
    /// M: Max connections per layer (typical: 12-48)
    pub m: usize,

    /// efConstruction: Search width during index build (typical: 100-800)
    pub ef_construction: usize,

    /// efSearch: Search width during queries (typical: 50-400)
    pub ef_search: usize,

    /// Minimum number of vectors to use HNSW (below this, use brute force)
    pub min_vectors_threshold: usize,
}

impl Default for HnswIndexConfig {
    fn default() -> Self {
        Self {
            m: 16,
            ef_construction: 400,
            ef_search: 200,
            min_vectors_threshold: 100,
        }
    }
}

/// Native (brute force) index configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NativeIndexConfig {
    /// Maximum vectors before recommending HNSW
    pub max_vectors: usize,
}

impl Default for NativeIndexConfig {
    fn default() -> Self {
        Self { max_vectors: 10000 }
    }
}

/// API configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ApiConfig {
    #[serde(default)]
    pub validation: ValidationConfig,
}

/// API validation limits
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ValidationConfig {
    /// Maximum collection name length
    pub collection_name_max_length: usize,

    /// Maximum vector dimension
    pub vector_dimension_max: u16,

    /// Minimum vector dimension
    pub vector_dimension_min: u16,

    /// Maximum top_k value
    pub top_k_max: u16,

    /// Minimum top_k value
    pub top_k_min: u16,

    /// Maximum batch size for operations
    pub batch_size_max: usize,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            collection_name_max_length: 255,
            vector_dimension_max: 4096,
            vector_dimension_min: 1,
            top_k_max: 1000,
            top_k_min: 1,
            batch_size_max: 1000,
        }
    }
}

/// Query engine configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QueryConfig {
    /// Maximum filter nesting depth
    pub max_filter_depth: usize,

    /// Maximum number of filter clauses
    pub max_filter_clauses: usize,

    /// Enable parallel segment scanning
    pub parallel_segments: bool,

    /// Maximum number of segments to scan in parallel
    pub max_parallel_segments: usize,
}

impl Default for QueryConfig {
    fn default() -> Self {
        Self {
            max_filter_depth: 32,
            max_filter_clauses: 100,
            parallel_segments: true,
            max_parallel_segments: 8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_configuration() {
        let config = AkidbConfig::default();

        // Storage configuration
        assert_eq!(config.storage.circuit_breaker.failure_threshold, 5);
        assert_eq!(config.storage.circuit_breaker.recovery_timeout_secs, 30);
        assert_eq!(config.storage.retry.max_attempts, 10);
        assert_eq!(config.storage.manifest_retry.max_attempts, 20);

        // Index configuration
        assert_eq!(config.index.hnsw.m, 16);
        assert_eq!(config.index.hnsw.ef_construction, 400);
        assert_eq!(config.index.hnsw.ef_search, 200);
        assert_eq!(config.index.hnsw.min_vectors_threshold, 100);

        // API configuration
        assert_eq!(config.api.validation.collection_name_max_length, 255);
        assert_eq!(config.api.validation.vector_dimension_max, 4096);
        assert_eq!(config.api.validation.top_k_max, 1000);

        // Query configuration
        assert_eq!(config.query.max_filter_depth, 32);
        assert_eq!(config.query.max_filter_clauses, 100);
    }

    #[test]
    fn test_retry_config_backoff() {
        let retry = RetryConfig::default();

        // Initial backoff
        assert_eq!(retry.backoff_for_attempt(0).as_millis(), 100);

        // Exponential backoff
        assert_eq!(retry.backoff_for_attempt(1).as_millis(), 200);
        assert_eq!(retry.backoff_for_attempt(2).as_millis(), 400);

        // Max backoff cap
        let long_backoff = retry.backoff_for_attempt(10);
        assert!(long_backoff.as_millis() <= 5000);
    }

    #[test]
    fn test_circuit_breaker_config() {
        let cb = CircuitBreakerConfig::default();
        assert_eq!(cb.recovery_timeout().as_secs(), 30);
    }

    #[test]
    fn test_validation_errors() {
        let mut config = AkidbConfig::default();

        // Invalid: failure_threshold = 0
        config.storage.circuit_breaker.failure_threshold = 0;
        assert!(config.validate().is_err());

        // Fix and validate again
        config.storage.circuit_breaker.failure_threshold = 5;
        assert!(config.validate().is_ok());

        // Invalid: dimension max < min
        config.api.validation.vector_dimension_max = 10;
        config.api.validation.vector_dimension_min = 100;
        assert!(config.validate().is_err());
    }
}
