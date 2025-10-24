//! Metadata store for fast filtering based on payload fields
//!
//! This module provides an inverted index for metadata fields,
//! enabling efficient filter query execution.

use async_trait::async_trait;
use parking_lot::RwLock;
use roaring::RoaringBitmap;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;

use akidb_core::Result;

/// Trait for metadata querying and indexing
#[async_trait]
pub trait MetadataStore: Send + Sync {
    /// Find doc IDs matching a term (exact match)
    async fn find_term(&self, collection: &str, field: &str, value: &Value) -> Result<RoaringBitmap>;

    /// Find doc IDs in range (supports gte, lte, gt, lt)
    async fn find_range(
        &self,
        collection: &str,
        field: &str,
        gte: Option<&Value>,
        lte: Option<&Value>,
        gt: Option<&Value>,
        lt: Option<&Value>,
    ) -> Result<RoaringBitmap>;

    /// Find doc IDs where field exists
    async fn find_exists(&self, collection: &str, field: &str) -> Result<RoaringBitmap>;

    /// Get all indexed doc IDs in collection
    ///
    /// Returns the union of all documents that have at least one indexed field.
    ///
    /// # Important Semantic Note
    ///
    /// This method returns documents that have been indexed with **at least one field**.
    /// Documents indexed with empty metadata objects `{}` will **not** be included,
    /// as they have no indexed fields.
    ///
    /// This is the correct behavior for filter operations where we only consider
    /// documents that have searchable indexed fields. For empty `must` filters,
    /// this returns all documents that can be searched.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Document with fields - will be included
    /// store.index_metadata("col", 1, &json!({"name": "Alice"})).await?;
    ///
    /// // Document with empty object - will NOT be included
    /// store.index_metadata("col", 2, &json!({})).await?;
    ///
    /// let all_docs = store.get_all_docs("col").await?;
    /// assert!(all_docs.contains(1));   // ✓ Included
    /// assert!(!all_docs.contains(2));  // ✗ Not included (no indexed fields)
    /// ```
    async fn get_all_docs(&self, collection: &str) -> Result<RoaringBitmap>;

    /// Index metadata for a document
    async fn index_metadata(&self, collection: &str, doc_id: u32, metadata: &Value) -> Result<()>;
}

/// In-memory metadata store using inverted indices
pub struct MemoryMetadataStore {
    /// collection → field → inverted index
    indices: Arc<RwLock<HashMap<String, HashMap<String, InvertedIndex>>>>,
}

impl MemoryMetadataStore {
    /// Create a new empty metadata store
    pub fn new() -> Self {
        Self {
            indices: Arc::new(RwLock::new(HashMap::new())),
        }
    }

}

impl Default for MemoryMetadataStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MetadataStore for MemoryMetadataStore {
    async fn find_term(&self, collection: &str, field: &str, value: &Value) -> Result<RoaringBitmap> {
        debug!(
            "Finding term in collection={}, field={}, value={}",
            collection, field, value
        );

        let indices = self.indices.read();
        if let Some(collection_indices) = indices.get(collection) {
            if let Some(index) = collection_indices.get(field) {
                return Ok(index.find_term(value));
            }
        }

        // Field not indexed yet
        Ok(RoaringBitmap::new())
    }

    async fn find_range(
        &self,
        collection: &str,
        field: &str,
        gte: Option<&Value>,
        lte: Option<&Value>,
        gt: Option<&Value>,
        lt: Option<&Value>,
    ) -> Result<RoaringBitmap> {
        debug!(
            "Finding range in collection={}, field={}, gte={:?}, lte={:?}, gt={:?}, lt={:?}",
            collection, field, gte, lte, gt, lt
        );

        let indices = self.indices.read();
        if let Some(collection_indices) = indices.get(collection) {
            if let Some(index) = collection_indices.get(field) {
                return Ok(index.find_range(gte, lte, gt, lt));
            }
        }

        // Field not indexed yet
        Ok(RoaringBitmap::new())
    }

    async fn find_exists(&self, collection: &str, field: &str) -> Result<RoaringBitmap> {
        debug!("Finding exists in collection={}, field={}", collection, field);

        let indices = self.indices.read();
        if let Some(collection_indices) = indices.get(collection) {
            if let Some(index) = collection_indices.get(field) {
                return Ok(index.exists.clone());
            }
        }

        // Field not indexed yet
        Ok(RoaringBitmap::new())
    }

    async fn get_all_docs(&self, collection: &str) -> Result<RoaringBitmap> {
        debug!("Getting all docs in collection={}", collection);

        let indices = self.indices.read();
        if let Some(collection_indices) = indices.get(collection) {
            // Union of all doc IDs across all fields
            let mut all_docs = RoaringBitmap::new();
            for index in collection_indices.values() {
                all_docs |= &index.exists;
            }
            return Ok(all_docs);
        }

        // Collection not indexed yet
        Ok(RoaringBitmap::new())
    }

    async fn index_metadata(&self, collection: &str, doc_id: u32, metadata: &Value) -> Result<()> {
        debug!(
            "Indexing metadata for collection={}, doc_id={}",
            collection, doc_id
        );

        // Validate that metadata is a JSON object
        let obj = metadata.as_object().ok_or_else(|| {
            akidb_core::Error::Validation(format!(
                "Metadata must be a JSON object, got: {}",
                match metadata {
                    Value::Null => "null",
                    Value::Bool(_) => "boolean",
                    Value::Number(_) => "number",
                    Value::String(_) => "string",
                    Value::Array(_) => "array",
                    _ => "unknown",
                }
            ))
        })?;

        // Index each field in the metadata object
        // Acquire write lock once for all fields to ensure atomicity
        let mut indices = self.indices.write();
        let collection_indices = indices.entry(collection.to_string()).or_default();

        for (field, value) in obj {
            // Get or create index directly within the lock
            let index = collection_indices
                .entry(field.clone())
                .or_insert_with(InvertedIndex::new);

            // Add document to index (no clone of entire index needed)
            index.add(doc_id, value.clone());
        }

        Ok(())
    }
}

/// Inverted index for a single field
#[derive(Debug, Clone)]
struct InvertedIndex {
    /// term → doc_ids bitmap
    terms: HashMap<ValueKey, RoaringBitmap>,
    /// all doc_ids with this field
    exists: RoaringBitmap,
    /// for range queries: sorted list of (value, doc_id)
    sorted_values: Vec<(f64, u32)>,
    /// Track current value for each doc_id (for updates)
    doc_values: HashMap<u32, ValueKey>,
}

impl InvertedIndex {
    fn new() -> Self {
        Self {
            terms: HashMap::new(),
            exists: RoaringBitmap::new(),
            sorted_values: Vec::new(),
            doc_values: HashMap::new(),
        }
    }

    /// Add a value for a document
    fn add(&mut self, doc_id: u32, value: Value) {
        // If doc_id already exists, remove it from old term index
        if let Some(old_key) = self.doc_values.get(&doc_id).cloned() {
            if let Some(bitmap) = self.terms.get_mut(&old_key) {
                bitmap.remove(doc_id);
                // Clean up empty bitmaps
                if bitmap.is_empty() {
                    self.terms.remove(&old_key);
                }
            }
        }

        // INVARIANT FIX: Always remove old sorted_values entries before adding new ones
        // This ensures that when a value changes from numeric to non-numeric (or vice versa),
        // we don't leave stale entries in sorted_values
        self.sorted_values.retain(|(_, did)| *did != doc_id);

        // Add to exists bitmap
        self.exists.insert(doc_id);

        // Add to terms index
        let key = ValueKey::from_value(&value);
        self.terms
            .entry(key.clone())
            .or_insert_with(RoaringBitmap::new)
            .insert(doc_id);

        // Track current value for this doc_id
        self.doc_values.insert(doc_id, key);

        // Add to sorted values for range queries (only for numbers)
        if let Some(num) = value.as_f64() {
            // Filter out NaN values as they cannot be compared
            if num.is_finite() {
                // Insert in sorted position to maintain order
                match self.sorted_values.binary_search_by(|probe| {
                    probe.0.partial_cmp(&num).unwrap_or(std::cmp::Ordering::Equal)
                }) {
                    Ok(pos) | Err(pos) => self.sorted_values.insert(pos, (num, doc_id)),
                }
            }
        }
    }

    /// Find docs matching exact term
    fn find_term(&self, value: &Value) -> RoaringBitmap {
        let key = ValueKey::from_value(value);
        self.terms.get(&key).cloned().unwrap_or_default()
    }

    /// Find docs in range
    fn find_range(
        &self,
        gte: Option<&Value>,
        lte: Option<&Value>,
        gt: Option<&Value>,
        lt: Option<&Value>,
    ) -> RoaringBitmap {
        // sorted_values is already sorted, no need to sort again
        let mut result = RoaringBitmap::new();

        // Extract bounds with validation
        let lower_bound = if let Some(gt_val) = gt {
            gt_val.as_f64().filter(|n| n.is_finite())
        } else if let Some(gte_val) = gte {
            gte_val.as_f64().filter(|n| n.is_finite())
        } else {
            None
        };

        let upper_bound = if let Some(lt_val) = lt {
            lt_val.as_f64().filter(|n| n.is_finite())
        } else if let Some(lte_val) = lte {
            lte_val.as_f64().filter(|n| n.is_finite())
        } else {
            None
        };

        // Validate: if bound parameter exists but couldn't be converted to number, return empty
        // This ensures consistent behavior for non-numeric range queries
        if (gt.is_some() || gte.is_some()) && lower_bound.is_none() {
            return RoaringBitmap::new();
        }
        if (lt.is_some() || lte.is_some()) && upper_bound.is_none() {
            return RoaringBitmap::new();
        }

        let gt_exclusive = gt.is_some();
        let lt_exclusive = lt.is_some();

        // Use binary search to find start position
        let start_pos = if let Some(lower) = lower_bound {
            self.sorted_values
                .binary_search_by(|probe| {
                    probe.0.partial_cmp(&lower).unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap_or_else(|pos| pos)
        } else {
            0
        };

        // For gt (exclusive), skip all values equal to lower bound
        let actual_start = if gt_exclusive && start_pos < self.sorted_values.len() {
            if let Some(lower) = lower_bound {
                // Find first value > lower
                let mut pos = start_pos;
                while pos < self.sorted_values.len() {
                    let (val, _) = self.sorted_values[pos];
                    if val > lower {
                        break;
                    }
                    pos += 1;
                }
                pos
            } else {
                start_pos
            }
        } else {
            start_pos
        };

        // Scan from actual start position (no need to check lower bound again)
        for i in actual_start..self.sorted_values.len() {
            let (num, doc_id) = self.sorted_values[i];

            // Only check upper bound (lower bound already satisfied by binary search)
            if let Some(upper) = upper_bound {
                if lt_exclusive {
                    if num >= upper {
                        break; // sorted, so all remaining values are >= upper
                    }
                } else if num > upper {
                    break;
                }
            }

            result.insert(doc_id);
        }

        result
    }
}

/// Hashable key for JSON values (for term matching)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ValueKey {
    String(String),
    Integer(i64),
    Float(u64),  // Store f64 as u64 bit representation to preserve precision
    Bool(bool),
    Null,
}

impl ValueKey {
    fn from_value(value: &Value) -> Self {
        match value {
            Value::String(s) => ValueKey::String(s.clone()),
            Value::Number(n) => {
                // Try i64 first for exact integer match
                if let Some(i) = n.as_i64() {
                    ValueKey::Integer(i)
                } else if let Some(u) = n.as_u64() {
                    // For u64 values that don't fit in i64, check if they can be
                    // exactly represented as f64 (up to 2^53)
                    const MAX_SAFE_INTEGER: u64 = 1u64 << 53;
                    if u <= MAX_SAFE_INTEGER {
                        // Safe to convert to i64 range or use as integer
                        if u <= i64::MAX as u64 {
                            ValueKey::Integer(u as i64)
                        } else {
                            // u64 is between i64::MAX and 2^53, use float representation
                            ValueKey::Float((u as f64).to_bits())
                        }
                    } else {
                        // Too large for exact f64 representation, use string
                        ValueKey::String(u.to_string())
                    }
                } else if let Some(f) = n.as_f64() {
                    // Store f64 as bit representation to preserve full precision
                    // This allows 3.14 and 3.99 to be different keys
                    ValueKey::Float(f.to_bits())
                } else {
                    // Fallback for numbers that can't be represented
                    ValueKey::Null
                }
            }
            Value::Bool(b) => ValueKey::Bool(*b),
            Value::Null => ValueKey::Null,
            // Serialize arrays and objects to JSON strings for term matching
            // This allows exact matching of complex types
            Value::Array(_) | Value::Object(_) => {
                match serde_json::to_string(value) {
                    Ok(json_str) => ValueKey::String(json_str),
                    Err(_) => ValueKey::Null,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_index_and_find_term() {
        let store = MemoryMetadataStore::new();

        // Index some documents
        store
            .index_metadata("test", 1, &json!({"category": "electronics"}))
            .await
            .unwrap();
        store
            .index_metadata("test", 2, &json!({"category": "electronics"}))
            .await
            .unwrap();
        store
            .index_metadata("test", 3, &json!({"category": "books"}))
            .await
            .unwrap();

        // Find by term
        let result = store
            .find_term("test", "category", &json!("electronics"))
            .await
            .unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains(1));
        assert!(result.contains(2));

        let result = store
            .find_term("test", "category", &json!("books"))
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(3));
    }

    #[tokio::test]
    async fn test_index_and_find_range() {
        let store = MemoryMetadataStore::new();

        // Index documents with numeric field
        store
            .index_metadata("test", 1, &json!({"price": 100}))
            .await
            .unwrap();
        store
            .index_metadata("test", 2, &json!({"price": 200}))
            .await
            .unwrap();
        store
            .index_metadata("test", 3, &json!({"price": 300}))
            .await
            .unwrap();
        store
            .index_metadata("test", 4, &json!({"price": 400}))
            .await
            .unwrap();

        // Find in range [150, 350]
        let result = store
            .find_range("test", "price", Some(&json!(150)), Some(&json!(350)), None, None)
            .await
            .unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains(2)); // 200
        assert!(result.contains(3)); // 300
    }

    #[tokio::test]
    async fn test_find_exists() {
        let store = MemoryMetadataStore::new();

        // Index documents
        store
            .index_metadata("test", 1, &json!({"brand": "sony"}))
            .await
            .unwrap();
        store
            .index_metadata("test", 2, &json!({"brand": "apple"}))
            .await
            .unwrap();
        store
            .index_metadata("test", 3, &json!({"category": "electronics"}))
            .await
            .unwrap();

        // Find where brand exists
        let result = store.find_exists("test", "brand").await.unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains(1));
        assert!(result.contains(2));

        // Find where category exists
        let result = store.find_exists("test", "category").await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(3));
    }

    #[tokio::test]
    async fn test_get_all_docs() {
        let store = MemoryMetadataStore::new();

        // Index documents
        store
            .index_metadata("test", 1, &json!({"field1": "value1"}))
            .await
            .unwrap();
        store
            .index_metadata("test", 2, &json!({"field2": "value2"}))
            .await
            .unwrap();
        store
            .index_metadata("test", 3, &json!({"field3": "value3"}))
            .await
            .unwrap();

        // Get all docs
        let result = store.get_all_docs("test").await.unwrap();
        assert_eq!(result.len(), 3);
        assert!(result.contains(1));
        assert!(result.contains(2));
        assert!(result.contains(3));
    }

    #[tokio::test]
    async fn test_reindex_same_document() {
        let store = MemoryMetadataStore::new();

        // First index
        store
            .index_metadata("test", 1, &json!({"price": 100}))
            .await
            .unwrap();

        // Reindex same doc with same value (should not create duplicates)
        store
            .index_metadata("test", 1, &json!({"price": 100}))
            .await
            .unwrap();

        // Should only have one result
        let result = store.find_term("test", "price", &json!(100)).await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(1));
    }

    #[tokio::test]
    async fn test_update_document_value() {
        let store = MemoryMetadataStore::new();

        // Initial index
        store
            .index_metadata("test", 1, &json!({"price": 100}))
            .await
            .unwrap();

        // Update to different value
        store
            .index_metadata("test", 1, &json!({"price": 200}))
            .await
            .unwrap();

        // Should only find in new value
        let result = store.find_term("test", "price", &json!(100)).await.unwrap();
        assert_eq!(result.len(), 0);

        let result = store.find_term("test", "price", &json!(200)).await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(1));
    }

    #[tokio::test]
    async fn test_large_integer_term_match() {
        let store = MemoryMetadataStore::new();

        // Test with large u64 values that exceed f64 precision
        let large1 = u64::MAX;
        let large2 = u64::MAX - 1;

        store
            .index_metadata("test", 1, &json!({"id": large1}))
            .await
            .unwrap();
        store
            .index_metadata("test", 2, &json!({"id": large2}))
            .await
            .unwrap();

        // Should distinguish between large integers
        let result = store.find_term("test", "id", &json!(large1)).await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(1));

        let result = store.find_term("test", "id", &json!(large2)).await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(2));
    }

    #[tokio::test]
    async fn test_range_gt_vs_gte() {
        let store = MemoryMetadataStore::new();

        store
            .index_metadata("test", 1, &json!({"score": 100}))
            .await
            .unwrap();
        store
            .index_metadata("test", 2, &json!({"score": 200}))
            .await
            .unwrap();
        store
            .index_metadata("test", 3, &json!({"score": 300}))
            .await
            .unwrap();

        // gt=100 should exclude score=100
        let result = store
            .find_range("test", "score", None, None, Some(&json!(100)), None)
            .await
            .unwrap();
        assert!(!result.contains(1));
        assert!(result.contains(2));
        assert!(result.contains(3));

        // gte=100 should include score=100
        let result = store
            .find_range("test", "score", Some(&json!(100)), None, None, None)
            .await
            .unwrap();
        assert!(result.contains(1));
        assert!(result.contains(2));
        assert!(result.contains(3));
    }

    #[tokio::test]
    async fn test_range_lt_vs_lte() {
        let store = MemoryMetadataStore::new();

        store
            .index_metadata("test", 1, &json!({"score": 100}))
            .await
            .unwrap();
        store
            .index_metadata("test", 2, &json!({"score": 200}))
            .await
            .unwrap();
        store
            .index_metadata("test", 3, &json!({"score": 300}))
            .await
            .unwrap();

        // lt=300 should exclude score=300
        let result = store
            .find_range("test", "score", None, None, None, Some(&json!(300)))
            .await
            .unwrap();
        assert!(result.contains(1));
        assert!(result.contains(2));
        assert!(!result.contains(3));

        // lte=300 should include score=300
        let result = store
            .find_range("test", "score", None, Some(&json!(300)), None, None)
            .await
            .unwrap();
        assert!(result.contains(1));
        assert!(result.contains(2));
        assert!(result.contains(3));
    }

    #[tokio::test]
    async fn test_range_with_non_numeric_bounds() {
        let store = MemoryMetadataStore::new();

        store
            .index_metadata("test", 1, &json!({"score": 100}))
            .await
            .unwrap();
        store
            .index_metadata("test", 2, &json!({"score": 200}))
            .await
            .unwrap();

        // Non-numeric gt should return empty
        let result = store
            .find_range("test", "score", None, None, Some(&json!("invalid")), None)
            .await
            .unwrap();
        assert_eq!(result.len(), 0);

        // Non-numeric gte should return empty
        let result = store
            .find_range("test", "score", Some(&json!("invalid")), None, None, None)
            .await
            .unwrap();
        assert_eq!(result.len(), 0);

        // Non-numeric lt should return empty
        let result = store
            .find_range("test", "score", None, None, None, Some(&json!("invalid")))
            .await
            .unwrap();
        assert_eq!(result.len(), 0);

        // Non-numeric lte should return empty
        let result = store
            .find_range("test", "score", None, Some(&json!("invalid")), None, None)
            .await
            .unwrap();
        assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn test_negative_range_query() {
        let store = MemoryMetadataStore::new();

        store
            .index_metadata("test", 1, &json!({"temp": -10}))
            .await
            .unwrap();
        store
            .index_metadata("test", 2, &json!({"temp": 0}))
            .await
            .unwrap();
        store
            .index_metadata("test", 3, &json!({"temp": 10}))
            .await
            .unwrap();

        // Range [-5, 5] should only include doc 2 (temp=0)
        let result = store
            .find_range("test", "temp", Some(&json!(-5)), Some(&json!(5)), None, None)
            .await
            .unwrap();
        assert!(result.contains(2));
        assert!(!result.contains(1));
        assert!(!result.contains(3));

        // Range [-15, -5] should only include doc 1 (temp=-10)
        let result = store
            .find_range("test", "temp", Some(&json!(-15)), Some(&json!(-5)), None, None)
            .await
            .unwrap();
        assert!(result.contains(1));
        assert!(!result.contains(2));
        assert!(!result.contains(3));
    }

    #[tokio::test]
    async fn test_array_term_matching() {
        let store = MemoryMetadataStore::new();

        // Index documents with different arrays
        store
            .index_metadata("test", 1, &json!({"tags": ["a", "b", "c"]}))
            .await
            .unwrap();
        store
            .index_metadata("test", 2, &json!({"tags": ["x", "y", "z"]}))
            .await
            .unwrap();
        store
            .index_metadata("test", 3, &json!({"tags": ["a", "b", "c"]}))
            .await
            .unwrap();

        // Should match exactly
        let result = store
            .find_term("test", "tags", &json!(["a", "b", "c"]))
            .await
            .unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains(1));
        assert!(result.contains(3));

        let result = store
            .find_term("test", "tags", &json!(["x", "y", "z"]))
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(2));

        // Different array should not match
        let result = store
            .find_term("test", "tags", &json!(["a", "b"]))
            .await
            .unwrap();
        assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn test_object_term_matching() {
        let store = MemoryMetadataStore::new();

        // Index documents with different objects
        store
            .index_metadata("test", 1, &json!({"config": {"x": 1, "y": 2}}))
            .await
            .unwrap();
        store
            .index_metadata("test", 2, &json!({"config": {"x": 3, "y": 4}}))
            .await
            .unwrap();

        // Should match exactly (note: JSON object key order matters)
        let result = store
            .find_term("test", "config", &json!({"x": 1, "y": 2}))
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(1));
    }

    #[tokio::test]
    async fn test_empty_string_term() {
        let store = MemoryMetadataStore::new();

        store
            .index_metadata("test", 1, &json!({"name": ""}))
            .await
            .unwrap();
        store
            .index_metadata("test", 2, &json!({"name": "a"}))
            .await
            .unwrap();

        // Empty string should match exactly
        let result = store.find_term("test", "name", &json!("")).await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(1));

        // Non-empty string should not match empty
        let result = store.find_term("test", "name", &json!("a")).await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(2));
    }

    #[tokio::test]
    async fn test_index_non_object_metadata_returns_error() {
        let store = MemoryMetadataStore::new();

        // Integer metadata should return error
        let result = store.index_metadata("test", 1, &json!(123)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must be a JSON object"));

        // String metadata should return error
        let result = store.index_metadata("test", 2, &json!("string")).await;
        assert!(result.is_err());

        // Null metadata should return error
        let result = store.index_metadata("test", 3, &json!(null)).await;
        assert!(result.is_err());

        // Array metadata should return error
        let result = store.index_metadata("test", 4, &json!([1, 2, 3])).await;
        assert!(result.is_err());

        // Boolean metadata should return error
        let result = store.index_metadata("test", 5, &json!(true)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_index_empty_object_metadata() {
        let store = MemoryMetadataStore::new();

        // Empty object should succeed
        let result = store.index_metadata("test", 1, &json!({})).await;
        assert!(result.is_ok());

        // But get_all_docs should not include this document (no indexed fields)
        let all_docs = store.get_all_docs("test").await.unwrap();
        assert!(!all_docs.contains(1));
    }

    #[tokio::test]
    async fn test_get_all_docs_semantic() {
        let store = MemoryMetadataStore::new();

        // Document with fields
        store
            .index_metadata("test", 1, &json!({"name": "Alice"}))
            .await
            .unwrap();

        // Document with multiple fields
        store
            .index_metadata("test", 2, &json!({"name": "Bob", "age": 30}))
            .await
            .unwrap();

        // Document with empty object (no fields)
        store.index_metadata("test", 3, &json!({})).await.unwrap();

        // get_all_docs should include documents with fields, not empty ones
        let all_docs = store.get_all_docs("test").await.unwrap();
        assert_eq!(all_docs.len(), 2);
        assert!(all_docs.contains(1));
        assert!(all_docs.contains(2));
        assert!(!all_docs.contains(3)); // Empty metadata not included
    }
}
