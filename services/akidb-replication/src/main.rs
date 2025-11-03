use clap::{Parser, Subcommand};
use tracing_subscriber::{fmt, EnvFilter};

mod config;
mod failover;
mod monitor;
mod setup;

use config::ReplicationConfig;
use failover::trigger_failover;
use monitor::check_status;
use setup::configure_replication;

#[derive(Parser, Debug)]
#[command(name = "akidb-replication")]
#[command(about = "AkiDB multi-site replication management tool", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Set up replication between two MinIO sites
    Setup {
        /// Primary MinIO endpoint (e.g., https://minio-us-west.example.com)
        #[arg(long)]
        primary: String,

        /// DR/Secondary MinIO endpoint (e.g., https://minio-us-east.example.com)
        #[arg(long)]
        dr: String,

        /// S3 bucket name to replicate
        #[arg(long)]
        bucket: String,

        /// MinIO access key for primary site
        #[arg(long, env = "MINIO_PRIMARY_ACCESS_KEY")]
        primary_access_key: String,

        /// MinIO secret key for primary site
        #[arg(long, env = "MINIO_PRIMARY_SECRET_KEY")]
        primary_secret_key: String,

        /// MinIO access key for DR site
        #[arg(long, env = "MINIO_DR_ACCESS_KEY")]
        dr_access_key: String,

        /// MinIO secret key for DR site
        #[arg(long, env = "MINIO_DR_SECRET_KEY")]
        dr_secret_key: String,

        /// Bandwidth limit (e.g., "100MB/s")
        #[arg(long)]
        bandwidth_limit: Option<String>,

        /// Replication mode (async or sync)
        #[arg(long, default_value = "async")]
        mode: String,
    },

    /// Check replication status
    Status {
        /// Primary MinIO endpoint
        #[arg(long)]
        primary: String,

        /// DR MinIO endpoint
        #[arg(long)]
        dr: String,

        /// MinIO access key
        #[arg(long, env = "MINIO_ACCESS_KEY")]
        access_key: String,

        /// MinIO secret key
        #[arg(long, env = "MINIO_SECRET_KEY")]
        secret_key: String,

        /// S3 bucket name
        #[arg(long)]
        bucket: String,
    },

    /// Trigger failover to DR site
    Failover {
        /// Target site to promote (primary or dr)
        #[arg(long)]
        to: String,

        /// Primary MinIO endpoint
        #[arg(long)]
        primary: String,

        /// DR MinIO endpoint
        #[arg(long)]
        dr: String,

        /// MinIO access key
        #[arg(long, env = "MINIO_ACCESS_KEY")]
        access_key: String,

        /// MinIO secret key
        #[arg(long, env = "MINIO_SECRET_KEY")]
        secret_key: String,

        /// Force failover even if primary is healthy
        #[arg(long, default_value = "false")]
        force: bool,
    },

    /// Generate replication configuration file
    GenerateConfig {
        /// Output configuration file path
        #[arg(long, default_value = "replication.yaml")]
        output: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    init_logging();

    let cli = Cli::parse();

    match cli.command {
        Commands::Setup {
            primary,
            dr,
            bucket,
            primary_access_key,
            primary_secret_key,
            dr_access_key,
            dr_secret_key,
            bandwidth_limit,
            mode,
        } => {
            println!("🔄 Setting up replication between sites...");
            println!("  Primary: {}", primary);
            println!("  DR: {}", dr);
            println!("  Bucket: {}", bucket);
            println!("  Mode: {}", mode);

            let config = ReplicationConfig {
                primary_endpoint: primary,
                dr_endpoint: dr,
                bucket,
                primary_access_key,
                primary_secret_key,
                dr_access_key,
                dr_secret_key,
                bandwidth_limit,
                mode,
            };

            configure_replication(config).await?;

            println!("✅ Replication configured successfully!");
            println!("\nNext steps:");
            println!("  1. Run 'akidb-replication status' to verify replication is working");
            println!("  2. Monitor replication lag with Prometheus metrics");
            println!("  3. Set up alerts for replication failures");

            Ok(())
        }

        Commands::Status {
            primary,
            dr,
            access_key,
            secret_key,
            bucket,
        } => {
            println!("📊 Checking replication status...\n");

            let status = check_status(primary, dr, bucket, access_key, secret_key).await?;

            println!("Replication Status:");
            println!("  Primary → DR:");
            println!("    Replicated: {} bytes", status.primary_to_dr_bytes);
            println!("    Lag: {} seconds", status.primary_to_dr_lag_secs);
            println!("    Status: {}", status.primary_to_dr_status);
            println!("\n  DR → Primary:");
            println!("    Replicated: {} bytes", status.dr_to_primary_bytes);
            println!("    Lag: {} seconds", status.dr_to_primary_lag_secs);
            println!("    Status: {}", status.dr_to_primary_status);
            println!("\n  Overall Health: {}", status.overall_health);

            if status.overall_health == "degraded" || status.overall_health == "unhealthy" {
                println!("\n⚠️  Replication is not healthy! Check MinIO logs.");
                std::process::exit(1);
            }

            Ok(())
        }

        Commands::Failover {
            to,
            primary,
            dr,
            access_key,
            secret_key,
            force,
        } => {
            println!("🔀 Triggering failover to '{}'...", to);

            if !force {
                println!("⚠️  WARNING: This will promote {} to primary!", to);
                println!("Continue? (y/N)");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Aborted.");
                    return Ok(());
                }
            }

            trigger_failover(to, primary, dr, access_key, secret_key, force).await?;

            println!("✅ Failover complete!");
            println!("\nPost-failover checklist:");
            println!("  1. Update DNS records to point to new primary");
            println!("  2. Update load balancer configuration");
            println!("  3. Verify application connectivity");
            println!("  4. Monitor replication from new primary");

            Ok(())
        }

        Commands::GenerateConfig { output } => {
            println!("📝 Generating replication configuration template...");

            let template = r#"# AkiDB MinIO Replication Configuration
# Generated by akidb-replication v0.4.0

primary:
  endpoint: https://minio-us-west.example.com
  access_key: ${MINIO_PRIMARY_ACCESS_KEY}
  secret_key: ${MINIO_PRIMARY_SECRET_KEY}
  region: us-west-1

dr:
  endpoint: https://minio-us-east.example.com
  access_key: ${MINIO_DR_ACCESS_KEY}
  secret_key: ${MINIO_DR_SECRET_KEY}
  region: us-east-1

replication:
  bucket: akidb
  mode: async  # async or sync
  bandwidth_limit: 100MB/s  # Optional

  # Replication rules
  rules:
    - prefix: collections/
      priority: 1
      enabled: true
    - prefix: segments/
      priority: 2
      enabled: true
    - prefix: manifests/
      priority: 3
      enabled: true

monitoring:
  prometheus_endpoint: http://localhost:9090
  alert_threshold_lag_seconds: 300  # 5 minutes
  health_check_interval_seconds: 60

failover:
  auto_failover_enabled: false
  max_lag_seconds: 600  # 10 minutes
  min_healthy_checks: 3
"#;

            std::fs::write(&output, template)?;
            println!("✅ Configuration template written to: {}", output);
            println!("\nEdit the file and use it with:");
            println!("  akidb-replication setup --config {}", output);

            Ok(())
        }
    }
}

fn init_logging() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(env_filter).with_target(false).init();
}
