//! Metadata storage using Arrow IPC format.
//!
//! This module provides the foundational structures for representing
//! structured payload data backed by Apache Arrow.

use std::io::Cursor;
use std::sync::Arc;

use arrow::array::{ArrayRef, RecordBatch};
use arrow::datatypes::{DataType, Schema};
use arrow::json::{ReaderBuilder, reader};
use arrow::ipc::writer::StreamWriter;
use arrow::ipc::reader::StreamReader;

use crate::error::Result;
use akidb_core::Error;

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
    ///
    /// # Arguments
    /// * `payloads` - Vector of JSON values to convert to Arrow format
    ///
    /// # Returns
    /// A `MetadataBlock` containing the Arrow RecordBatch representation
    ///
    /// # Errors
    /// Returns error if JSON cannot be converted to Arrow format
    pub fn from_json(payloads: Vec<serde_json::Value>) -> Result<Self> {
        if payloads.is_empty() {
            return Err(Error::Validation(
                "Cannot create MetadataBlock from empty payload list".to_string(),
            ));
        }

        // Convert Vec<Value> to newline-delimited JSON
        let json_lines: Vec<String> = payloads
            .iter()
            .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "{}".to_string()))
            .collect();
        let json_string = json_lines.join("\n");

        // First, infer the schema from the JSON data
        let cursor_for_schema = Cursor::new(json_string.as_bytes());
        let (schema, _) = reader::infer_json_schema(cursor_for_schema, Some(payloads.len()))
            .map_err(|e| Error::Serialization(format!("Failed to infer Arrow schema: {}", e)))?;

        // Now build the reader with the inferred schema
        let cursor = Cursor::new(json_string.as_bytes());
        let mut reader = ReaderBuilder::new(Arc::new(schema.clone()))
            .with_batch_size(payloads.len())
            .build(cursor)
            .map_err(|e| Error::Serialization(format!("Arrow JSON reader error: {}", e)))?;

        // Read the first (and only) batch
        let batch = reader
            .next()
            .ok_or_else(|| Error::Serialization("No data in Arrow reader".to_string()))?
            .map_err(|e| Error::Serialization(format!("Failed to read Arrow batch: {}", e)))?;

        Ok(MetadataBlock {
            schema: Arc::new(schema),
            batch,
            compression: CompressionType::None,
        })
    }

    /// Convert metadata back to JSON payloads.
    ///
    /// # Returns
    /// A vector of JSON values, one per row in the RecordBatch
    ///
    /// # Errors
    /// Returns error if Arrow data cannot be converted to JSON
    pub fn to_json(&self) -> Result<Vec<serde_json::Value>> {
        let num_rows = self.batch.num_rows();
        let mut results = Vec::with_capacity(num_rows);

        // Iterate over each row and convert to JSON
        for row_idx in 0..num_rows {
            let mut row_obj = serde_json::Map::new();

            // Iterate over each column
            for (col_idx, field) in self.schema.fields().iter().enumerate() {
                let column = self.batch.column(col_idx);
                let value = Self::array_value_to_json(column, row_idx)?;
                row_obj.insert(field.name().clone(), value);
            }

            results.push(serde_json::Value::Object(row_obj));
        }

        Ok(results)
    }

    /// Helper function to convert an Arrow array value to JSON
    fn array_value_to_json(array: &ArrayRef, row_idx: usize) -> Result<serde_json::Value> {
        use arrow::array::*;

        if array.is_null(row_idx) {
            return Ok(serde_json::Value::Null);
        }

        match array.data_type() {
            DataType::Boolean => {
                let arr = array.as_any().downcast_ref::<BooleanArray>()
                    .ok_or_else(|| Error::Serialization("Failed to downcast Boolean array".to_string()))?;
                Ok(serde_json::Value::Bool(arr.value(row_idx)))
            }
            DataType::Int8 | DataType::Int16 | DataType::Int32 => {
                let arr = array.as_any().downcast_ref::<Int32Array>()
                    .ok_or_else(|| Error::Serialization("Failed to downcast Int32 array".to_string()))?;
                Ok(serde_json::json!(arr.value(row_idx)))
            }
            DataType::Int64 => {
                let arr = array.as_any().downcast_ref::<Int64Array>()
                    .ok_or_else(|| Error::Serialization("Failed to downcast Int64 array".to_string()))?;
                Ok(serde_json::json!(arr.value(row_idx)))
            }
            DataType::UInt8 | DataType::UInt16 | DataType::UInt32 | DataType::UInt64 => {
                let arr = array.as_any().downcast_ref::<UInt64Array>()
                    .ok_or_else(|| Error::Serialization("Failed to downcast UInteger array".to_string()))?;
                Ok(serde_json::json!(arr.value(row_idx)))
            }
            DataType::Float32 => {
                let arr = array.as_any().downcast_ref::<Float32Array>()
                    .ok_or_else(|| Error::Serialization("Failed to downcast Float32 array".to_string()))?;
                Ok(serde_json::json!(arr.value(row_idx)))
            }
            DataType::Float64 => {
                let arr = array.as_any().downcast_ref::<Float64Array>()
                    .ok_or_else(|| Error::Serialization("Failed to downcast Float64 array".to_string()))?;
                Ok(serde_json::json!(arr.value(row_idx)))
            }
            DataType::Utf8 => {
                let arr = array.as_any().downcast_ref::<StringArray>()
                    .ok_or_else(|| Error::Serialization("Failed to downcast String array".to_string()))?;
                Ok(serde_json::Value::String(arr.value(row_idx).to_string()))
            }
            DataType::LargeUtf8 => {
                let arr = array.as_any().downcast_ref::<LargeStringArray>()
                    .ok_or_else(|| Error::Serialization("Failed to downcast LargeString array".to_string()))?;
                Ok(serde_json::Value::String(arr.value(row_idx).to_string()))
            }
            _ => {
                // For unsupported types, return string representation
                Ok(serde_json::Value::String(format!("{:?}", array)))
            }
        }
    }

    /// Serialize to bytes using Arrow IPC format.
    ///
    /// # Returns
    /// Bytes containing the serialized Arrow IPC stream
    ///
    /// # Errors
    /// Returns error if serialization fails
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();

        // Write Arrow IPC stream format
        {
            let mut writer = StreamWriter::try_new(&mut buffer, &self.schema)
                .map_err(|e| Error::Serialization(format!("Failed to create Arrow IPC writer: {}", e)))?;

            writer.write(&self.batch)
                .map_err(|e| Error::Serialization(format!("Failed to write Arrow batch: {}", e)))?;

            writer.finish()
                .map_err(|e| Error::Serialization(format!("Failed to finish Arrow IPC stream: {}", e)))?;
        }

        // Apply compression if needed
        match self.compression {
            CompressionType::None => Ok(buffer),
            CompressionType::Zstd => {
                // Use zstd compression (level 3 for balance)
                let compressed = zstd::encode_all(&buffer[..], 3)
                    .map_err(|e| Error::Serialization(format!("Zstd compression failed: {}", e)))?;
                Ok(compressed)
            }
            CompressionType::Lz4 => {
                // LZ4 compression not yet implemented
                Err(Error::NotImplemented("LZ4 compression not yet supported".to_string()))
            }
        }
    }

    /// Deserialize from bytes into a `MetadataBlock`.
    ///
    /// # Arguments
    /// * `data` - Bytes containing the serialized Arrow IPC stream
    ///
    /// # Returns
    /// A `MetadataBlock` reconstructed from the serialized data
    ///
    /// # Errors
    /// Returns error if deserialization fails
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        // TODO: Auto-detect compression type from magic bytes
        // For now, assume no compression
        let decompressed_data = data;

        let cursor = Cursor::new(decompressed_data);
        let reader = StreamReader::try_new(cursor, None)
            .map_err(|e| Error::Serialization(format!("Failed to create Arrow IPC reader: {}", e)))?;

        let schema = reader.schema();

        // Read the first (and only) batch
        let batch = reader
            .into_iter()
            .next()
            .ok_or_else(|| Error::Serialization("No data in Arrow IPC stream".to_string()))?
            .map_err(|e| Error::Serialization(format!("Failed to read Arrow batch: {}", e)))?;

        Ok(MetadataBlock {
            schema,
            batch,
            compression: CompressionType::None,
        })
    }

    /// Deserialize from Zstd-compressed bytes.
    ///
    /// # Arguments
    /// * `compressed_data` - Zstd-compressed bytes
    ///
    /// # Returns
    /// A `MetadataBlock` reconstructed from the compressed data
    ///
    /// # Errors
    /// Returns error if decompression or deserialization fails
    pub fn deserialize_zstd(compressed_data: &[u8]) -> Result<Self> {
        let decompressed = zstd::decode_all(compressed_data)
            .map_err(|e| Error::Serialization(format!("Zstd decompression failed: {}", e)))?;

        Self::deserialize(&decompressed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_roundtrip() {
        // Test converting JSON → Arrow → JSON
        let payloads = vec![
            json!({"name": "Product A", "price": 99.99, "available": true}),
            json!({"name": "Product B", "price": 149.99, "available": false}),
            json!({"name": "Product C", "price": 199.99, "available": true}),
        ];

        // Convert to Arrow
        let metadata = MetadataBlock::from_json(payloads.clone()).expect("Failed to create MetadataBlock");

        // Verify batch properties
        assert_eq!(metadata.batch.num_rows(), 3);
        assert_eq!(metadata.batch.num_columns(), 3); // name, price, available

        // Convert back to JSON
        let result = metadata.to_json().expect("Failed to convert to JSON");

        // Verify roundtrip
        assert_eq!(result.len(), 3);

        // Check first row
        assert_eq!(result[0]["name"], "Product A");
        assert_eq!(result[0]["price"], 99.99);
        assert_eq!(result[0]["available"], true);

        // Check second row
        assert_eq!(result[1]["name"], "Product B");
        assert_eq!(result[1]["price"], 149.99);
        assert_eq!(result[1]["available"], false);
    }

    #[test]
    fn test_serialization_roundtrip() {
        // Test serializing and deserializing the metadata block
        let payloads = vec![
            json!({"id": 1, "name": "Item 1"}),
            json!({"id": 2, "name": "Item 2"}),
        ];

        let metadata = MetadataBlock::from_json(payloads).expect("Failed to create MetadataBlock");

        // Serialize
        let serialized = metadata.serialize().expect("Failed to serialize");
        assert!(!serialized.is_empty());

        // Deserialize
        let deserialized = MetadataBlock::deserialize(&serialized).expect("Failed to deserialize");

        // Verify
        assert_eq!(deserialized.batch.num_rows(), metadata.batch.num_rows());
        assert_eq!(deserialized.batch.num_columns(), metadata.batch.num_columns());

        // Convert to JSON to verify content
        let original_json = metadata.to_json().expect("Failed to convert to JSON");
        let deserialized_json = deserialized.to_json().expect("Failed to convert to JSON");

        assert_eq!(original_json, deserialized_json);
    }

    #[test]
    fn test_empty_payload_error() {
        // Test that empty payload list returns error
        let result = MetadataBlock::from_json(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_mixed_types() {
        // Test JSON with mixed types
        let payloads = vec![
            json!({"str_field": "text", "int_field": 42, "float_field": 3.14, "bool_field": true}),
            json!({"str_field": "more text", "int_field": 100, "float_field": 2.71, "bool_field": false}),
        ];

        let metadata = MetadataBlock::from_json(payloads).expect("Failed to create MetadataBlock");
        assert_eq!(metadata.batch.num_rows(), 2);
        assert_eq!(metadata.batch.num_columns(), 4);

        let result = metadata.to_json().expect("Failed to convert to JSON");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["str_field"], "text");
        assert_eq!(result[0]["int_field"], 42);
    }

    #[test]
    fn test_zstd_compression() {
        // Test Zstd compression
        let payloads = vec![
            json!({"data": "This is some test data that should compress well"}),
            json!({"data": "This is more test data that should compress well"}),
        ];

        let mut metadata = MetadataBlock::from_json(payloads).expect("Failed to create MetadataBlock");
        metadata.compression = CompressionType::Zstd;

        // Serialize with compression
        let compressed = metadata.serialize().expect("Failed to serialize with Zstd");
        assert!(!compressed.is_empty());

        // Deserialize
        let deserialized = MetadataBlock::deserialize_zstd(&compressed).expect("Failed to deserialize Zstd");

        // Verify content
        let original_json = metadata.to_json().expect("Failed to convert to JSON");
        let deserialized_json = deserialized.to_json().expect("Failed to convert to JSON");
        assert_eq!(original_json, deserialized_json);
    }

    #[test]
    fn test_large_batch() {
        // Test with larger dataset
        let mut payloads = Vec::new();
        for i in 0..1000 {
            payloads.push(json!({
                "id": i,
                "name": format!("Item {}", i),
                "value": i as f64 * 1.5,
            }));
        }

        let metadata = MetadataBlock::from_json(payloads).expect("Failed to create MetadataBlock");
        assert_eq!(metadata.batch.num_rows(), 1000);

        // Test serialization
        let serialized = metadata.serialize().expect("Failed to serialize");
        let deserialized = MetadataBlock::deserialize(&serialized).expect("Failed to deserialize");
        assert_eq!(deserialized.batch.num_rows(), 1000);
    }
}
