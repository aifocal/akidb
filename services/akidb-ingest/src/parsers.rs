use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

pub mod csv_parser;
pub mod jsonl_parser;
pub mod parquet_parser;

pub use csv_parser::CsvParser;
pub use jsonl_parser::JsonlParser;
pub use parquet_parser::ParquetParser;

/// A single vector record with ID, embeddings, and payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorRecord {
    pub id: String,
    pub vector: Vec<f32>,
    pub payload: HashMap<String, serde_json::Value>,
}

/// Trait for parsers that can read vectors from different file formats
pub trait VectorParser: Send {
    /// Parse the file and return an iterator of vector records
    fn parse(&mut self) -> Result<Vec<VectorRecord>, Box<dyn std::error::Error>>;

    /// Get estimated total records (if available)
    fn estimated_total(&self) -> Option<usize> {
        None
    }
}

/// Error type for parsing errors
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Arrow error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    #[error("Parquet error: {0}")]
    Parquet(#[from] parquet::errors::ParquetError),

    #[error("Column '{0}' not found")]
    ColumnNotFound(String),

    #[error("Invalid vector dimension: expected {expected}, got {actual}")]
    InvalidDimension { expected: usize, actual: usize },

    #[error("Parse error: {0}")]
    Other(String),
}
