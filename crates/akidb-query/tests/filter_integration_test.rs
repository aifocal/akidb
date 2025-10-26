//! Integration tests for filter pushdown functionality
//!
//! Tests the FilterParser → RoaringBitmap conversion

use std::sync::Arc;

use akidb_query::FilterParser;
use akidb_storage::{MemoryMetadataStore, MetadataStore};
use serde_json::json;

/// Test end-to-end filter pushdown: metadata indexing → filter parsing
#[tokio::test]
async fn test_filter_pushdown_end_to_end() {
    // 1. Set up metadata store and index some documents
    let metadata_store = Arc::new(MemoryMetadataStore::new()) as Arc<dyn MetadataStore>;

    // Index sample documents
    metadata_store
        .index_metadata(
            "products",
            1,
            &json!({"category": "electronics", "price": 100}),
        )
        .await
        .unwrap();
    metadata_store
        .index_metadata(
            "products",
            2,
            &json!({"category": "electronics", "price": 200}),
        )
        .await
        .unwrap();
    metadata_store
        .index_metadata("products", 3, &json!({"category": "books", "price": 50}))
        .await
        .unwrap();
    metadata_store
        .index_metadata(
            "products",
            4,
            &json!({"category": "electronics", "price": 300}),
        )
        .await
        .unwrap();

    // 2. Create filter parser
    let filter_parser = FilterParser::new(Arc::clone(&metadata_store));

    // 3. Parse filter
    let filter_bitmap = filter_parser
        .parse_with_collection(
            &json!({
                "must": [
                    {"field": "category", "match": "electronics"},
                    {"field": "price", "range": {"gte": 150, "lte": 350}}
                ]
            }),
            "products",
        )
        .await
        .unwrap();

    // 4. Verify results
    // Should match documents [2, 4]:
    // - Doc 1: category=electronics, price=100 (✗ price < 150)
    // - Doc 2: category=electronics, price=200 (✓)
    // - Doc 3: category=books, price=50 (✗ category)
    // - Doc 4: category=electronics, price=300 (✓)
    assert_eq!(filter_bitmap.len(), 2);
    assert!(filter_bitmap.contains(2));
    assert!(filter_bitmap.contains(4));
}

/// Test filter parsing with complex boolean logic
#[tokio::test]
async fn test_complex_filter_pushdown() {
    let metadata_store = Arc::new(MemoryMetadataStore::new()) as Arc<dyn MetadataStore>;

    // Index documents
    metadata_store
        .index_metadata(
            "items",
            1,
            &json!({"brand": "sony", "category": "electronics", "price": 100}),
        )
        .await
        .unwrap();
    metadata_store
        .index_metadata(
            "items",
            2,
            &json!({"brand": "apple", "category": "electronics", "price": 200}),
        )
        .await
        .unwrap();
    metadata_store
        .index_metadata(
            "items",
            3,
            &json!({"brand": "sony", "category": "books", "price": 50}),
        )
        .await
        .unwrap();
    metadata_store
        .index_metadata(
            "items",
            4,
            &json!({"brand": "samsung", "category": "electronics", "price": 150}),
        )
        .await
        .unwrap();

    let filter_parser = FilterParser::new(Arc::clone(&metadata_store));

    // Complex filter: (brand=sony OR brand=apple) AND category=electronics
    let filter_bitmap = filter_parser
        .parse_with_collection(
            &json!({
                "must": [
                    {
                        "should": [
                            {"field": "brand", "match": "sony"},
                            {"field": "brand", "match": "apple"}
                        ]
                    },
                    {"field": "category", "match": "electronics"}
                ]
            }),
            "items",
        )
        .await
        .unwrap();

    // Should match [1, 2]:
    // - Doc 1: brand=sony, category=electronics (✓)
    // - Doc 2: brand=apple, category=electronics (✓)
    // - Doc 3: brand=sony, category=books (✗ category)
    // - Doc 4: brand=samsung, category=electronics (✗ brand)
    assert_eq!(filter_bitmap.len(), 2);
    assert!(filter_bitmap.contains(1));
    assert!(filter_bitmap.contains(2));
}

/// Test must_not filter (exclusion)
#[tokio::test]
async fn test_must_not_filter_pushdown() {
    let metadata_store = Arc::new(MemoryMetadataStore::new()) as Arc<dyn MetadataStore>;

    // Index documents
    for i in 1..=5 {
        metadata_store
            .index_metadata(
                "items",
                i,
                &json!({"status": if i % 2 == 0 { "discontinued" } else { "active" }}),
            )
            .await
            .unwrap();
    }

    let filter_parser = FilterParser::new(Arc::clone(&metadata_store));

    // Filter: exclude discontinued items
    let filter_bitmap = filter_parser
        .parse_with_collection(
            &json!({
                "must_not": [
                    {"field": "status", "match": "discontinued"}
                ]
            }),
            "items",
        )
        .await
        .unwrap();

    // Should match [1, 3, 5] (odd numbers = active)
    assert_eq!(filter_bitmap.len(), 3);
    assert!(filter_bitmap.contains(1));
    assert!(filter_bitmap.contains(3));
    assert!(filter_bitmap.contains(5));
}

/// Test invalid filter handling (graceful degradation)
#[tokio::test]
async fn test_invalid_filter_graceful_handling() {
    let metadata_store = Arc::new(MemoryMetadataStore::new()) as Arc<dyn MetadataStore>;
    let filter_parser = FilterParser::new(Arc::clone(&metadata_store));

    // Invalid filter (missing required fields)
    let result = filter_parser
        .parse_with_collection(&json!({"invalid_operator": []}), "test")
        .await;

    // Should return an error
    assert!(result.is_err());
}
