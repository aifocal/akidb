//! Parquet encoder for batching vector documents into columnar format.
//!
//! Reduces S3 API calls by 90%+ by batching 100+ documents per upload.
//! Typical compression: 2-3x for vector data.

use akidb_core::error::{CoreError, CoreResult};
use akidb_core::vector::VectorDocument;
use arrow::array::{
    ArrayRef, BinaryArray, FixedSizeListArray, Float32Array, RecordBatch, StringArray,
    TimestampMillisecondArray, UInt32Array,
};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use bytes::Bytes;
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;
use std::io::Cursor;
use std::sync::Arc;

/// Parquet encoding configuration
#[derive(Debug, Clone)]
pub struct ParquetConfig {
    /// Compression algorithm (Snappy recommended for speed)
    pub compression: Compression,
    /// Row group size (default: 10,000)
    pub row_group_size: usize,
    /// Enable dictionary encoding (recommended for repeated values)
    pub enable_dictionary: bool,
}

impl Default for ParquetConfig {
    fn default() -> Self {
        Self {
            compression: Compression::SNAPPY,
            row_group_size: 10_000,
            enable_dictionary: true,
        }
    }
}

/// Parquet encoder for vector documents
pub struct ParquetEncoder {
    config: ParquetConfig,
}

impl ParquetEncoder {
    /// Create new encoder with config
    pub fn new(config: ParquetConfig) -> Self {
        Self { config }
    }

    /// Create encoder with default config
    pub fn default() -> Self {
        Self::new(ParquetConfig::default())
    }

    /// Define Arrow schema for vector documents
    fn schema(dimension: u32) -> Arc<Schema> {
        let fields = vec![
            Field::new("document_id", DataType::Binary, false),
            Field::new("external_id", DataType::Utf8, true),
            Field::new("dimension", DataType::UInt32, false),
            Field::new(
                "vector",
                DataType::FixedSizeList(
                    Arc::new(Field::new("item", DataType::Float32, false)),
                    dimension as i32,
                ),
                false,
            ),
            Field::new("metadata_json", DataType::Utf8, true),
            Field::new(
                "inserted_at",
                DataType::Timestamp(TimeUnit::Millisecond, None),
                false,
            ),
        ];

        Arc::new(Schema::new(fields))
    }

    /// Encode vector documents to Parquet bytes
    ///
    /// # Arguments
    /// * `documents` - Batch of documents to encode
    /// * `dimension` - Vector dimension (must match all documents)
    ///
    /// # Returns
    /// Parquet-encoded bytes ready for S3 upload
    pub fn encode_batch(&self, documents: &[VectorDocument], dimension: u32) -> CoreResult<Bytes> {
        if documents.is_empty() {
            return Err(CoreError::ValidationError(
                "Cannot encode empty batch".to_string(),
            ));
        }

        // Validate all documents have correct dimension
        for doc in documents {
            if doc.vector.len() != dimension as usize {
                return Err(CoreError::ValidationError(format!(
                    "Document {} has dimension {} but expected {}",
                    doc.doc_id,
                    doc.vector.len(),
                    dimension
                )));
            }
        }

        // Build Arrow arrays
        // Store document IDs as owned data, then create slice references
        let document_id_bytes: Vec<[u8; 16]> =
            documents.iter().map(|d| d.doc_id.to_bytes()).collect();
        let document_id_refs: Vec<&[u8]> = document_id_bytes.iter().map(|b| b.as_slice()).collect();

        let external_ids: Vec<Option<&str>> =
            documents.iter().map(|d| d.external_id.as_deref()).collect();

        let dimensions: Vec<u32> = vec![dimension; documents.len()];

        // Flatten vectors for FixedSizeList
        let vectors: Vec<f32> = documents
            .iter()
            .flat_map(|d| d.vector.iter().copied())
            .collect();

        let metadata_jsons: Vec<Option<String>> = documents
            .iter()
            .map(|d| d.metadata.as_ref().map(|m| m.to_string()))
            .collect();

        let inserted_ats: Vec<i64> = documents
            .iter()
            .map(|d| d.inserted_at.timestamp_millis())
            .collect();

        // Create Arrow arrays
        let document_id_array: ArrayRef = Arc::new(BinaryArray::from(document_id_refs));
        let external_id_array: ArrayRef = Arc::new(StringArray::from(external_ids));
        let dimension_array: ArrayRef = Arc::new(UInt32Array::from(dimensions));

        // Create FixedSizeListArray for vectors
        let values_array = Arc::new(Float32Array::from(vectors));
        let vector_field = Arc::new(Field::new("item", DataType::Float32, false));
        let vector_array: ArrayRef = Arc::new(
            FixedSizeListArray::try_new(
                vector_field,
                dimension as i32,
                values_array,
                None, // No nulls
            )
            .map_err(|e| CoreError::SerializationError(e.to_string()))?,
        );

        let metadata_array: ArrayRef = Arc::new(StringArray::from(metadata_jsons));
        let inserted_at_array: ArrayRef = Arc::new(TimestampMillisecondArray::from(inserted_ats));

        let schema = Self::schema(dimension);

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                document_id_array,
                external_id_array,
                dimension_array,
                vector_array,
                metadata_array,
                inserted_at_array,
            ],
        )
        .map_err(|e| CoreError::SerializationError(e.to_string()))?;

        // Configure Parquet writer
        let props = WriterProperties::builder()
            .set_compression(self.config.compression)
            .set_max_row_group_size(self.config.row_group_size)
            .set_dictionary_enabled(self.config.enable_dictionary)
            .build();

        // Write to in-memory buffer
        let mut buffer = Cursor::new(Vec::new());
        let mut writer = ArrowWriter::try_new(&mut buffer, schema.clone(), Some(props))
            .map_err(|e| CoreError::SerializationError(e.to_string()))?;

        writer
            .write(&batch)
            .map_err(|e| CoreError::SerializationError(e.to_string()))?;

        writer
            .close()
            .map_err(|e| CoreError::SerializationError(e.to_string()))?;

        Ok(Bytes::from(buffer.into_inner()))
    }

    /// Decode Parquet bytes back to vector documents
    pub fn decode_batch(&self, data: &[u8]) -> CoreResult<Vec<VectorDocument>> {
        use arrow::array::Array;
        use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

        // Create Parquet reader
        let reader = ParquetRecordBatchReaderBuilder::try_new(Bytes::from(data.to_vec()))
            .map_err(|e| {
                CoreError::DeserializationError(format!("Failed to create Parquet reader: {}", e))
            })?
            .build()
            .map_err(|e| {
                CoreError::DeserializationError(format!("Failed to build Parquet reader: {}", e))
            })?;

        let mut documents = Vec::new();

        for batch in reader {
            let batch = batch.map_err(|e| {
                CoreError::DeserializationError(format!("Failed to read batch: {}", e))
            })?;

            // Extract columns
            let document_ids = batch
                .column(0)
                .as_any()
                .downcast_ref::<BinaryArray>()
                .ok_or_else(|| {
                    CoreError::DeserializationError("Invalid document_id column".to_string())
                })?;

            let external_ids = batch
                .column(1)
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    CoreError::DeserializationError("Invalid external_id column".to_string())
                })?;

            let dimensions = batch
                .column(2)
                .as_any()
                .downcast_ref::<UInt32Array>()
                .ok_or_else(|| {
                    CoreError::DeserializationError("Invalid dimension column".to_string())
                })?;

            let vectors = batch
                .column(3)
                .as_any()
                .downcast_ref::<FixedSizeListArray>()
                .ok_or_else(|| {
                    CoreError::DeserializationError("Invalid vector column".to_string())
                })?;

            let metadata_jsons = batch
                .column(4)
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    CoreError::DeserializationError("Invalid metadata column".to_string())
                })?;

            let inserted_ats = batch
                .column(5)
                .as_any()
                .downcast_ref::<TimestampMillisecondArray>()
                .ok_or_else(|| {
                    CoreError::DeserializationError("Invalid inserted_at column".to_string())
                })?;

            // Build VectorDocuments
            for i in 0..batch.num_rows() {
                // Document ID
                let doc_id_bytes = document_ids.value(i);
                if doc_id_bytes.len() != 16 {
                    return Err(CoreError::DeserializationError(format!(
                        "Invalid document ID length: expected 16, got {}",
                        doc_id_bytes.len()
                    )));
                }
                let mut doc_id_array = [0u8; 16];
                doc_id_array.copy_from_slice(doc_id_bytes);
                let doc_id = akidb_core::DocumentId::from_bytes(&doc_id_array).map_err(|e| {
                    CoreError::DeserializationError(format!("Invalid document ID: {}", e))
                })?;

                // External ID (optional)
                let external_id = if external_ids.is_null(i) {
                    None
                } else {
                    Some(external_ids.value(i).to_string())
                };

                // Dimension
                let dimension = dimensions.value(i);

                // Vector
                let vector_list = vectors.value(i);
                let vector_values = vector_list
                    .as_any()
                    .downcast_ref::<Float32Array>()
                    .ok_or_else(|| {
                        CoreError::DeserializationError("Invalid vector values".to_string())
                    })?;

                let vector: Vec<f32> = (0..dimension as usize)
                    .map(|j| vector_values.value(j))
                    .collect();

                // Metadata (optional)
                let metadata = if metadata_jsons.is_null(i) {
                    None
                } else {
                    let json_str = metadata_jsons.value(i);
                    Some(serde_json::from_str(json_str).map_err(|e| {
                        CoreError::DeserializationError(format!(
                            "Failed to parse metadata JSON: {}",
                            e
                        ))
                    })?)
                };

                // Inserted at
                let inserted_at_millis = inserted_ats.value(i);
                let inserted_at = chrono::DateTime::from_timestamp_millis(inserted_at_millis)
                    .ok_or_else(|| {
                        CoreError::DeserializationError("Invalid timestamp".to_string())
                    })?;

                documents.push(VectorDocument {
                    doc_id,
                    external_id,
                    vector,
                    metadata,
                    inserted_at,
                });
            }
        }

        Ok(documents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use akidb_core::ids::DocumentId;
    use chrono::Utc;

    #[test]
    fn test_parquet_encode_single_batch() {
        let encoder = ParquetEncoder::default();

        let docs = vec![
            VectorDocument {
                doc_id: DocumentId::new(),
                external_id: Some("doc1".to_string()),
                vector: vec![1.0, 2.0, 3.0],
                metadata: None,
                inserted_at: Utc::now(),
            },
            VectorDocument {
                doc_id: DocumentId::new(),
                external_id: Some("doc2".to_string()),
                vector: vec![4.0, 5.0, 6.0],
                metadata: Some(serde_json::json!({"tag": "test"})),
                inserted_at: Utc::now(),
            },
        ];

        let bytes = encoder.encode_batch(&docs, 3).unwrap();

        // Parquet header magic bytes: "PAR1"
        assert_eq!(&bytes[0..4], b"PAR1");

        // Verify we got valid Parquet output
        // For small batches, Parquet overhead may be larger than raw data,
        // so we just check that encoding succeeded
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_parquet_encode_large_batch() {
        let encoder = ParquetEncoder::default();

        let docs: Vec<VectorDocument> = (0..1000)
            .map(|i| VectorDocument {
                doc_id: DocumentId::new(),
                external_id: Some(format!("doc{}", i)),
                vector: vec![i as f32; 512], // 512-dim
                metadata: None,
                inserted_at: Utc::now(),
            })
            .collect();

        let bytes = encoder.encode_batch(&docs, 512).unwrap();

        // Should be valid Parquet
        assert_eq!(&bytes[0..4], b"PAR1");

        // Compression ratio should be decent
        let raw_size = docs.len() * (16 + 20 + 4 + 512 * 4);
        let compression_ratio = raw_size as f64 / bytes.len() as f64;

        println!(
            "Raw: {} KB, Compressed: {} KB, Ratio: {:.2}x",
            raw_size / 1024,
            bytes.len() / 1024,
            compression_ratio
        );

        // Should get at least 1.5x compression
        assert!(compression_ratio >= 1.5);
    }

    #[test]
    fn test_parquet_dimension_mismatch() {
        let encoder = ParquetEncoder::default();

        let docs = vec![
            VectorDocument {
                doc_id: DocumentId::new(),
                external_id: None,
                vector: vec![1.0, 2.0, 3.0],
                metadata: None,
                inserted_at: Utc::now(),
            },
            VectorDocument {
                doc_id: DocumentId::new(),
                external_id: None,
                vector: vec![1.0, 2.0], // Wrong dimension!
                metadata: None,
                inserted_at: Utc::now(),
            },
        ];

        let result = encoder.encode_batch(&docs, 3);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("expected 3"));
    }

    #[test]
    fn test_parquet_empty_batch() {
        let encoder = ParquetEncoder::default();

        let result = encoder.encode_batch(&[], 3);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty batch"));
    }

    #[test]
    fn test_parquet_roundtrip() {
        let encoder = ParquetEncoder::default();

        let docs = vec![
            VectorDocument {
                doc_id: DocumentId::new(),
                external_id: Some("doc1".to_string()),
                vector: vec![1.0, 2.0, 3.0],
                metadata: Some(serde_json::json!({"tag": "test"})),
                inserted_at: Utc::now(),
            },
            VectorDocument {
                doc_id: DocumentId::new(),
                external_id: None,
                vector: vec![4.0, 5.0, 6.0],
                metadata: None,
                inserted_at: Utc::now(),
            },
        ];

        // Encode
        let bytes = encoder.encode_batch(&docs, 3).unwrap();

        // Decode
        let decoded = encoder.decode_batch(&bytes).unwrap();

        // Verify roundtrip
        assert_eq!(decoded.len(), docs.len());
        for (original, decoded) in docs.iter().zip(decoded.iter()) {
            assert_eq!(original.doc_id, decoded.doc_id);
            assert_eq!(original.external_id, decoded.external_id);
            assert_eq!(original.vector, decoded.vector);
            assert_eq!(original.metadata, decoded.metadata);
            // Note: Timestamps may lose sub-millisecond precision
            assert_eq!(
                original.inserted_at.timestamp_millis(),
                decoded.inserted_at.timestamp_millis()
            );
        }
    }

    #[test]
    fn test_parquet_roundtrip_large() {
        let encoder = ParquetEncoder::default();

        let docs: Vec<VectorDocument> = (0..100)
            .map(|i| VectorDocument {
                doc_id: DocumentId::new(),
                external_id: Some(format!("doc{}", i)),
                vector: vec![i as f32; 128],
                metadata: Some(serde_json::json!({"index": i})),
                inserted_at: Utc::now(),
            })
            .collect();

        // Encode
        let bytes = encoder.encode_batch(&docs, 128).unwrap();

        // Decode
        let decoded = encoder.decode_batch(&bytes).unwrap();

        // Verify
        assert_eq!(decoded.len(), 100);
        for (original, decoded) in docs.iter().zip(decoded.iter()) {
            assert_eq!(original.doc_id, decoded.doc_id);
            assert_eq!(original.vector, decoded.vector);
        }
    }
}
