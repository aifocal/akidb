//! Metadata storage using Arrow IPC format.
//!
//! This module provides the foundational structures for representing
//! structured payload data backed by Apache Arrow.

use std::sync::Arc;

use arrow::datatypes::Schema;
use arrow::record_batch::RecordBatch;

use crate::error::Result;

/// Metadata block containing structured payload data.
#[derive(Debug, Clone)]
pub struct MetadataBlock {
    /// Arrow schema defining the structure.
    pub schema: Arc<Schema>,

    /// Arrow `RecordBatch` containing the data.
    pub batch: RecordBatch,

    /// Compression type for serialization.
    pub compression: CompressionType,
}

/// Compression options for metadata serialization.
#[derive(Debug, Clone, Copy)]
pub enum CompressionType {
    /// No compression.
    None,
    /// Zstandard compression.
    Zstd,
    /// LZ4 compression (faster, less compression).
    Lz4,
}

impl MetadataBlock {
    /// Create a new `MetadataBlock` from JSON payloads.
    pub fn from_json(_payloads: Vec<serde_json::Value>) -> Result<Self> {
        todo!("Convert JSON to Arrow RecordBatch")
    }

    /// Convert metadata back to JSON payloads.
    pub fn to_json(&self) -> Result<Vec<serde_json::Value>> {
        todo!("Convert Arrow RecordBatch to JSON")
    }

    /// Serialize to bytes using Arrow IPC format.
    pub fn serialize(&self) -> Result<Vec<u8>> {
        todo!("Serialize using Arrow IPC")
    }

    /// Deserialize from bytes into a `MetadataBlock`.
    pub fn deserialize(_data: &[u8]) -> Result<Self> {
        todo!("Deserialize from Arrow IPC")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "Metadata block JSON roundtrip not implemented"]
    fn test_json_roundtrip() {
        // Test converting JSON → Arrow → JSON.
        todo!()
    }

    #[test]
    #[ignore = "Metadata block serialization not implemented"]
    fn test_serialization_roundtrip() {
        // Test serializing and deserializing the metadata block.
        todo!()
    }
}
