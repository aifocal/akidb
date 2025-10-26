//! Filter parser for translating the JSON filter DSL into executable plans.
//!
//! The parser is responsible for transforming user supplied filter expressions
//! into a [`FilterTree`] abstract syntax tree (AST) and evaluating that tree
//! against the metadata index. A successful evaluation returns a `RoaringBitmap`
//! containing the set of document identifiers that satisfy the filter.
//!
//! Supported boolean operators:
//! - `must`: logical AND
//! - `should`: logical OR
//! - `must_not`: logical NOT
//!
//! Supported leaf predicates:
//! - `term` (expressed as `{ "field": "...", "match": ... }`)
//! - `range` (expressed as `{ "field": "...", "range": { "gte": ..., "lte": ... } }`)
//! - `exists` (expressed as `{ "exists": "field_name" }` or `{ "exists": { "field": "field_name" } }`)

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use roaring::RoaringBitmap;
use serde_json::Value;

use akidb_core::{Error, Result};
use akidb_storage::MetadataStore;

/// Maximum recursion depth allowed while parsing filter expressions.
const MAX_FILTER_DEPTH: usize = 32;

/// Upper bound on the number of child clauses allowed per boolean operator.
const MAX_BOOLEAN_CLAUSES: usize = 128;

/// Parser that converts JSON filter documents into `RoaringBitmap` sets.
///
/// The parser acts in three stages:
/// 1. Convert JSON into a [`FilterTree`] AST via [`FilterParser::parse_tree`]
/// 2. Evaluate the AST via [`FilterParser::evaluate_tree`] to obtain doc id sets
/// 3. Return the bitmap to the caller
pub struct FilterParser {
    metadata_store: Arc<dyn MetadataStore>,
    default_collection: Option<String>,
}

impl FilterParser {
    /// Create a new `FilterParser` without a default collection.
    pub fn new(metadata_store: Arc<dyn MetadataStore>) -> Self {
        Self {
            metadata_store,
            default_collection: None,
        }
    }

    /// Create a new `FilterParser` that always targets the provided collection.
    pub fn with_collection(
        metadata_store: Arc<dyn MetadataStore>,
        collection: impl Into<String>,
    ) -> Self {
        Self {
            metadata_store,
            default_collection: Some(collection.into()),
        }
    }

    /// Update the default collection associated with this parser.
    pub fn set_default_collection(&mut self, collection: impl Into<String>) {
        self.default_collection = Some(collection.into());
    }

    /// Parse using the configured default collection.
    ///
    /// If no default collection has been set, this method returns a validation error.
    pub async fn parse(&self, filter: &Value) -> Result<RoaringBitmap> {
        let collection = self
            .default_collection
            .as_deref()
            .ok_or_else(|| {
                Error::Validation(
                    "parse requires a collection; use `with_collection`, `set_default_collection`, or call `parse_with_collection`"
                        .to_string(),
                )
            })?;

        self.parse_with_collection(filter, collection).await
    }

    /// Parse and evaluate a filter expression in one step.
    ///
    /// This is a convenience wrapper that first builds the [`FilterTree`] AST
    /// and then evaluates it against the metadata store for the provided
    /// collection.
    pub async fn parse_with_collection(
        &self,
        filter: &Value,
        collection: &str,
    ) -> Result<RoaringBitmap> {
        let tree = self.parse_tree(filter)?;
        self.evaluate_tree(&tree, collection).await
    }

    /// Parse a JSON filter document into a [`FilterTree`] AST.
    pub fn parse_tree(&self, filter: &Value) -> Result<FilterTree> {
        self.parse_tree_recursive(filter, 0)
    }

    fn parse_tree_recursive(&self, node: &Value, depth: usize) -> Result<FilterTree> {
        if depth > MAX_FILTER_DEPTH {
            return Err(Error::Validation(format!(
                "filter nesting exceeds maximum depth of {}",
                MAX_FILTER_DEPTH
            )));
        }

        let obj = node
            .as_object()
            .ok_or_else(|| Error::Validation(format!("expected object filter, got {}", node)))?;

        // Collect boolean clauses if present.
        let mut boolean_clauses = Vec::new();

        if let Some(must_value) = obj.get("must") {
            boolean_clauses.push(FilterTree::Must(
                self.parse_boolean_array("must", must_value, depth)?,
            ));
        }

        if let Some(should_value) = obj.get("should") {
            boolean_clauses.push(FilterTree::Should(self.parse_boolean_array(
                "should",
                should_value,
                depth,
            )?));
        }

        if let Some(must_not_value) = obj.get("must_not") {
            boolean_clauses.push(FilterTree::MustNot(self.parse_boolean_array(
                "must_not",
                must_not_value,
                depth,
            )?));
        }

        if !boolean_clauses.is_empty() {
            return if boolean_clauses.len() == 1 {
                // Safe: we just checked non-empty, so there is exactly 1 element
                Ok(boolean_clauses
                    .into_iter()
                    .next()
                    .expect("boolean_clauses has exactly 1 element"))
            } else {
                Ok(FilterTree::Must(boolean_clauses))
            };
        }

        if let (Some(field_value), Some(term_value)) = (obj.get("field"), obj.get("match")) {
            return Ok(FilterTree::Term {
                field: self.parse_field_name(field_value)?,
                value: term_value.clone(),
            });
        }

        if let (Some(field_value), Some(range_value)) = (obj.get("field"), obj.get("range")) {
            return self.build_range_node(field_value, range_value);
        }

        if let Some(exists_value) = obj.get("exists") {
            return Ok(FilterTree::Exists {
                field: self.parse_exists_field(exists_value)?,
            });
        }

        Err(Error::Validation(format!(
            "unsupported filter expression: {}",
            node
        )))
    }

    fn parse_boolean_array(
        &self,
        name: &str,
        value: &Value,
        depth: usize,
    ) -> Result<Vec<FilterTree>> {
        let array = value.as_array().ok_or_else(|| {
            Error::Validation(format!("{} clause expects array, found {}", name, value))
        })?;

        if array.len() > MAX_BOOLEAN_CLAUSES {
            return Err(Error::Validation(format!(
                "{} clause exceeds maximum of {} entries",
                name, MAX_BOOLEAN_CLAUSES
            )));
        }

        array
            .iter()
            .map(|item| self.parse_tree_recursive(item, depth + 1))
            .collect()
    }

    fn build_range_node(&self, field: &Value, range_value: &Value) -> Result<FilterTree> {
        let range_obj = range_value.as_object().ok_or_else(|| {
            Error::Validation(format!(
                "range clause expects object, found {}",
                range_value
            ))
        })?;

        let gte = range_obj.get("gte").cloned();
        let lte = range_obj.get("lte").cloned();

        if gte.is_none() && lte.is_none() {
            return Err(Error::Validation(
                "range clause requires at least one of gte or lte".to_string(),
            ));
        }

        Ok(FilterTree::Range {
            field: self.parse_field_name(field)?,
            gte,
            lte,
        })
    }

    fn parse_field_name(&self, value: &Value) -> Result<String> {
        value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| Error::Validation(format!("field name must be string, found {}", value)))
    }

    fn parse_exists_field(&self, value: &Value) -> Result<String> {
        match value {
            Value::String(s) => Ok(s.clone()),
            Value::Object(map) => map
                .get("field")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| {
                    Error::Validation(
                        "exists clause requires string or object with `field` key".to_string(),
                    )
                }),
            _ => Err(Error::Validation(
                "exists clause requires string or object".to_string(),
            )),
        }
    }

    /// Evaluate a previously parsed [`FilterTree`] against the metadata store.
    pub async fn evaluate_tree(
        &self,
        tree: &FilterTree,
        collection: &str,
    ) -> Result<RoaringBitmap> {
        self.evaluate_tree_internal(tree, collection).await
    }

    fn evaluate_tree_internal<'a>(
        &'a self,
        tree: &'a FilterTree,
        collection: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<RoaringBitmap>> + Send + 'a>> {
        match tree {
            FilterTree::Must(clauses) => Box::pin(async move {
                if clauses.is_empty() {
                    return self.metadata_store.get_all_docs(collection).await;
                }

                let mut iter = clauses.iter();
                // Safe: we just checked clauses is non-empty above
                let mut bitmap = self
                    .evaluate_tree_internal(
                        iter.next().expect("clauses is non-empty, checked above"),
                        collection,
                    )
                    .await?;

                for clause in iter {
                    let clause_bitmap = self.evaluate_tree_internal(clause, collection).await?;
                    bitmap &= clause_bitmap;
                }

                Ok(bitmap)
            }),
            FilterTree::Should(clauses) => Box::pin(async move {
                let mut bitmap = RoaringBitmap::new();

                for clause in clauses {
                    let clause_bitmap = self.evaluate_tree_internal(clause, collection).await?;
                    bitmap |= clause_bitmap;
                }

                Ok(bitmap)
            }),
            FilterTree::MustNot(clauses) => Box::pin(async move {
                if clauses.is_empty() {
                    return self.metadata_store.get_all_docs(collection).await;
                }

                let mut excluded = RoaringBitmap::new();

                for clause in clauses {
                    let clause_bitmap = self.evaluate_tree_internal(clause, collection).await?;
                    excluded |= clause_bitmap;
                }

                let mut all_docs = self.metadata_store.get_all_docs(collection).await?;
                all_docs -= excluded;
                Ok(all_docs)
            }),
            FilterTree::Term { field, value } => Box::pin(async move {
                self.metadata_store
                    .find_term(collection, field, value)
                    .await
            }),
            FilterTree::Range { field, gte, lte } => Box::pin(async move {
                self.metadata_store
                    .find_range(collection, field, gte.as_ref(), lte.as_ref())
                    .await
            }),
            FilterTree::Exists { field } => {
                Box::pin(async move { self.metadata_store.find_exists(collection, field).await })
            }
        }
    }
}

/// Abstract syntax tree capturing the semantics of a parsed filter.
#[derive(Debug, Clone)]
pub enum FilterTree {
    /// Logical AND of nested clauses.
    Must(Vec<FilterTree>),
    /// Logical OR of nested clauses.
    Should(Vec<FilterTree>),
    /// Logical NOT of nested clauses.
    MustNot(Vec<FilterTree>),
    /// Exact term match: `field == value`.
    Term { field: String, value: Value },
    /// Range predicate on scalar fields (`gte` and/or `lte`).
    Range {
        field: String,
        gte: Option<Value>,
        lte: Option<Value>,
    },
    /// Field presence predicate.
    Exists { field: String },
}
