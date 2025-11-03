//! Metadata store providing term, range, and existence queries over document payloads.
//!
//! The in-memory implementation maintains a two-level map:
//! `collection → field → inverted index`. Each inverted index maps metadata values
//! to roaring bitmaps of document identifiers and tracks the document identifiers
//! that contain the field. Numeric values additionally populate a sorted map so
//! range lookups can execute using `O(log n)` bound location rather than scanning
//! every term.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ops::Bound;

use async_trait::async_trait;
use ordered_float::OrderedFloat;
use parking_lot::RwLock;
use roaring::RoaringBitmap;
use serde_json::{Map, Value};

use akidb_core::{Error, Result};

/// Interface for metadata indexing backends.
#[async_trait]
pub trait MetadataStore: Send + Sync {
    /// Insert or replace metadata for a document.
    async fn insert_metadata(&self, collection: &str, doc_id: u32, metadata: &Value) -> Result<()>;

    /// Remove metadata for a document.
    async fn remove_metadata(&self, collection: &str, doc_id: u32) -> Result<()>;

    /// Return document identifiers whose field value matches the provided term(s).
    async fn find_term(
        &self,
        collection: &str,
        field: &str,
        value: &Value,
    ) -> Result<RoaringBitmap>;

    /// Return document identifiers within the inclusive numeric range.
    async fn find_range(
        &self,
        collection: &str,
        field: &str,
        gte: Option<&Value>,
        lte: Option<&Value>,
    ) -> Result<RoaringBitmap>;

    /// Return document identifiers that contain the field.
    async fn find_exists(&self, collection: &str, field: &str) -> Result<RoaringBitmap>;

    /// Return all document identifiers indexed for the collection.
    async fn get_all_docs(&self, collection: &str) -> Result<RoaringBitmap>;

    /// Compatibility helper used by legacy call sites.
    async fn index_metadata(&self, collection: &str, doc_id: u32, metadata: &Value) -> Result<()> {
        self.insert_metadata(collection, doc_id, metadata).await
    }
}

/// In-memory metadata store backed by roaring bitmaps.
pub struct MemoryMetadataStore {
    indices: RwLock<HashMap<String, HashMap<String, InvertedIndex>>>,
}

impl MemoryMetadataStore {
    /// Construct an empty metadata store.
    pub fn new() -> Self {
        Self {
            indices: RwLock::new(HashMap::new()),
        }
    }

    fn remove_doc_from_collection(collection: &mut HashMap<String, InvertedIndex>, doc_id: u32) {
        let mut empty_fields = Vec::new();
        for (field, index) in collection.iter_mut() {
            if index.remove(doc_id) {
                empty_fields.push(field.clone());
            }
        }

        for field in empty_fields {
            collection.remove(&field);
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
    async fn insert_metadata(&self, collection: &str, doc_id: u32, metadata: &Value) -> Result<()> {
        let object = metadata_object(metadata)?;
        let mut indices = self.indices.write();
        let remove_collection = {
            let collection_indices = indices.entry(collection.to_string()).or_default();
            Self::remove_doc_from_collection(collection_indices, doc_id);

            if object.is_empty() {
                collection_indices.is_empty()
            } else {
                for (field, value) in object {
                    let index = collection_indices
                        .entry(field.clone())
                        .or_insert_with(InvertedIndex::new);
                    index.insert(doc_id, value)?;
                }
                collection_indices.values().all(InvertedIndex::is_empty)
            }
        };

        if remove_collection {
            indices.remove(collection);
        }

        Ok(())
    }

    async fn remove_metadata(&self, collection: &str, doc_id: u32) -> Result<()> {
        let mut indices = self.indices.write();
        let mut remove_collection = false;

        if let Some(collection_indices) = indices.get_mut(collection) {
            Self::remove_doc_from_collection(collection_indices, doc_id);
            remove_collection = collection_indices.is_empty();
        }

        if remove_collection {
            indices.remove(collection);
        }

        Ok(())
    }

    async fn find_term(
        &self,
        collection: &str,
        field: &str,
        value: &Value,
    ) -> Result<RoaringBitmap> {
        let search_keys = value_keys_for_query(value)?;

        let indices = self.indices.read();
        if let Some(collection_indices) = indices.get(collection) {
            if let Some(index) = collection_indices.get(field) {
                return Ok(index.lookup_terms(&search_keys));
            }
        }

        Ok(RoaringBitmap::new())
    }

    async fn find_range(
        &self,
        collection: &str,
        field: &str,
        gte: Option<&Value>,
        lte: Option<&Value>,
    ) -> Result<RoaringBitmap> {
        let lower = match gte {
            Some(value) => Some(number_from_value(value)?),
            None => None,
        };
        let upper = match lte {
            Some(value) => Some(number_from_value(value)?),
            None => None,
        };

        if let (Some(lo), Some(hi)) = (lower, upper) {
            if lo > hi {
                return Ok(RoaringBitmap::new());
            }
        }

        let indices = self.indices.read();
        if let Some(collection_indices) = indices.get(collection) {
            if let Some(index) = collection_indices.get(field) {
                return Ok(index.find_range(lower, upper));
            }
        }

        Ok(RoaringBitmap::new())
    }

    async fn find_exists(&self, collection: &str, field: &str) -> Result<RoaringBitmap> {
        let indices = self.indices.read();
        if let Some(collection_indices) = indices.get(collection) {
            if let Some(index) = collection_indices.get(field) {
                return Ok(index.exists.clone());
            }
        }

        Ok(RoaringBitmap::new())
    }

    async fn get_all_docs(&self, collection: &str) -> Result<RoaringBitmap> {
        let indices = self.indices.read();
        if let Some(collection_indices) = indices.get(collection) {
            let mut union = RoaringBitmap::new();
            for index in collection_indices.values() {
                union |= &index.exists;
            }
            return Ok(union);
        }

        Ok(RoaringBitmap::new())
    }
}

/// Per-field inverted index storing value → doc ids.
struct InvertedIndex {
    terms: HashMap<ValueKey, RoaringBitmap>,
    numeric_terms: BTreeMap<OrderedFloat<f64>, RoaringBitmap>,
    exists: RoaringBitmap,
    doc_terms: HashMap<u32, Vec<ValueKey>>,
}

impl InvertedIndex {
    fn new() -> Self {
        Self {
            terms: HashMap::new(),
            numeric_terms: BTreeMap::new(),
            exists: RoaringBitmap::new(),
            doc_terms: HashMap::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.exists.is_empty()
    }

    fn insert(&mut self, doc_id: u32, value: &Value) -> Result<()> {
        let raw_keys = value_keys_for_index(value)?;
        if raw_keys.is_empty() {
            self.remove(doc_id);
            return Ok(());
        }

        let unique_keys: Vec<ValueKey> = {
            let mut unique = BTreeSet::new();
            for key in raw_keys {
                unique.insert(key);
            }
            unique.into_iter().collect()
        };

        self.remove(doc_id);
        self.exists.insert(doc_id);

        for key in &unique_keys {
            let bitmap = self.terms.entry(key.clone()).or_default();
            bitmap.insert(doc_id);

            if let ValueKey::Number(num) = key {
                let numeric = self.numeric_terms.entry(*num).or_default();
                numeric.insert(doc_id);
            }
        }

        self.doc_terms.insert(doc_id, unique_keys);
        Ok(())
    }

    fn remove(&mut self, doc_id: u32) -> bool {
        if let Some(keys) = self.doc_terms.remove(&doc_id) {
            for key in keys {
                if let Some(bitmap) = self.terms.get_mut(&key) {
                    bitmap.remove(doc_id);
                    if bitmap.is_empty() {
                        self.terms.remove(&key);
                    }
                }

                if let ValueKey::Number(num) = key {
                    if let Some(bitmap) = self.numeric_terms.get_mut(&num) {
                        bitmap.remove(doc_id);
                        if bitmap.is_empty() {
                            self.numeric_terms.remove(&num);
                        }
                    }
                }
            }
        }

        self.exists.remove(doc_id);
        self.exists.is_empty()
    }

    fn lookup_terms(&self, keys: &[ValueKey]) -> RoaringBitmap {
        let mut bitmap = RoaringBitmap::new();
        for key in keys {
            if let Some(ids) = self.terms.get(key) {
                bitmap |= ids;
            }
        }
        bitmap
    }

    fn find_range(
        &self,
        lower: Option<OrderedFloat<f64>>,
        upper: Option<OrderedFloat<f64>>,
    ) -> RoaringBitmap {
        if lower.is_none() && upper.is_none() {
            return self.exists.clone();
        }

        let lower_bound = lower.as_ref().map_or(Bound::Unbounded, Bound::Included);
        let upper_bound = upper.as_ref().map_or(Bound::Unbounded, Bound::Included);

        let mut bitmap = RoaringBitmap::new();
        for (_key, ids) in self.numeric_terms.range((lower_bound, upper_bound)) {
            bitmap |= ids;
        }
        bitmap
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum ValueKey {
    Null,
    Bool(bool),
    Number(OrderedFloat<f64>),
    String(String),
}

fn metadata_object(value: &Value) -> Result<&Map<String, Value>> {
    value.as_object().ok_or_else(|| {
        Error::Validation(format!(
            "metadata must be a JSON object, received {}",
            value
        ))
    })
}

fn value_keys_for_index(value: &Value) -> Result<Vec<ValueKey>> {
    let mut keys = Vec::new();
    collect_keys(value, &mut keys)?;
    Ok(keys)
}

fn value_keys_for_query(value: &Value) -> Result<Vec<ValueKey>> {
    let keys = match value {
        Value::Array(values) => {
            let mut result = Vec::with_capacity(values.len());
            for element in values {
                collect_keys(element, &mut result)?;
            }
            result
        }
        _ => {
            let mut result = Vec::new();
            collect_keys(value, &mut result)?;
            result
        }
    };

    if keys.is_empty() {
        return Err(Error::Validation(
            "term query requires at least one comparable value".to_string(),
        ));
    }

    Ok(keys)
}

fn collect_keys(value: &Value, output: &mut Vec<ValueKey>) -> Result<()> {
    match value {
        Value::Null => output.push(ValueKey::Null),
        Value::Bool(boolean) => output.push(ValueKey::Bool(*boolean)),
        Value::Number(number) => {
            output.push(ValueKey::Number(number_from_number(number)?));
        }
        Value::String(text) => output.push(ValueKey::String(text.clone())),
        Value::Array(values) => {
            for entry in values {
                collect_keys(entry, output)?;
            }
        }
        Value::Object(_) => {
            return Err(Error::Validation(
                "nested objects are not supported for metadata indexing".to_string(),
            ))
        }
    }
    Ok(())
}

fn number_from_value(value: &Value) -> Result<OrderedFloat<f64>> {
    let number = value.as_f64().ok_or_else(|| {
        Error::Validation(format!("range bound must be numeric, received {}", value))
    })?;

    if !number.is_finite() {
        return Err(Error::Validation("range bound must be finite".to_string()));
    }

    Ok(OrderedFloat(number))
}

fn number_from_number(number: &serde_json::Number) -> Result<OrderedFloat<f64>> {
    if let Some(int) = number.as_i64() {
        return Ok(OrderedFloat(int as f64));
    }
    if let Some(uint) = number.as_u64() {
        return Ok(OrderedFloat(uint as f64));
    }
    if let Some(float) = number.as_f64() {
        if !float.is_finite() {
            return Err(Error::Validation(
                "numeric metadata values must be finite".to_string(),
            ));
        }
        return Ok(OrderedFloat(float));
    }

    Err(Error::Validation(format!(
        "unsupported numeric representation: {}",
        number
    )))
}
