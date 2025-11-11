//! Configuration for batch S3 uploads

/// Batch upload configuration
#[derive(Debug, Clone)]
pub struct S3BatchConfig {
    /// Batch size (number of documents per Parquet file)
    pub batch_size: usize,
    /// Maximum wait time before flushing partial batch (ms)
    pub max_wait_ms: u64,
    /// Enable Parquet compression
    pub enable_compression: bool,
}

impl Default for S3BatchConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,          // 100 docs per batch
            max_wait_ms: 5000,        // 5 seconds max wait
            enable_compression: true, // Snappy compression
        }
    }
}

impl S3BatchConfig {
    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.batch_size == 0 {
            return Err("batch_size must be > 0".to_string());
        }

        if self.batch_size > 10_000 {
            return Err("batch_size too large (max: 10,000)".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_config_defaults() {
        let config = S3BatchConfig::default();
        assert_eq!(config.batch_size, 100);
        assert_eq!(config.max_wait_ms, 5000);
        assert!(config.enable_compression);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_batch_config_validation() {
        let mut config = S3BatchConfig::default();

        // Zero batch size
        config.batch_size = 0;
        assert!(config.validate().is_err());

        // Too large
        config.batch_size = 20_000;
        assert!(config.validate().is_err());

        // Valid
        config.batch_size = 100;
        assert!(config.validate().is_ok());
    }
}
