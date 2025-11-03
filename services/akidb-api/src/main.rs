use akidb_api::{run_server, telemetry};

#[tokio::main]
async fn main() {
    // Initialize OpenTelemetry + Jaeger (or logging-only fallback)
    let telemetry_config = telemetry::TelemetryConfig::from_env();
    let _guard = match telemetry::init_telemetry(telemetry_config) {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Failed to initialize telemetry: {}", e);
            std::process::exit(1);
        }
    };

    // Run server
    if let Err(err) = run_server().await {
        tracing::error!(error = %err, "Server terminated with error");
        std::process::exit(1);
    }

    // Guard is dropped here, ensuring proper telemetry shutdown
}
