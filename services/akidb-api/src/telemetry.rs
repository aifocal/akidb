// ! OpenTelemetry and Jaeger integration for distributed tracing
//!
//! This module provides:
//! - OpenTelemetry tracing layer setup
//! - Jaeger exporter configuration
//! - Trace context propagation
//! - Span lifecycle management

use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::{RandomIdGenerator, Sampler, TracerProvider};
use opentelemetry_sdk::Resource;
use std::time::Duration;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// OpenTelemetry configuration
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// Service name for traces
    pub service_name: String,

    /// Jaeger endpoint (e.g., "http://localhost:4317")
    pub jaeger_endpoint: String,

    /// Enable telemetry (can be disabled in development)
    pub enabled: bool,

    /// Sampling ratio (0.0 to 1.0)
    pub sampling_ratio: f64,

    /// Export timeout in seconds
    pub export_timeout_secs: u64,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            service_name: "akidb-api".to_string(),
            jaeger_endpoint: "http://localhost:4317".to_string(),
            enabled: true,
            sampling_ratio: 1.0, // Sample all traces
            export_timeout_secs: 10,
        }
    }
}

impl TelemetryConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            service_name: std::env::var("AKIDB_SERVICE_NAME")
                .unwrap_or_else(|_| "akidb-api".to_string()),
            jaeger_endpoint: std::env::var("AKIDB_JAEGER_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:4317".to_string()),
            enabled: std::env::var("AKIDB_TELEMETRY_ENABLED")
                .map(|v| v.to_lowercase() == "true" || v == "1")
                .unwrap_or(true),
            sampling_ratio: std::env::var("AKIDB_SAMPLING_RATIO")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
            export_timeout_secs: std::env::var("AKIDB_EXPORT_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
        }
    }
}

/// Initialize OpenTelemetry with Jaeger exporter
///
/// This function sets up:
/// - Global trace propagator (W3C Trace Context)
/// - OTLP exporter to Jaeger
/// - Tracing subscriber with OpenTelemetry layer
///
/// # Returns
///
/// A guard that should be kept alive for the duration of the application.
/// Dropping the guard will flush and shutdown the tracer.
pub fn init_telemetry(config: TelemetryConfig) -> Result<TelemetryGuard, Box<dyn std::error::Error>> {
    if !config.enabled {
        tracing::info!("OpenTelemetry is disabled (AKIDB_TELEMETRY_ENABLED=false)");
        init_logging_only();
        return Ok(TelemetryGuard { provider: None });
    }

    tracing::info!(
        "Initializing OpenTelemetry: service={}, endpoint={}",
        config.service_name,
        config.jaeger_endpoint
    );

    // Set global propagator for trace context
    global::set_text_map_propagator(TraceContextPropagator::new());

    // Create OTLP exporter to Jaeger
    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint(&config.jaeger_endpoint)
        .with_timeout(Duration::from_secs(config.export_timeout_secs));

    // Build tracer provider
    let provider = TracerProvider::builder()
        .with_batch_exporter(
            opentelemetry_otlp::new_pipeline()
                .tracing()
                .with_exporter(exporter)
                .install_batch(opentelemetry_sdk::runtime::Tokio)?,
        )
        .with_sampler(Sampler::TraceIdRatioBased(config.sampling_ratio))
        .with_id_generator(RandomIdGenerator::default())
        .with_resource(Resource::new(vec![
            opentelemetry::KeyValue::new("service.name", config.service_name.clone()),
            opentelemetry::KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
        ]))
        .build();

    // Set global tracer provider
    global::set_tracer_provider(provider.clone());

    // Create OpenTelemetry tracing layer
    let tracer = provider.tracer("akidb-api");
    let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    // Create env filter for log levels
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    // Create fmt layer for console logging
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true);

    // Initialize global subscriber with both layers
    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(telemetry_layer)
        .try_init()?;

    tracing::info!("OpenTelemetry initialized successfully");

    Ok(TelemetryGuard {
        provider: Some(provider),
    })
}

/// Initialize logging without OpenTelemetry (fallback)
fn init_logging_only() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    tracing::info!("Logging initialized (OpenTelemetry disabled)");
}

/// Guard to ensure telemetry is properly shutdown
///
/// When dropped, this will flush all pending spans and shutdown the tracer provider.
pub struct TelemetryGuard {
    provider: Option<TracerProvider>,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(provider) = self.provider.take() {
            tracing::info!("Shutting down OpenTelemetry...");

            // Shutdown provider (flushes pending spans)
            if let Err(e) = provider.shutdown() {
                eprintln!("Error shutting down tracer provider: {}", e);
            }

            // Shutdown global tracer provider
            global::shutdown_tracer_provider();

            tracing::info!("OpenTelemetry shutdown complete");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TelemetryConfig::default();
        assert_eq!(config.service_name, "akidb-api");
        assert_eq!(config.jaeger_endpoint, "http://localhost:4317");
        assert_eq!(config.enabled, true);
        assert_eq!(config.sampling_ratio, 1.0);
        assert_eq!(config.export_timeout_secs, 10);
    }

    #[test]
    fn test_config_from_env() {
        std::env::set_var("AKIDB_SERVICE_NAME", "test-service");
        std::env::set_var("AKIDB_JAEGER_ENDPOINT", "http://jaeger:4317");
        std::env::set_var("AKIDB_TELEMETRY_ENABLED", "true");
        std::env::set_var("AKIDB_SAMPLING_RATIO", "0.5");

        let config = TelemetryConfig::from_env();
        assert_eq!(config.service_name, "test-service");
        assert_eq!(config.jaeger_endpoint, "http://jaeger:4317");
        assert_eq!(config.enabled, true);
        assert_eq!(config.sampling_ratio, 0.5);

        // Cleanup
        std::env::remove_var("AKIDB_SERVICE_NAME");
        std::env::remove_var("AKIDB_JAEGER_ENDPOINT");
        std::env::remove_var("AKIDB_TELEMETRY_ENABLED");
        std::env::remove_var("AKIDB_SAMPLING_RATIO");
    }

    #[test]
    fn test_disabled_telemetry() {
        std::env::set_var("AKIDB_TELEMETRY_ENABLED", "false");

        let config = TelemetryConfig::from_env();
        assert_eq!(config.enabled, false);

        std::env::remove_var("AKIDB_TELEMETRY_ENABLED");
    }
}
