use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing_subscriber::{fmt, EnvFilter};

mod export;
mod import;
mod manifest;
mod verify;

use export::export_package;
use import::import_package;
use verify::verify_package;

#[derive(Parser, Debug)]
#[command(name = "akidb-pkg")]
#[command(about = "AkiDB package management tool for air-gap deployments", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Export a collection to .akipkg format
    Export {
        /// Collection name to export
        #[arg(long)]
        collection: String,

        /// Output .akipkg file path
        #[arg(long)]
        output: PathBuf,

        /// Private key for signing (Ed25519)
        #[arg(long)]
        sign_key: Option<PathBuf>,

        /// S3 endpoint
        #[arg(
            long,
            env = "AKIDB_S3_ENDPOINT",
            default_value = "http://localhost:9000"
        )]
        s3_endpoint: String,

        /// S3 access key
        #[arg(long, env = "AKIDB_S3_ACCESS_KEY")]
        s3_access_key: String,

        /// S3 secret key
        #[arg(long, env = "AKIDB_S3_SECRET_KEY")]
        s3_secret_key: String,

        /// S3 bucket
        #[arg(long, env = "AKIDB_S3_BUCKET", default_value = "akidb")]
        s3_bucket: String,

        /// S3 region
        #[arg(long, env = "AKIDB_S3_REGION", default_value = "us-east-1")]
        s3_region: String,
    },

    /// Verify an .akipkg package integrity
    Verify {
        /// .akipkg file to verify
        #[arg(long)]
        file: PathBuf,

        /// Public key for signature verification
        #[arg(long)]
        public_key: Option<PathBuf>,
    },

    /// Import an .akipkg package
    Import {
        /// .akipkg file to import
        #[arg(long)]
        file: PathBuf,

        /// Target collection name (defaults to original name)
        #[arg(long)]
        collection: Option<String>,

        /// Verify signature before importing
        #[arg(long, default_value = "true")]
        verify_signature: bool,

        /// S3 endpoint
        #[arg(
            long,
            env = "AKIDB_S3_ENDPOINT",
            default_value = "http://localhost:9000"
        )]
        s3_endpoint: String,

        /// S3 access key
        #[arg(long, env = "AKIDB_S3_ACCESS_KEY")]
        s3_access_key: String,

        /// S3 secret key
        #[arg(long, env = "AKIDB_S3_SECRET_KEY")]
        s3_secret_key: String,

        /// S3 bucket
        #[arg(long, env = "AKIDB_S3_BUCKET", default_value = "akidb")]
        s3_bucket: String,

        /// S3 region
        #[arg(long, env = "AKIDB_S3_REGION", default_value = "us-east-1")]
        s3_region: String,
    },

    /// Inspect an .akipkg package
    Inspect {
        /// .akipkg file to inspect
        #[arg(long)]
        file: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    init_logging();

    let cli = Cli::parse();

    match cli.command {
        Commands::Export {
            collection,
            output,
            sign_key,
            s3_endpoint,
            s3_access_key,
            s3_secret_key,
            s3_bucket,
            s3_region,
        } => {
            println!("📦 Exporting collection '{}' to {}...", collection, output.display());

            export_package(
                collection,
                output,
                sign_key,
                s3_endpoint,
                s3_access_key,
                s3_secret_key,
                s3_bucket,
                s3_region,
            )
            .await?;

            println!("✅ Export complete!");
            Ok(())
        }

        Commands::Verify { file, public_key } => {
            println!("🔍 Verifying package {}...", file.display());

            let result = verify_package(file, public_key).await?;

            if result.is_valid() {
                println!("✅ Package is valid!");
                println!("  Collection: {}", result.manifest.collection_name);
                println!("  Vectors: {}", result.manifest.total_vectors);
                println!("  Segments: {}", result.manifest.total_segments);
                println!("  Size: {} bytes", result.manifest.compressed_size_bytes);
                if result.signature_valid {
                    println!("  Signature: ✅ Valid");
                } else {
                    println!("  Signature: ⚠️  Not verified (no public key provided)");
                }
            } else {
                println!("❌ Package verification failed!");
                for error in &result.errors {
                    println!("  - {}", error);
                }
                std::process::exit(1);
            }

            Ok(())
        }

        Commands::Import {
            file,
            collection,
            verify_signature,
            s3_endpoint,
            s3_access_key,
            s3_secret_key,
            s3_bucket,
            s3_region,
        } => {
            println!("📥 Importing package {}...", file.display());

            import_package(
                file,
                collection,
                verify_signature,
                s3_endpoint,
                s3_access_key,
                s3_secret_key,
                s3_bucket,
                s3_region,
            )
            .await?;

            println!("✅ Import complete!");
            Ok(())
        }

        Commands::Inspect { file } => {
            println!("🔍 Inspecting package {}...", file.display());

            let result = verify_package(file, None).await?;

            println!("\nPackage Information:");
            println!("  Collection: {}", result.manifest.collection_name);
            println!("  Version: {}", result.manifest.version);
            println!("  Snapshot Version: {}", result.manifest.snapshot_version);
            println!("  Created: {}", result.manifest.created_at);
            println!("  AkiDB Version: {}", result.manifest.akidb_version);
            println!("\nData:");
            println!("  Total Vectors: {}", result.manifest.total_vectors);
            println!("  Total Segments: {}", result.manifest.total_segments);
            println!("  Vector Dimension: {}", result.manifest.vector_dim);
            println!("  Distance Metric: {}", result.manifest.distance_metric);
            println!("\nSize:");
            println!(
                "  Compressed: {} bytes ({:.2} MB)",
                result.manifest.compressed_size_bytes,
                result.manifest.compressed_size_bytes as f64 / 1_000_000.0
            );
            println!(
                "  Uncompressed: {} bytes ({:.2} MB)",
                result.manifest.uncompressed_size_bytes,
                result.manifest.uncompressed_size_bytes as f64 / 1_000_000.0
            );
            println!(
                "  Compression Ratio: {:.2}x",
                result.manifest.uncompressed_size_bytes as f64
                    / result.manifest.compressed_size_bytes as f64
            );

            Ok(())
        }
    }
}

fn init_logging() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(env_filter).with_target(false).init();
}
