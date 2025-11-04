use serde::{Deserialize, Serialize};

/// Replication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationConfig {
    pub primary_endpoint: String,
    pub dr_endpoint: String,
    pub bucket: String,
    pub primary_access_key: String,
    pub primary_secret_key: String,
    pub dr_access_key: String,
    pub dr_secret_key: String,
    pub bandwidth_limit: Option<String>,
    pub mode: String,
}

impl ReplicationConfig {
    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.primary_endpoint.is_empty() {
            return Err("Primary endpoint cannot be empty".to_string());
        }
        if self.dr_endpoint.is_empty() {
            return Err("DR endpoint cannot be empty".to_string());
        }
        if self.bucket.is_empty() {
            return Err("Bucket name cannot be empty".to_string());
        }
        if self.mode != "async" && self.mode != "sync" {
            return Err(format!(
                "Invalid mode '{}', must be 'async' or 'sync'",
                self.mode
            ));
        }
        Ok(())
    }

    /// Generate MinIO replication command
    pub fn to_minio_command(&self) -> String {
        format!(
            r#"# Configure site replication between primary and DR

# Set up primary site alias
mc alias set primary {} {} {}

# Set up DR site alias
mc alias set dr {} {} {}

# Enable site replication
mc admin replicate add primary dr --bucket {}

# Verify replication status
mc admin replicate info primary
"#,
            self.primary_endpoint,
            self.primary_access_key,
            self.primary_secret_key,
            self.dr_endpoint,
            self.dr_access_key,
            self.dr_secret_key,
            self.bucket
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let config = ReplicationConfig {
            primary_endpoint: "http://primary:9000".to_string(),
            dr_endpoint: "http://dr:9000".to_string(),
            bucket: "akidb".to_string(),
            primary_access_key: "key1".to_string(),
            primary_secret_key: "secret1".to_string(),
            dr_access_key: "key2".to_string(),
            dr_secret_key: "secret2".to_string(),
            bandwidth_limit: Some("100MB/s".to_string()),
            mode: "async".to_string(),
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_mode() {
        let config = ReplicationConfig {
            primary_endpoint: "http://primary:9000".to_string(),
            dr_endpoint: "http://dr:9000".to_string(),
            bucket: "akidb".to_string(),
            primary_access_key: "key1".to_string(),
            primary_secret_key: "secret1".to_string(),
            dr_access_key: "key2".to_string(),
            dr_secret_key: "secret2".to_string(),
            bandwidth_limit: None,
            mode: "invalid".to_string(),
        };

        assert!(config.validate().is_err());
    }
}
