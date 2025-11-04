use super::{ParseError, VectorParser, VectorRecord};
use csv::ReaderBuilder;
use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;

/// CSV parser for vector data
pub struct CsvParser {
    file_path: PathBuf,
    id_column: String,
    vector_column: String,
    payload_columns: Vec<String>,
}

impl CsvParser {
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
}

impl VectorParser for CsvParser {
    fn parse(&mut self) -> Result<Vec<VectorRecord>, Box<dyn std::error::Error>> {
        let file = File::open(&self.file_path)?;
        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .from_reader(file);

        // Get headers
        let headers = reader.headers()?.clone();

        // Find column indices
        let id_idx = headers
            .iter()
            .position(|h| h == self.id_column)
            .ok_or_else(|| ParseError::ColumnNotFound(self.id_column.clone()))?;

        let vector_idx = headers
            .iter()
            .position(|h| h == self.vector_column)
            .ok_or_else(|| ParseError::ColumnNotFound(self.vector_column.clone()))?;

        // Find payload column indices
        let payload_indices: Vec<(usize, String)> = if self.payload_columns.is_empty() {
            // Include all columns except ID and vector
            headers
                .iter()
                .enumerate()
                .filter(|(idx, _)| *idx != id_idx && *idx != vector_idx)
                .map(|(idx, name)| (idx, name.to_string()))
                .collect()
        } else {
            self.payload_columns
                .iter()
                .map(|col| {
                    headers
                        .iter()
                        .position(|h| h == col)
                        .map(|idx| (idx, col.clone()))
                        .ok_or_else(|| ParseError::ColumnNotFound(col.clone()))
                })
                .collect::<Result<Vec<_>, _>>()?
        };

        // Parse records
        let mut records = Vec::new();

        for result in reader.records() {
            let record = result?;

            // Extract ID
            let id = record
                .get(id_idx)
                .ok_or_else(|| ParseError::Other("Missing ID column".to_string()))?
                .to_string();

            // Extract vector
            let vector_str = record
                .get(vector_idx)
                .ok_or_else(|| ParseError::Other("Missing vector column".to_string()))?;

            // Parse vector (assume JSON array format)
            let vector: Vec<f32> = if vector_str.starts_with('[') {
                serde_json::from_str(vector_str)?
            } else {
                // Try comma-separated values
                vector_str
                    .split(',')
                    .map(|s| s.trim().parse::<f32>())
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| ParseError::Other(format!("Failed to parse vector: {}", e)))?
            };

            // Extract payload
            let mut payload = HashMap::new();
            for (idx, name) in &payload_indices {
                if let Some(value) = record.get(*idx) {
                    // Try to parse as JSON, otherwise store as string
                    let json_value = if let Ok(v) = serde_json::from_str::<serde_json::Value>(value)
                    {
                        v
                    } else {
                        serde_json::Value::String(value.to_string())
                    };
                    payload.insert(name.clone(), json_value);
                }
            }

            records.push(VectorRecord {
                id,
                vector,
                payload,
            });
        }

        Ok(records)
    }

    fn estimated_total(&self) -> Option<usize> {
        // Could count lines for a better estimate, but that requires a full read
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_csv_parser_basic() {
        // Create a temporary CSV file
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            "id,vector,name,price\n\
             product_1,\"[0.1, 0.2, 0.3]\",Laptop,999.99\n\
             product_2,\"[0.4, 0.5, 0.6]\",Mouse,29.99"
        )
        .unwrap();

        let mut parser = CsvParser::new(
            temp_file.path().to_path_buf(),
            "id".to_string(),
            "vector".to_string(),
            vec!["name".to_string(), "price".to_string()],
        )
        .unwrap();

        let records = parser.parse().unwrap();
        assert_eq!(records.len(), 2);

        assert_eq!(records[0].id, "product_1");
        assert_eq!(records[0].vector, vec![0.1, 0.2, 0.3]);
        assert_eq!(
            records[0].payload.get("name").unwrap(),
            &serde_json::Value::String("Laptop".to_string())
        );

        assert_eq!(records[1].id, "product_2");
        assert_eq!(records[1].vector, vec![0.4, 0.5, 0.6]);
    }

    #[test]
    fn test_csv_parser_all_payload_columns() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            "id,vector,col1,col2\n\
             test_1,\"[1.0, 2.0]\",value1,value2"
        )
        .unwrap();

        let mut parser = CsvParser::new(
            temp_file.path().to_path_buf(),
            "id".to_string(),
            "vector".to_string(),
            vec![], // Empty = use all columns
        )
        .unwrap();

        let records = parser.parse().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].payload.len(), 2); // col1 and col2
    }
}
