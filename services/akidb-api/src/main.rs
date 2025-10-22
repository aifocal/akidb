use akidb_api::run_server;
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() {
    init_tracing();

    if let Err(err) = run_server().await {
        tracing::error!(error = %err, "Server terminated with error");
        std::process::exit(1);
    }
}

fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(env_filter).with_target(false).init();
}
