use super::{ParseError, VectorParser, VectorRecord};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

/// JSONL (JSON Lines) parser for vector data
pub struct JsonlParser {
    file_path: PathBuf,
    id_column: String,
    vector_column: String,
    payload_columns: Vec<String>,
}

impl JsonlParser {
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

impl VectorParser for JsonlParser {
    fn parse(&mut self) -> Result<Vec<VectorRecord>, Box<dyn std::error::Error>> {
        let file = File::open(&self.file_path)?;
        let reader = BufReader::new(file);

        let mut records = Vec::new();

        for (line_num, line) in reader.lines().enumerate() {
            let line = line?;

            // Skip empty lines
            if line.trim().is_empty() {
                continue;
            }

            // Parse JSON object
            let obj: serde_json::Value = serde_json::from_str(&line).map_err(|e| {
                ParseError::Other(format!("Line {}: Invalid JSON: {}", line_num + 1, e))
            })?;

            let obj = obj.as_object().ok_or_else(|| {
                ParseError::Other(format!("Line {}: Expected JSON object", line_num + 1))
            })?;

            // Extract ID
            let id = obj
                .get(&self.id_column)
                .and_then(|v| v.as_str())
                .ok_or_else(|| ParseError::ColumnNotFound(self.id_column.clone()))?
                .to_string();

            // Extract vector
            let vector_value = obj
                .get(&self.vector_column)
                .ok_or_else(|| ParseError::ColumnNotFound(self.vector_column.clone()))?;

            let vector: Vec<f32> = if let Some(arr) = vector_value.as_array() {
                arr.iter()
                    .map(|v| {
                        v.as_f64().map(|f| f as f32).ok_or_else(|| {
                            ParseError::Other(format!(
                                "Line {}: Vector element is not a number",
                                line_num + 1
                            ))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                return Err(ParseError::Other(format!(
                    "Line {}: Vector is not an array",
                    line_num + 1
                ))
                .into());
            };

            // Extract payload
            let mut payload = HashMap::new();
            if self.payload_columns.is_empty() {
                // Include all fields except ID and vector
                for (key, value) in obj {
                    if key != &self.id_column && key != &self.vector_column {
                        payload.insert(key.clone(), value.clone());
                    }
                }
            } else {
                // Include only specified columns
                for col in &self.payload_columns {
                    if let Some(value) = obj.get(col) {
                        payload.insert(col.clone(), value.clone());
                    }
                }
            }

            records.push(VectorRecord { id, vector, payload });
        }

        Ok(records)
    }

    fn estimated_total(&self) -> Option<usize> {
        // Could count lines, but that requires a full read
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_jsonl_parser_basic() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            r#"{{"id":"product_1","vector":[0.1,0.2,0.3],"name":"Laptop","price":999.99}}"#
        )
        .unwrap();
        writeln!(
            temp_file,
            r#"{{"id":"product_2","vector":[0.4,0.5,0.6],"name":"Mouse","price":29.99}}"#
        )
        .unwrap();

        let mut parser = JsonlParser::new(
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
            records[0].payload.get("name").unwrap().as_str().unwrap(),
            "Laptop"
        );

        assert_eq!(records[1].id, "product_2");
        assert_eq!(records[1].vector, vec![0.4, 0.5, 0.6]);
    }

    #[test]
    fn test_jsonl_parser_all_payload_columns() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            r#"{{"id":"test_1","vector":[1.0,2.0],"col1":"value1","col2":"value2"}}"#
        )
        .unwrap();

        let mut parser = JsonlParser::new(
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

    #[test]
    fn test_jsonl_parser_empty_lines() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            r#"{{"id":"test_1","vector":[1.0],"data":"test"}}"#
        )
        .unwrap();
        writeln!(temp_file).unwrap(); // Empty line
        writeln!(
            temp_file,
            r#"{{"id":"test_2","vector":[2.0],"data":"test2"}}"#
        )
        .unwrap();

        let mut parser = JsonlParser::new(
            temp_file.path().to_path_buf(),
            "id".to_string(),
            "vector".to_string(),
            vec![],
        )
        .unwrap();

        let records = parser.parse().unwrap();
        assert_eq!(records.len(), 2); // Empty line ignored
    }
}
