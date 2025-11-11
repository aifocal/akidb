//! Configuration management for AkiDB servers.
//!
//! Supports multiple configuration sources with precedence:
//! 1. Environment variables (highest priority)
//! 2. TOML configuration file
//! 3. Default values (lowest priority)

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main configuration structure for AkiDB servers.
///
/// Can be loaded from TOML file or constructed with defaults.
/// Environment variables override TOML settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Server configuration
    pub server: ServerConfig,

    /// Database configuration
    pub database: DatabaseConfig,

    /// Optional features
    #[serde(default)]
    pub features: FeaturesConfig,

    /// HNSW index tuning parameters
    #[serde(default)]
    pub hnsw: HnswConfig,

    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingConfig,
}

/// Server configuration (host, port, protocol)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server host address (default: "0.0.0.0")
    #[serde(default = "default_host")]
    pub host: String,

    /// REST API port (default: 8080)
    #[serde(default = "default_rest_port")]
    pub rest_port: u16,

    /// gRPC API port (default: 9090)
    #[serde(default = "default_grpc_port")]
    pub grpc_port: u16,

    /// Request timeout in seconds (default: 30)
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// SQLite database path (default: "sqlite://akidb.db")
    #[serde(default = "default_db_path")]
    pub path: String,

    /// Max connections in pool (default: 10)
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// Connection timeout in seconds (default: 5)
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout_seconds: u64,
}

/// Optional features configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeaturesConfig {
    /// Enable metrics collection (default: true)
    #[serde(default = "default_true")]
    pub metrics_enabled: bool,

    /// Enable vector persistence (default: true)
    #[serde(default = "default_true")]
    pub vector_persistence_enabled: bool,

    /// Enable auto-initialization of default tenant/database (default: true)
    #[serde(default = "default_true")]
    pub auto_initialize: bool,
}

/// HNSW index tuning parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswConfig {
    /// HNSW M parameter (default: 32)
    #[serde(default = "default_hnsw_m")]
    pub m: u32,

    /// HNSW ef_construction parameter (default: 200)
    #[serde(default = "default_hnsw_ef_construction")]
    pub ef_construction: u32,

    /// Max document count threshold for HNSW (default: 10_000)
    /// Collections with fewer documents use brute-force search
    #[serde(default = "default_hnsw_threshold")]
    pub threshold: usize,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level: trace, debug, info, warn, error (default: "info")
    #[serde(default = "default_log_level")]
    pub level: String,

    /// Log format: json or pretty (default: "pretty")
    #[serde(default = "default_log_format")]
    pub format: String,
}

// Default value functions
fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_rest_port() -> u16 {
    8080
}

fn default_grpc_port() -> u16 {
    9090
}

fn default_timeout() -> u64 {
    30
}

fn default_db_path() -> String {
    "sqlite://akidb.db".to_string()
}

fn default_max_connections() -> u32 {
    10
}

fn default_connection_timeout() -> u64 {
    5
}

fn default_true() -> bool {
    true
}

fn default_hnsw_m() -> u32 {
    32
}

fn default_hnsw_ef_construction() -> u32 {
    200
}

fn default_hnsw_threshold() -> usize {
    10_000
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "pretty".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            database: DatabaseConfig::default(),
            features: FeaturesConfig::default(),
            hnsw: HnswConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            rest_port: default_rest_port(),
            grpc_port: default_grpc_port(),
            timeout_seconds: default_timeout(),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: default_db_path(),
            max_connections: default_max_connections(),
            connection_timeout_seconds: default_connection_timeout(),
        }
    }
}

impl Default for FeaturesConfig {
    fn default() -> Self {
        Self {
            metrics_enabled: true,
            vector_persistence_enabled: true,
            auto_initialize: true,
        }
    }
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            m: default_hnsw_m(),
            ef_construction: default_hnsw_ef_construction(),
            threshold: default_hnsw_threshold(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: default_log_format(),
        }
    }
}

impl Config {
    /// Load configuration from a TOML file.
    ///
    /// Returns an error if the file doesn't exist or has invalid TOML syntax.
    pub fn from_file(path: impl Into<PathBuf>) -> Result<Self, ConfigError> {
        let path = path.into();
        let contents = std::fs::read_to_string(&path).map_err(|e| ConfigError::IoError {
            path: path.clone(),
            source: e,
        })?;

        toml::from_str(&contents).map_err(|e| ConfigError::TomlError { path, source: e })
    }

    /// Load configuration with environment variable overrides.
    ///
    /// Loads from TOML file if it exists, otherwise uses defaults.
    /// Environment variables override TOML/default values.
    ///
    /// Supported environment variables:
    /// - `AKIDB_HOST` - Server host address
    /// - `AKIDB_REST_PORT` - REST API port
    /// - `AKIDB_GRPC_PORT` - gRPC API port
    /// - `AKIDB_DB_PATH` - Database path
    /// - `AKIDB_LOG_LEVEL` - Log level
    pub fn load() -> Result<Self, ConfigError> {
        // Try to load from config.toml, otherwise use defaults
        let mut config = if std::path::Path::new("config.toml").exists() {
            Self::from_file("config.toml")?
        } else {
            Self::default()
        };

        // Apply environment variable overrides
        config.apply_env_overrides();

        Ok(config)
    }

    /// Apply environment variable overrides to the configuration.
    ///
    /// This allows environment variables to override TOML settings.
    pub fn apply_env_overrides(&mut self) {
        if let Ok(host) = std::env::var("AKIDB_HOST") {
            self.server.host = host;
        }

        if let Ok(port) = std::env::var("AKIDB_REST_PORT") {
            if let Ok(port) = port.parse() {
                self.server.rest_port = port;
            }
        }

        if let Ok(port) = std::env::var("AKIDB_GRPC_PORT") {
            if let Ok(port) = port.parse() {
                self.server.grpc_port = port;
            }
        }

        if let Ok(path) = std::env::var("AKIDB_DB_PATH") {
            self.database.path = path;
        }

        if let Ok(level) = std::env::var("AKIDB_LOG_LEVEL") {
            self.logging.level = level;
        }

        if let Ok(format) = std::env::var("AKIDB_LOG_FORMAT") {
            self.logging.format = format;
        }

        if let Ok(enabled) = std::env::var("AKIDB_METRICS_ENABLED") {
            if let Ok(enabled) = enabled.parse() {
                self.features.metrics_enabled = enabled;
            }
        }

        if let Ok(enabled) = std::env::var("AKIDB_VECTOR_PERSISTENCE_ENABLED") {
            if let Ok(enabled) = enabled.parse() {
                self.features.vector_persistence_enabled = enabled;
            }
        }
    }

    /// Validate the configuration.
    ///
    /// Returns an error if any configuration values are invalid.
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate ports
        if self.server.rest_port == 0 {
            return Err(ConfigError::ValidationError(
                "rest_port must be non-zero".to_string(),
            ));
        }

        if self.server.grpc_port == 0 {
            return Err(ConfigError::ValidationError(
                "grpc_port must be non-zero".to_string(),
            ));
        }

        if self.server.rest_port == self.server.grpc_port {
            return Err(ConfigError::ValidationError(
                "rest_port and grpc_port must be different".to_string(),
            ));
        }

        // Port ranges are automatically valid for u16 (1-65535)

        // Validate database path
        if self.database.path.is_empty() {
            return Err(ConfigError::ValidationError(
                "database.path cannot be empty".to_string(),
            ));
        }

        // Validate database path parent directory exists
        if let Some(stripped) = self.database.path.strip_prefix("sqlite://") {
            let db_path = PathBuf::from(stripped);

            // Check if it's an absolute path or relative path
            if db_path.is_absolute() {
                // For absolute paths, check if parent directory exists
                if let Some(parent) = db_path.parent() {
                    if !parent.exists() {
                        return Err(ConfigError::ValidationError(format!(
                            "Database parent directory does not exist: {}. Create it first with: mkdir -p {}",
                            parent.display(),
                            parent.display()
                        )));
                    }

                    // Check if parent directory is writable
                    if let Ok(metadata) = parent.metadata() {
                        if metadata.permissions().readonly() {
                            return Err(ConfigError::ValidationError(format!(
                                "Database parent directory is read-only: {}",
                                parent.display()
                            )));
                        }
                    }
                }
            }
            // Relative paths are OK - will be created relative to CWD
        }

        // Validate database pool settings
        if self.database.max_connections == 0 {
            return Err(ConfigError::ValidationError(
                "database.max_connections must be > 0".to_string(),
            ));
        }

        if self.database.max_connections > 1000 {
            return Err(ConfigError::ValidationError(
                "database.max_connections must be <= 1000 (recommended: 5-50)".to_string(),
            ));
        }

        if self.database.connection_timeout_seconds == 0 {
            return Err(ConfigError::ValidationError(
                "database.connection_timeout_seconds must be > 0".to_string(),
            ));
        }

        // Validate server timeout
        if self.server.timeout_seconds == 0 {
            return Err(ConfigError::ValidationError(
                "server.timeout_seconds must be > 0".to_string(),
            ));
        }

        // Validate HNSW parameters
        if self.hnsw.m < 2 || self.hnsw.m > 100 {
            return Err(ConfigError::ValidationError(
                "hnsw.m must be between 2 and 100".to_string(),
            ));
        }

        if self.hnsw.ef_construction < 10 || self.hnsw.ef_construction > 1000 {
            return Err(ConfigError::ValidationError(
                "hnsw.ef_construction must be between 10 and 1000".to_string(),
            ));
        }

        // Validate log level
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&self.logging.level.as_str()) {
            return Err(ConfigError::ValidationError(format!(
                "logging.level must be one of: {}",
                valid_levels.join(", ")
            )));
        }

        // Validate log format
        let valid_formats = ["json", "pretty"];
        if !valid_formats.contains(&self.logging.format.as_str()) {
            return Err(ConfigError::ValidationError(format!(
                "logging.format must be one of: {}",
                valid_formats.join(", ")
            )));
        }

        Ok(())
    }
}

/// Configuration error types
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// I/O error reading configuration file
    #[error("Failed to read config file {path:?}: {source}")]
    IoError {
        path: PathBuf,
        source: std::io::Error,
    },

    /// TOML parsing error
    #[error("Failed to parse TOML in {path:?}: {source}")]
    TomlError {
        path: PathBuf,
        source: toml::de::Error,
    },

    /// Validation error
    #[error("Configuration validation failed: {0}")]
    ValidationError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();

        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.rest_port, 8080);
        assert_eq!(config.server.grpc_port, 9090);
        assert_eq!(config.database.path, "sqlite://akidb.db");
        assert!(config.features.metrics_enabled);
        assert!(config.features.vector_persistence_enabled);
    }

    #[test]
    fn test_config_validation_valid() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_same_ports() {
        let mut config = Config::default();
        config.server.rest_port = 8080;
        config.server.grpc_port = 8080;

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("rest_port and grpc_port must be different"));
    }

    #[test]
    fn test_config_validation_invalid_hnsw_m() {
        let mut config = Config::default();
        config.hnsw.m = 1; // Too small

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("hnsw.m must be"));
    }

    #[test]
    fn test_config_validation_invalid_log_level() {
        let mut config = Config::default();
        config.logging.level = "invalid".to_string();

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("logging.level must be"));
    }

    #[test]
    fn test_toml_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("host"));
        assert!(toml_str.contains("rest_port"));
        assert!(toml_str.contains("grpc_port"));
    }

    #[test]
    fn test_toml_deserialization() {
        let toml_str = r#"
            [server]
            host = "127.0.0.1"
            rest_port = 8081
            grpc_port = 9091

            [database]
            path = "sqlite:///tmp/test.db"

            [features]
            metrics_enabled = false

            [hnsw]
            m = 16
            ef_construction = 100
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.rest_port, 8081);
        assert_eq!(config.server.grpc_port, 9091);
        assert_eq!(config.database.path, "sqlite:///tmp/test.db");
        assert!(!config.features.metrics_enabled);
        assert_eq!(config.hnsw.m, 16);
        assert_eq!(config.hnsw.ef_construction, 100);
    }

    #[test]
    fn test_env_override() {
        std::env::set_var("AKIDB_HOST", "192.168.1.100");
        std::env::set_var("AKIDB_REST_PORT", "9999");
        std::env::set_var("AKIDB_DB_PATH", "sqlite:///custom/path.db");

        let mut config = Config::default();
        config.apply_env_overrides();

        assert_eq!(config.server.host, "192.168.1.100");
        assert_eq!(config.server.rest_port, 9999);
        assert_eq!(config.database.path, "sqlite:///custom/path.db");

        // Clean up
        std::env::remove_var("AKIDB_HOST");
        std::env::remove_var("AKIDB_REST_PORT");
        std::env::remove_var("AKIDB_DB_PATH");
    }
}
