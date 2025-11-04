use super::{ParseError, VectorParser, VectorRecord};
use arrow::array::{Array, Float32Array, ListArray, StringArray};
use arrow::record_batch::RecordBatch;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;

/// Parquet parser for vector data
pub struct ParquetParser {
    file_path: PathBuf,
    id_column: String,
    vector_column: String,
    payload_columns: Vec<String>,
}

impl ParquetParser {
    pub fn new(
        file_path: PathBuf,
        id_column: String,
        vector_column: String,
        payload_columns: Vec<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            file_path,
            id_column,
            vector_column,
            payload_columns,
        })
    }

    /// Extract vectors from a ListArray column
    fn extract_vectors(&self, array: &Arc<dyn Array>) -> Result<Vec<Vec<f32>>, ParseError> {
        let list_array = array
            .as_any()
            .downcast_ref::<ListArray>()
            .ok_or_else(|| ParseError::Other("Vector column is not a list".to_string()))?;

        let mut vectors = Vec::new();

        for i in 0..list_array.len() {
            if list_array.is_null(i) {
                return Err(ParseError::Other(format!("Null vector at row {}", i)));
            }

            let value_array = list_array.value(i);
            let float_array = value_array
                .as_any()
                .downcast_ref::<Float32Array>()
                .ok_or_else(|| ParseError::Other("Vector elements are not float32".to_string()))?;

            let vector: Vec<f32> = (0..float_array.len())
                .map(|j| float_array.value(j))
                .collect();

            vectors.push(vector);
        }

        Ok(vectors)
    }
}

impl VectorParser for ParquetParser {
    fn parse(&mut self) -> Result<Vec<VectorRecord>, Box<dyn std::error::Error>> {
        let file = File::open(&self.file_path)?;

        let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
        let mut reader = builder.build()?;

        let mut all_records = Vec::new();

        while let Some(batch) = reader.next() {
            let batch: RecordBatch = batch?;
            let schema = batch.schema();

            // Find column indices
            let id_idx = schema
                .index_of(&self.id_column)
                .map_err(|_| ParseError::ColumnNotFound(self.id_column.clone()))?;

            let vector_idx = schema
                .index_of(&self.vector_column)
                .map_err(|_| ParseError::ColumnNotFound(self.vector_column.clone()))?;

            // Extract IDs
            let id_array = batch.column(id_idx);
            let id_string_array = id_array
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| ParseError::Other("ID column is not string".to_string()))?;

            // Extract vectors
            let vector_array = batch.column(vector_idx);
            let vectors = self.extract_vectors(vector_array)?;

            // Extract payload columns
            let payload_indices: Vec<(usize, String)> = if self.payload_columns.is_empty() {
                // Use all columns except ID and vector
                (0..schema.fields().len())
                    .filter(|&i| i != id_idx && i != vector_idx)
                    .map(|i| (i, schema.field(i).name().clone()))
                    .collect()
            } else {
                self.payload_columns
                    .iter()
                    .map(|col| {
                        schema
                            .index_of(col)
                            .map(|idx| (idx, col.clone()))
                            .map_err(|_| ParseError::ColumnNotFound(col.clone()))
                    })
                    .collect::<Result<Vec<_>, _>>()?
            };

            // Build records
            for row_idx in 0..batch.num_rows() {
                let id = id_string_array.value(row_idx).to_string();
                let vector = vectors[row_idx].clone();

                let mut payload = HashMap::new();
                for (col_idx, col_name) in &payload_indices {
                    let array = batch.column(*col_idx);

                    // Convert Arrow value to JSON value (simplified - only handles strings and numbers)
                    let json_value = if let Some(string_array) =
                        array.as_any().downcast_ref::<StringArray>()
                    {
                        serde_json::Value::String(string_array.value(row_idx).to_string())
                    } else if let Some(float_array) = array.as_any().downcast_ref::<Float32Array>()
                    {
                        serde_json::Value::Number(
                            serde_json::Number::from_f64(float_array.value(row_idx) as f64)
                                .unwrap(),
                        )
                    } else {
                        // Fallback: convert to string
                        serde_json::Value::String(format!("{:?}", array))
                    };

                    payload.insert(col_name.clone(), json_value);
                }

                all_records.push(VectorRecord {
                    id,
                    vector,
                    payload,
                });
            }
        }

        Ok(all_records)
    }

    fn estimated_total(&self) -> Option<usize> {
        // Could read Parquet metadata for row count
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires a test Parquet file
    fn test_parquet_parser_basic() {
        // This test would need a valid Parquet file
        // For now, just ensure the parser can be constructed
        let parser = ParquetParser::new(
            PathBuf::from("test.parquet"),
            "id".to_string(),
            "vector".to_string(),
            vec![],
        );
        assert!(parser.is_ok());
    }
}
