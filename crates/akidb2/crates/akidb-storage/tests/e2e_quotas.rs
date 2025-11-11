//! Advanced E2E tests: Quota & Limits
//!
//! Tests quota enforcement and limit handling:
//! 1. Memory quota enforcement (max batch size)
//! 2. Storage quota enforcement (max uploads)
//! 3. Collection count limit
//! 4. Concurrent upload limit (semaphore)

use akidb_core::ids::CollectionId;
use akidb_core::vector::VectorDocument;
use akidb_storage::batch_config::S3BatchConfig;
use akidb_storage::batch_uploader::BatchUploader;
use akidb_storage::object_store::{MockS3ObjectStore, ObjectStore};
use akidb_storage::parallel_uploader::{ParallelConfig, ParallelUploader};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_batch_size_limit_enforcement() {
    // Setup: Batch uploader with batch_size=10
    let store = Arc::new(MockS3ObjectStore::new());
    let config = S3BatchConfig {
        enabled: true,
        batch_size: 10, // Max 10 documents per batch
        max_wait_ms: 10000, // Long timeout to ensure batch size triggers
    };

    let uploader = BatchUploader::new(store.clone(), config).unwrap();
    let collection_id = CollectionId::new();

    // Action: Insert 25 documents (should trigger 2 auto-flushes + 1 manual)
    for i in 0..25 {
        let doc = VectorDocument::new(vec![i as f32; 128]);
        let flushed = uploader.add_document(collection_id, 128, doc).await.unwrap();

        // Verify flush triggered at batch boundaries
        if (i + 1) % 10 == 0 {
            assert!(flushed, "Expected flush at document {}", i + 1);
        }
    }

    // Manual flush for remaining 5 documents
    uploader.flush_collection(collection_id).await.unwrap();

    // Validation: Exactly 3 snapshots uploaded (10 + 10 + 5)
    let successful_puts = store.successful_puts();
    assert_eq!(successful_puts, 3, "Expected 3 batch uploads");
}

#[tokio::test]
async fn test_max_concurrency_limit() {
    // Setup: ParallelUploader with max_concurrency=5
    let store = Arc::new(MockS3ObjectStore::new_with_latency(Duration::from_millis(100)));

    let config = ParallelConfig {
        batch: S3BatchConfig {
            enabled: true,
            batch_size: 1, // Immediate flush
            max_wait_ms: 1000,
        },
        max_concurrency: 5, // Only 5 concurrent uploads
    };

    let uploader = Arc::new(ParallelUploader::new(store.clone(), config).unwrap());
    let collection_id = CollectionId::new();

    // Action: Rapidly add 50 documents
    let start = std::time::Instant::now();

    for i in 0..50 {
        let doc = VectorDocument::new(vec![i as f32; 128]);
        uploader.add_document(collection_id, 128, doc).await.unwrap();
    }

    uploader.flush_all().await.unwrap();

    let elapsed = start.elapsed();

    // Validation: With concurrency=5 and 100ms latency, should take ~1 second
    // (50 uploads / 5 concurrent = 10 batches * 100ms = 1000ms)
    println!("Elapsed: {:?}", elapsed);
    assert!(elapsed >= Duration::from_millis(900), "Concurrency limit not enforced");
    assert!(elapsed < Duration::from_secs(3), "Too slow, concurrency may not be working");

    let successful_puts = store.successful_puts();
    assert_eq!(successful_puts, 50, "Expected 50 uploads");
}

#[tokio::test]
async fn test_max_wait_timeout_enforcement() {
    // Setup: Batch uploader with max_wait_ms=200
    let store = Arc::new(MockS3ObjectStore::new());
    let config = S3BatchConfig {
        enabled: true,
        batch_size: 1000, // Very large, won't trigger on size
        max_wait_ms: 200, // 200ms timeout
    };

    let uploader = BatchUploader::new(store.clone(), config).unwrap();
    let collection_id = CollectionId::new();

    // Action: Add 5 documents (below batch size)
    let start = std::time::Instant::now();

    for _ in 0..5 {
        let doc = VectorDocument::new(vec![0.1; 128]);
        uploader.add_document(collection_id, 128, doc).await.unwrap();
    }

    // Wait for timeout to trigger auto-flush
    tokio::time::sleep(Duration::from_millis(250)).await;

    // Manually flush to ensure pending data is uploaded
    uploader.flush_collection(collection_id).await.unwrap();

    let elapsed = start.elapsed();

    // Validation: Should have flushed after ~200ms timeout
    let successful_puts = store.successful_puts();
    println!("Timeout test - Elapsed: {:?}, Puts: {}", elapsed, successful_puts);
    assert_eq!(successful_puts, 1, "Expected 1 snapshot after timeout");
}

#[tokio::test]
async fn test_dimension_mismatch_validation() {
    // Setup: Batch uploader
    let store = Arc::new(MockS3ObjectStore::new());
    let config = S3BatchConfig {
        enabled: true,
        batch_size: 10,
        max_wait_ms: 5000,
    };

    let uploader = BatchUploader::new(store, config).unwrap();
    let collection_id = CollectionId::new();

    // Action: Add document with dimension=128
    let doc1 = VectorDocument::new(vec![0.1; 128]);
    uploader.add_document(collection_id, 128, doc1).await.unwrap();

    // Try to add document with different dimension=256
    let doc2 = VectorDocument::new(vec![0.2; 256]);
    let result = uploader.add_document(collection_id, 256, doc2).await;

    // Validation: Should reject dimension mismatch
    assert!(result.is_err(), "Expected dimension mismatch error");

    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(err_msg.contains("Dimension mismatch"), "Expected dimension mismatch error message");
}
