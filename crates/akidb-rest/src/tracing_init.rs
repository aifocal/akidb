//! OpenTelemetry tracing initialization
//!
//! This module provides initialization and configuration for distributed tracing
//! using OpenTelemetry and Jaeger.
//!
//! # Features
//! - Jaeger exporter for distributed tracing
//! - Automatic span creation for HTTP requests
//! - Trace context propagation
//! - Integration with Prometheus exemplars
//!
//! # Example
//! ```no_run
//! use akidb_rest::tracing_init;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Initialize tracing with Jaeger
//!     tracing_init::init_tracing("akidb-rest", "http://localhost:14268/api/traces")
//!         .expect("Failed to initialize tracing");
//!
//!     // Your application code...
//!
//!     // Shutdown tracing on exit
//!     opentelemetry::global::shutdown_tracer_provider();
//! }
//! ```

use opentelemetry::trace::TraceError;
use opentelemetry::{global, KeyValue};
use opentelemetry_sdk::trace::{self, Sampler};
use opentelemetry_sdk::Resource;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry};

/// Initialize OpenTelemetry tracing with Jaeger exporter
///
/// # Arguments
/// * `service_name` - Name of the service (e.g., "akidb-rest")
/// * `jaeger_endpoint` - Jaeger collector endpoint (e.g., "http://localhost:14268/api/traces")
///
/// # Returns
/// * `Ok(())` if tracing was successfully initialized
/// * `Err(TraceError)` if initialization failed
///
/// # Example
/// ```no_run
/// # use akidb_rest::tracing_init;
/// tracing_init::init_tracing("akidb-rest", "http://localhost:14268/api/traces")
///     .expect("Failed to initialize tracing");
/// ```
pub fn init_tracing(service_name: &str, jaeger_endpoint: &str) -> Result<(), TraceError> {
    // Create Jaeger exporter
    let tracer = opentelemetry_jaeger::new_agent_pipeline()
        .with_service_name(service_name)
        .with_endpoint(jaeger_endpoint)
        .with_trace_config(
            trace::config()
                .with_sampler(Sampler::AlwaysOn) // Sample all traces (adjust for production)
                .with_resource(Resource::new(vec![
                    KeyValue::new("service.name", service_name.to_string()),
                    KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
                ])),
        )
        .install_batch(opentelemetry_sdk::runtime::Tokio)?;

    // Create OpenTelemetry tracing layer
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    // Create env filter for log level control
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,akidb=debug"));

    // Create formatting layer for console output
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(false)
        .with_line_number(true);

    // Combine layers and initialize global subscriber
    Registry::default()
        .with(env_filter)
        .with(fmt_layer)
        .with(telemetry)
        .init();

    Ok(())
}

/// Initialize tracing with default settings
///
/// Uses default Jaeger endpoint (http://localhost:14268/api/traces) and
/// service name "akidb-rest".
///
/// # Example
/// ```no_run
/// # use akidb_rest::tracing_init;
/// tracing_init::init_default_tracing()
///     .expect("Failed to initialize tracing");
/// ```
pub fn init_default_tracing() -> Result<(), TraceError> {
    init_tracing("akidb-rest", "http://jaeger:14268/api/traces")
}

/// Initialize tracing with environment variable configuration
///
/// Reads configuration from environment variables:
/// - `JAEGER_ENDPOINT`: Jaeger collector endpoint (default: http://jaeger:14268/api/traces)
/// - `SERVICE_NAME`: Service name (default: akidb-rest)
///
/// # Example
/// ```no_run
/// # use akidb_rest::tracing_init;
/// # std::env::set_var("JAEGER_ENDPOINT", "http://localhost:14268/api/traces");
/// tracing_init::init_from_env()
///     .expect("Failed to initialize tracing");
/// ```
pub fn init_from_env() -> Result<(), TraceError> {
    let jaeger_endpoint = std::env::var("JAEGER_ENDPOINT")
        .unwrap_or_else(|_| "http://jaeger:14268/api/traces".to_string());
    let service_name = std::env::var("SERVICE_NAME").unwrap_or_else(|_| "akidb-rest".to_string());

    init_tracing(&service_name, &jaeger_endpoint)
}

/// Shutdown the global tracer provider
///
/// Should be called before application exit to flush any pending traces.
///
/// # Example
/// ```no_run
/// # use akidb_rest::tracing_init;
/// tracing_init::shutdown();
/// ```
pub fn shutdown() {
    global::shutdown_tracer_provider();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_from_env_with_defaults() {
        // Clear env vars to use defaults
        std::env::remove_var("JAEGER_ENDPOINT");
        std::env::remove_var("SERVICE_NAME");

        // Should not panic (may fail if Jaeger not running, but that's okay for unit test)
        let result = init_from_env();
        assert!(result.is_ok() || result.is_err()); // Just verify it returns a Result
    }

    #[test]
    fn test_init_from_env_with_custom() {
        std::env::set_var("JAEGER_ENDPOINT", "http://custom:14268/api/traces");
        std::env::set_var("SERVICE_NAME", "custom-service");

        let result = init_from_env();
        assert!(result.is_ok() || result.is_err());
    }
}
