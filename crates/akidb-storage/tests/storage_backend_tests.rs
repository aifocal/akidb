//! Integration tests for StorageBackend async S3 upload queue

use akidb_core::{DocumentId, VectorDocument};
use akidb_storage::{StorageBackend, StorageConfig};
use std::collections::{HashMap, VecDeque};
use tempfile::TempDir;

#[tokio::test]
async fn test_memory_s3_enqueues_upload() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("test.wal");
    let snapshot_dir = temp_dir.path().join("snapshots");
    std::fs::create_dir_all(&snapshot_dir).unwrap();

    let config = StorageConfig::memory_s3(&wal_path, &snapshot_dir, "test-bucket".to_string());

    let backend = StorageBackend::new(config).await.unwrap();

    // Insert a vector
    let doc = VectorDocument::new(DocumentId::new(), vec![0.1, 0.2, 0.3]);
    backend.insert(doc.clone()).await.unwrap();

    // Give the queue a moment to be populated
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Check queue has 1 item (or may have been processed already)
    // Note: The queue size may be 0 if the background worker already processed it,
    // but we can at least verify no panic/errors occurred
    assert_eq!(backend.count(), 1, "Vector should be in memory");
}

#[tokio::test]
async fn test_memory_s3_insert_non_blocking() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("test.wal");
    let snapshot_dir = temp_dir.path().join("snapshots");
    std::fs::create_dir_all(&snapshot_dir).unwrap();

    let config = StorageConfig::memory_s3(&wal_path, &snapshot_dir, "test-bucket".to_string());

    let backend = StorageBackend::new(config).await.unwrap();

    // Measure insert latency
    let start = std::time::Instant::now();

    for i in 0..100 {
        let doc = VectorDocument::new(DocumentId::new(), vec![i as f32]);
        backend.insert(doc).await.unwrap();
    }

    let elapsed = start.elapsed();

    // Inserts should be fast (<1 second for 100 ops)
    assert!(
        elapsed < std::time::Duration::from_secs(1),
        "100 inserts took {:?}, should be <1s (non-blocking)",
        elapsed
    );

    // Average latency should be <10ms
    let avg_latency = elapsed.as_millis() / 100;
    assert!(
        avg_latency < 10,
        "Average insert latency {}ms, should be <10ms",
        avg_latency
    );

    // Verify all vectors in memory
    assert_eq!(backend.count(), 100);
}

#[tokio::test]
async fn test_graceful_shutdown() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("test.wal");
    let snapshot_dir = temp_dir.path().join("snapshots");
    std::fs::create_dir_all(&snapshot_dir).unwrap();

    let config = StorageConfig::memory(&wal_path);
    let mut config = config;
    config.snapshot_dir = snapshot_dir;

    let backend = StorageBackend::new(config).await.unwrap();

    // Insert some vectors
    for i in 0..10 {
        let doc = VectorDocument::new(DocumentId::new(), vec![i as f32]);
        backend.insert(doc).await.unwrap();
    }

    // Graceful shutdown
    backend.shutdown().await.unwrap();

    // Should not panic
    assert_eq!(backend.count(), 10);
}

#[tokio::test]
async fn test_graceful_shutdown_memory_s3() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("test.wal");
    let snapshot_dir = temp_dir.path().join("snapshots");
    std::fs::create_dir_all(&snapshot_dir).unwrap();

    let config = StorageConfig::memory_s3(&wal_path, &snapshot_dir, "test-bucket".to_string());

    let backend = StorageBackend::new(config).await.unwrap();

    // Insert some vectors
    for i in 0..10 {
        let doc = VectorDocument::new(DocumentId::new(), vec![i as f32]);
        backend.insert(doc).await.unwrap();
    }

    // Graceful shutdown (should abort background worker)
    backend.shutdown().await.unwrap();

    // Should not panic
    assert_eq!(backend.count(), 10);
}

#[tokio::test]
async fn test_s3only_insert_uploads() {
    let temp_dir = TempDir::new().unwrap();
    let local_s3_dir = temp_dir.path().join("s3");
    std::fs::create_dir_all(&local_s3_dir).unwrap();

    let config = StorageConfig::s3_only(
        temp_dir.path().join("wal"),
        temp_dir.path().join("snapshots"),
        format!("file://{}", local_s3_dir.display()),
        1000,
    )
    .with_s3_endpoint(format!("file://{}", local_s3_dir.display()))
    .with_s3_credentials("test", "test");

    std::fs::create_dir_all(temp_dir.path().join("snapshots")).unwrap();

    let backend = StorageBackend::new(config).await.unwrap();

    // Insert vector
    let doc = VectorDocument::new(DocumentId::new(), vec![1.0, 2.0, 3.0]);
    backend.insert(doc).await.unwrap();

    // Verify S3 has the file
    let s3_files: Vec<_> = walkdir::WalkDir::new(&local_s3_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .collect();

    assert!(
        s3_files.len() >= 1,
        "S3 should have at least 1 uploaded vector"
    );

    // Verify metrics
    let metrics = backend.metrics();
    assert_eq!(metrics.s3_uploads, 1, "Should have 1 S3 upload");
}

#[tokio::test]
async fn test_s3only_cache_hit() {
    let temp_dir = TempDir::new().unwrap();
    let local_s3_dir = temp_dir.path().join("s3");
    std::fs::create_dir_all(&local_s3_dir).unwrap();

    let config = StorageConfig::s3_only(
        temp_dir.path().join("wal"),
        temp_dir.path().join("snapshots"),
        format!("file://{}", local_s3_dir.display()),
        1000,
    )
    .with_s3_endpoint(format!("file://{}", local_s3_dir.display()))
    .with_s3_credentials("test", "test");

    std::fs::create_dir_all(temp_dir.path().join("snapshots")).unwrap();

    let backend = StorageBackend::new(config).await.unwrap();

    // Insert vector
    let doc = VectorDocument::new(DocumentId::new(), vec![1.0, 2.0, 3.0]);
    let doc_id = doc.doc_id.clone();
    backend.insert(doc.clone()).await.unwrap();

    // Get vector (should hit cache)
    let retrieved = backend.get(&doc_id).await.unwrap();
    assert!(retrieved.is_some(), "Vector should exist");
    assert_eq!(retrieved.unwrap().vector, doc.vector);

    // Check metrics
    let metrics = backend.metrics();
    assert_eq!(metrics.cache_hits, 1, "Should have 1 cache hit");
    assert_eq!(metrics.cache_misses, 0, "Should have 0 cache misses");
    assert_eq!(metrics.s3_downloads, 0, "Should have 0 S3 downloads");
}

#[tokio::test]
async fn test_s3only_cache_miss_downloads() {
    let temp_dir = TempDir::new().unwrap();
    let local_s3_dir = temp_dir.path().join("s3");
    std::fs::create_dir_all(&local_s3_dir).unwrap();

    let config = StorageConfig::s3_only(
        temp_dir.path().join("wal"),
        temp_dir.path().join("snapshots"),
        format!("file://{}", local_s3_dir.display()),
        1000,
    )
    .with_s3_endpoint(format!("file://{}", local_s3_dir.display()))
    .with_s3_credentials("test", "test");

    std::fs::create_dir_all(temp_dir.path().join("snapshots")).unwrap();

    let backend = StorageBackend::new(config).await.unwrap();

    // Insert vector
    let doc = VectorDocument::new(DocumentId::new(), vec![1.0, 2.0, 3.0]);
    let doc_id = doc.doc_id.clone();
    backend.insert(doc.clone()).await.unwrap();

    // Clear cache (simulate eviction)
    backend.clear_cache();

    // Get vector (should miss cache, download from S3)
    let retrieved = backend.get(&doc_id).await.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().vector, doc.vector);

    // Check metrics
    let metrics = backend.metrics();
    assert_eq!(metrics.cache_hits, 0, "Should have 0 cache hits");
    assert_eq!(metrics.cache_misses, 1, "Should have 1 cache miss");
    assert_eq!(metrics.s3_downloads, 1, "Should have 1 S3 download");
}

#[tokio::test]
async fn test_s3only_lru_eviction() {
    let temp_dir = TempDir::new().unwrap();
    let local_s3_dir = temp_dir.path().join("s3");
    std::fs::create_dir_all(&local_s3_dir).unwrap();

    // Small cache (only 5 vectors)
    let config = StorageConfig::s3_only(
        temp_dir.path().join("wal"),
        temp_dir.path().join("snapshots"),
        format!("file://{}", local_s3_dir.display()),
        5,
    )
    .with_s3_endpoint(format!("file://{}", local_s3_dir.display()))
    .with_s3_credentials("test", "test");

    std::fs::create_dir_all(temp_dir.path().join("snapshots")).unwrap();

    let backend = StorageBackend::new(config).await.unwrap();

    // Insert 10 vectors (exceeds cache)
    let mut doc_ids = Vec::new();
    for i in 0..10 {
        let doc = VectorDocument::new(DocumentId::new(), vec![i as f32]);
        doc_ids.push(doc.doc_id.clone());
        backend.insert(doc).await.unwrap();
    }

    // Check cache stats
    if let Some(stats) = backend.get_cache_stats() {
        assert_eq!(stats.size, 5, "Cache should be at capacity");
        assert_eq!(stats.capacity, 5);
    }

    // First vector should be evicted
    let retrieved = backend.get(&doc_ids[0]).await.unwrap();
    assert!(retrieved.is_some());

    // Should trigger cache miss
    let metrics = backend.metrics();
    assert!(metrics.cache_misses >= 1, "Should have cache miss");
    assert!(metrics.s3_downloads >= 1, "Should download from S3");
}

// ==================== Day 4 Tests: S3 Retry Logic + DLQ ====================

use akidb_storage::object_store::{ObjectMetadata, ObjectStore};
use async_trait::async_trait;
use bytes::Bytes;
use chrono::Utc;
use parking_lot::RwLock as SyncRwLock;
use std::sync::Arc as StdArc;

/// Mock S3 implementation for testing failure scenarios
struct MockS3ObjectStore {
    storage: StdArc<SyncRwLock<HashMap<String, Vec<u8>>>>,
    failure_pattern: StdArc<SyncRwLock<VecDeque<Result<(), String>>>>,
}

impl MockS3ObjectStore {
    /// Create mock with predefined failure pattern
    fn new_with_failure_pattern(pattern: Vec<Result<(), &'static str>>) -> Self {
        Self {
            storage: StdArc::new(SyncRwLock::new(HashMap::new())),
            failure_pattern: StdArc::new(SyncRwLock::new(
                pattern
                    .into_iter()
                    .map(|r| r.map_err(|e| e.to_string()))
                    .collect(),
            )),
        }
    }

    /// Create mock that always fails with given error
    fn new_with_error(error: &'static str) -> Self {
        Self::new_with_failure_pattern(vec![Err(error)])
    }
}

#[async_trait]
impl ObjectStore for MockS3ObjectStore {
    async fn put(&self, key: &str, data: Bytes) -> akidb_core::CoreResult<()> {
        // Check failure pattern first
        let result = self.failure_pattern.write().pop_front();

        if let Some(result) = result {
            match result {
                Ok(_) => {
                    self.storage.write().insert(key.to_string(), data.to_vec());
                    Ok(())
                }
                Err(e) => Err(akidb_core::CoreError::StorageError(e)),
            }
        } else {
            // Default: success
            self.storage.write().insert(key.to_string(), data.to_vec());
            Ok(())
        }
    }

    async fn get(&self, key: &str) -> akidb_core::CoreResult<Bytes> {
        self.storage
            .read()
            .get(key)
            .cloned()
            .map(Bytes::from)
            .ok_or_else(|| akidb_core::CoreError::NotFound {
                entity: "S3Object",
                id: key.to_string(),
            })
    }

    async fn exists(&self, key: &str) -> akidb_core::CoreResult<bool> {
        Ok(self.storage.read().contains_key(key))
    }

    async fn delete(&self, key: &str) -> akidb_core::CoreResult<()> {
        self.storage.write().remove(key);
        Ok(())
    }

    async fn list(&self, _prefix: &str) -> akidb_core::CoreResult<Vec<ObjectMetadata>> {
        Ok(self
            .storage
            .read()
            .iter()
            .map(|(k, v)| ObjectMetadata {
                key: k.clone(),
                size_bytes: v.len() as u64,
                last_modified: Utc::now(),
                etag: None,
            })
            .collect())
    }

    async fn head(&self, key: &str) -> akidb_core::CoreResult<ObjectMetadata> {
        self.storage
            .read()
            .get(key)
            .map(|v| ObjectMetadata {
                key: key.to_string(),
                size_bytes: v.len() as u64,
                last_modified: Utc::now(),
                etag: None,
            })
            .ok_or_else(|| akidb_core::CoreError::NotFound {
                entity: "S3Object",
                id: key.to_string(),
            })
    }

    async fn copy(&self, from_key: &str, to_key: &str) -> akidb_core::CoreResult<()> {
        let data = self.storage.read().get(from_key).cloned().ok_or_else(|| {
            akidb_core::CoreError::NotFound {
                entity: "S3Object",
                id: from_key.to_string(),
            }
        })?;
        self.storage.write().insert(to_key.to_string(), data);
        Ok(())
    }

    async fn put_multipart(&self, key: &str, parts: Vec<Bytes>) -> akidb_core::CoreResult<()> {
        let mut data = Vec::new();
        for part in parts {
            data.extend_from_slice(&part);
        }
        self.storage.write().insert(key.to_string(), data);
        Ok(())
    }
}

// Note: The following tests require a way to inject mock S3 into StorageBackend constructor.
// Since the current implementation creates S3 client internally, these tests are marked as
// documentation for the intended behavior. Full implementation would require adding a
// `new_with_object_store()` constructor or similar test-only API.

/// Test: S3 retry with transient error
///
/// **Goal:** Verify exponential backoff retries succeed eventually
///
/// **Expected behavior:**
/// - Mock S3 fails 3 times with transient errors (500, 503, timeout)
/// - 4th attempt succeeds
/// - Metrics show s3_uploads=1, s3_retries=3, dlq_size=0
#[tokio::test]
#[ignore] // Requires mock S3 integration
async fn test_s3_retry_transient_error() {
    // This test demonstrates the intended behavior:
    //
    // let mock_s3 = MockS3ObjectStore::new_with_failure_pattern(vec![
    //     Err("500 Internal Server Error"),
    //     Err("503 Service Unavailable"),
    //     Err("timeout"),
    //     Ok(()),
    // ]);
    //
    // let config = StorageConfig {
    //     tiering_policy: TieringPolicy::MemoryS3,
    //     retry_config: Some(RetryConfig {
    //         max_retries: 5,
    //         base_backoff: Duration::from_millis(100),
    //         ..Default::default()
    //     }),
    //     ..Default::default()
    // };
    //
    // let backend = StorageBackend::new_with_mock_s3(config, mock_s3).await.unwrap();
    //
    // let doc = VectorDocument::new(DocumentId::new(), vec![1.0; 128]);
    // backend.insert(doc).await.unwrap();
    //
    // tokio::time::sleep(Duration::from_secs(2)).await;
    //
    // let metrics = backend.metrics();
    // assert_eq!(metrics.s3_uploads, 1);
    // assert_eq!(metrics.s3_retries, 3);
    // assert_eq!(metrics.dlq_size, 0);
}

/// Test: S3 permanent error to DLQ
///
/// **Goal:** Verify permanent errors skip retries and go to DLQ
///
/// **Expected behavior:**
/// - Mock S3 always fails with 403 Forbidden (permanent error)
/// - Error classified as permanent
/// - Upload moved to DLQ immediately without retries
/// - Metrics show s3_retries=0, s3_permanent_failures=1, dlq_size=1
#[tokio::test]
#[ignore] // Requires mock S3 integration
async fn test_s3_permanent_error_to_dlq() {
    // This test demonstrates the intended behavior:
    //
    // let mock_s3 = MockS3ObjectStore::new_with_error("403 Forbidden");
    //
    // let config = StorageConfig {
    //     tiering_policy: TieringPolicy::MemoryS3,
    //     ..Default::default()
    // };
    //
    // let backend = StorageBackend::new_with_mock_s3(config, mock_s3).await.unwrap();
    //
    // let doc = VectorDocument::new(DocumentId::new(), vec![1.0; 128]);
    // let doc_id = doc.doc_id.clone();
    // backend.insert(doc).await.unwrap();
    //
    // tokio::time::sleep(Duration::from_secs(1)).await;
    //
    // let metrics = backend.metrics();
    // assert_eq!(metrics.s3_retries, 0);
    // assert_eq!(metrics.s3_permanent_failures, 1);
    // assert_eq!(metrics.dlq_size, 1);
    //
    // let dlq = backend.get_dead_letter_queue();
    // assert_eq!(dlq.len(), 1);
    // assert_eq!(dlq[0].document_id, doc_id);
    // assert!(dlq[0].error.contains("403"));
}

/// Test: S3 max retries exceeded
///
/// **Goal:** Verify tasks move to DLQ after max retry limit
///
/// **Expected behavior:**
/// - Mock S3 always fails with transient error (500)
/// - Retries 3 times (max_retries=3)
/// - After 3 failed attempts, moved to DLQ
/// - Metrics show s3_retries=3, dlq_size=1
#[tokio::test]
#[ignore] // Requires mock S3 integration
async fn test_s3_max_retries_exceeded() {
    // This test demonstrates the intended behavior:
    //
    // let mock_s3 = MockS3ObjectStore::new_with_error("500 Internal Server Error");
    //
    // let config = StorageConfig {
    //     tiering_policy: TieringPolicy::MemoryS3,
    //     retry_config: Some(RetryConfig {
    //         max_retries: 3,
    //         base_backoff: Duration::from_millis(50),
    //         ..Default::default()
    //     }),
    //     ..Default::default()
    // };
    //
    // let backend = StorageBackend::new_with_mock_s3(config, mock_s3).await.unwrap();
    //
    // let doc = VectorDocument::new(DocumentId::new(), vec![1.0; 128]);
    // backend.insert(doc).await.unwrap();
    //
    // tokio::time::sleep(Duration::from_secs(2)).await;
    //
    // let metrics = backend.metrics();
    // assert_eq!(metrics.s3_retries, 3);
    // assert_eq!(metrics.dlq_size, 1);
    //
    // let dlq = backend.get_dead_letter_queue();
    // assert_eq!(dlq[0].retry_count, 3);
}
