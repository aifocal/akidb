use clap::{Parser, ValueEnum};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

pub mod language;
mod parsers;
mod pipeline;

use parsers::{CsvParser, JsonlParser, ParquetParser, VectorParser};
use pipeline::IngestPipeline;

#[derive(Parser, Debug)]
#[command(name = "akidb-ingest")]
#[command(about = "AkiDB offline batch vector ingest tool", long_about = None)]
#[command(version)]
struct Cli {
    /// Collection name to ingest into
    #[arg(long)]
    collection: String,

    /// Input file path (CSV, JSONL, or Parquet)
    #[arg(long)]
    file: PathBuf,

    /// File format (auto-detected if not specified)
    #[arg(long, value_enum)]
    format: Option<FileFormat>,

    /// Name of the ID column
    #[arg(long, default_value = "id")]
    id_column: String,

    /// Name of the vector column
    #[arg(long, default_value = "vector")]
    vector_column: String,

    /// Comma-separated list of payload columns
    #[arg(long, value_delimiter = ',')]
    payload_columns: Vec<String>,

    /// Batch size for inserts (vectors per batch)
    #[arg(long, default_value = "10000")]
    batch_size: usize,

    /// Number of parallel parsing threads
    #[arg(long, default_value = "8")]
    parallel: usize,

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

    /// S3 bucket name
    #[arg(long, env = "AKIDB_S3_BUCKET", default_value = "akidb")]
    s3_bucket: String,

    /// S3 region
    #[arg(long, env = "AKIDB_S3_REGION", default_value = "us-east-1")]
    s3_region: String,
}

#[derive(Debug, Clone, ValueEnum)]
enum FileFormat {
    Csv,
    Jsonl,
    Parquet,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    init_logging();

    let args = Cli::parse();

    info!("Starting AkiDB ingest");
    info!("Collection: {}", args.collection);
    info!("File: {}", args.file.display());
    info!("Batch size: {}", args.batch_size);
    info!("Parallel threads: {}", args.parallel);

    // Detect file format if not specified
    let format = args
        .format
        .clone()
        .unwrap_or_else(|| detect_format(&args.file));
    info!("Detected format: {:?}", format);

    // Verify file exists
    if !args.file.exists() {
        return Err(format!("File not found: {}", args.file.display()).into());
    }

    // Create progress bar
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} [{elapsed_precise}] {msg}")
            .unwrap(),
    );
    pb.set_message("Initializing...");

    // Parse and ingest based on format
    let result = match format {
        FileFormat::Csv => {
            let parser = CsvParser::new(
                args.file.clone(),
                args.id_column.clone(),
                args.vector_column.clone(),
                args.payload_columns.clone(),
            )?;
            ingest_with_parser(parser, args, pb).await
        }
        FileFormat::Jsonl => {
            let parser = JsonlParser::new(
                args.file.clone(),
                args.id_column.clone(),
                args.vector_column.clone(),
                args.payload_columns.clone(),
            )?;
            ingest_with_parser(parser, args, pb).await
        }
        FileFormat::Parquet => {
            let parser = ParquetParser::new(
                args.file.clone(),
                args.id_column.clone(),
                args.vector_column.clone(),
                args.payload_columns.clone(),
            )?;
            ingest_with_parser(parser, args, pb).await
        }
    };

    match result {
        Ok(stats) => {
            println!("\n✅ Ingest complete!");
            println!("  Vectors ingested: {}", stats.total_vectors);
            println!("  Time elapsed: {:.2}s", stats.duration_secs);
            println!("  Throughput: {:.0} vectors/sec", stats.throughput());
            println!("  Segments created: {}", stats.segments_created);
            Ok(())
        }
        Err(e) => {
            eprintln!("\n❌ Ingest failed: {}", e);
            Err(e)
        }
    }
}

/// Ingest vectors using a specific parser
async fn ingest_with_parser<P: VectorParser>(
    parser: P,
    args: Cli,
    pb: ProgressBar,
) -> Result<IngestStats, Box<dyn std::error::Error>> {
    use crate::pipeline::S3IngestConfig;

    let s3_config = S3IngestConfig {
        endpoint: args.s3_endpoint,
        access_key: args.s3_access_key,
        secret_key: args.s3_secret_key,
        bucket: args.s3_bucket,
        region: args.s3_region,
    };

    let pipeline =
        IngestPipeline::new(args.collection, args.batch_size, args.parallel, s3_config).await?;

    pipeline.ingest(parser, pb).await
}

/// Detect file format from extension
fn detect_format(path: &std::path::Path) -> FileFormat {
    match path.extension().and_then(|s| s.to_str()) {
        Some("csv") => FileFormat::Csv,
        Some("jsonl") | Some("json") => FileFormat::Jsonl,
        Some("parquet") => FileFormat::Parquet,
        _ => {
            eprintln!("Warning: Could not detect format from extension, assuming CSV");
            FileFormat::Csv
        }
    }
}

/// Initialize logging
fn init_logging() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(env_filter).with_target(false).init();
}

/// Ingest statistics
#[derive(Debug)]
pub struct IngestStats {
    pub total_vectors: usize,
    pub duration_secs: f64,
    pub segments_created: usize,
}

impl IngestStats {
    pub fn throughput(&self) -> f64 {
        self.total_vectors as f64 / self.duration_secs
    }
}
