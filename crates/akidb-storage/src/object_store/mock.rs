//! Mock S3 ObjectStore for testing
//!
//! Provides in-memory S3 simulation with configurable failure patterns for testing
//! retry logic, circuit breakers, and DLQ behavior without real AWS/MinIO dependencies.
//!
//! # Features
//!
//! - **Deterministic Failures**: Pre-defined failure sequences
//! - **Random Failures**: Configurable failure rate (e.g., 30% fail)
//! - **Call History**: Track all operations for assertions
//! - **Latency Simulation**: Realistic network delays
//! - **Always-Fail Mode**: Test permanent error handling
//!
//! # Examples
//!
//! ```rust
//! use akidb_storage::object_store::{MockS3ObjectStore, MockFailure, ObjectStore};
//! use bytes::Bytes;
//!
//! # async fn example() -> akidb_core::CoreResult<()> {
//! // Deterministic failure pattern
//! let mock = MockS3ObjectStore::new_with_failures(vec![
//!     MockFailure::Transient("500 Internal Server Error"),
//!     MockFailure::Transient("503 Service Unavailable"),
//!     MockFailure::Ok,  // Third attempt succeeds
//! ]);
//!
//! // First put fails
//! assert!(mock.put("key1", Bytes::from("data1")).await.is_err());
//!
//! // Second put fails
//! assert!(mock.put("key2", Bytes::from("data2")).await.is_err());
//!
//! // Third put succeeds
//! assert!(mock.put("key3", Bytes::from("data3")).await.is_ok());
//!
//! // Verify call history
//! let history = mock.get_call_history();
//! assert_eq!(history.len(), 3);
//! assert_eq!(mock.failed_puts(), 2);
//! assert_eq!(mock.successful_puts(), 1);
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use bytes::Bytes;
use chrono::Utc;
use parking_lot::RwLock;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::{ObjectMetadata, ObjectStore};
use akidb_core::{CoreError, CoreResult};

/// Mock S3 failure pattern.
#[derive(Debug, Clone)]
pub enum MockFailure {
    /// Transient error (retry-able). Examples: 500, 503, 504, timeout.
    Transient(&'static str),

    /// Permanent error (should go to DLQ). Examples: 403, 404, 400.
    Permanent(&'static str),

    /// Success (no error).
    Ok,
}

impl MockFailure {
    /// Convert failure to CoreError.
    fn to_error(&self) -> Option<CoreError> {
        match self {
            MockFailure::Transient(msg) => {
                Some(CoreError::StorageError(format!("Transient: {}", msg)))
            }
            MockFailure::Permanent(msg) => {
                Some(CoreError::StorageError(format!("Permanent: {}", msg)))
            }
            MockFailure::Ok => None,
        }
    }
}

/// Mock S3 configuration.
#[derive(Debug, Clone)]
pub struct MockS3Config {
    /// Simulated network latency (realistic S3 latency is ~10-50ms).
    pub latency: Duration,

    /// Enable call history tracking.
    pub track_history: bool,
}

impl Default for MockS3Config {
    fn default() -> Self {
        Self {
            latency: Duration::from_millis(10), // Realistic S3 latency
            track_history: true,
        }
    }
}

/// Mock S3 call history entry.
#[derive(Debug, Clone)]
pub struct CallHistoryEntry {
    /// Operation type: "put", "get", "delete", "list", "head", "copy".
    pub operation: String,

    /// Object key.
    pub key: String,

    /// Whether operation succeeded.
    pub success: bool,

    /// Timestamp of operation.
    pub timestamp: Instant,
}

/// Mock S3 ObjectStore implementation for testing.
///
/// This is an in-memory implementation that simulates S3 behavior with
/// configurable failure patterns for comprehensive testing.
pub struct MockS3ObjectStore {
    /// In-memory storage (simulates S3 bucket).
    storage: Arc<RwLock<HashMap<String, Bytes>>>,

    /// Failure pattern queue (deterministic failures).
    failure_queue: Arc<RwLock<VecDeque<MockFailure>>>,

    /// Configuration.
    config: MockS3Config,

    /// Call history (for assertions).
    call_history: Arc<RwLock<Vec<CallHistoryEntry>>>,
}

impl MockS3ObjectStore {
    /// Create new mock S3 with default config (no failures).
    pub fn new() -> Self {
        Self::new_with_config(MockS3Config::default())
    }

    /// Create new mock S3 with custom config.
    pub fn new_with_config(config: MockS3Config) -> Self {
        Self {
            storage: Arc::new(RwLock::new(HashMap::new())),
            failure_queue: Arc::new(RwLock::new(VecDeque::new())),
            config,
            call_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create mock S3 with deterministic failure pattern.
    ///
    /// Failures are consumed in order. Once the queue is empty, all operations succeed.
    ///
    /// # Example
    ///
    /// ```rust
    /// use akidb_storage::object_store::{MockS3ObjectStore, MockFailure};
    ///
    /// let mock = MockS3ObjectStore::new_with_failures(vec![
    ///     MockFailure::Transient("500 Internal Server Error"),
    ///     MockFailure::Transient("503 Service Unavailable"),
    ///     MockFailure::Ok,  // Third attempt succeeds
    /// ]);
    /// ```
    pub fn new_with_failures(pattern: Vec<MockFailure>) -> Self {
        let mut mock = Self::new();
        mock.failure_queue = Arc::new(RwLock::new(pattern.into()));
        mock
    }

    /// Create mock S3 that always fails with given error.
    ///
    /// Pre-fills the failure queue with 1000 identical errors, effectively
    /// simulating an "always fail" scenario.
    ///
    /// # Arguments
    ///
    /// - `error`: Error message
    /// - `is_transient`: If true, creates transient errors (retry-able).
    ///                   If false, creates permanent errors (DLQ).
    ///
    /// # Example
    ///
    /// ```rust
    /// use akidb_storage::object_store::MockS3ObjectStore;
    ///
    /// // Permanent error (DLQ)
    /// let mock = MockS3ObjectStore::new_always_fail("403 Forbidden", false);
    ///
    /// // Transient error (retry)
    /// let mock = MockS3ObjectStore::new_always_fail("503 Service Unavailable", true);
    /// ```
    pub fn new_always_fail(error: &'static str, is_transient: bool) -> Self {
        let failure = if is_transient {
            MockFailure::Transient(error)
        } else {
            MockFailure::Permanent(error)
        };

        // Pre-fill with 1000 failures (effectively "always fail")
        let pattern = vec![failure; 1000];
        Self::new_with_failures(pattern)
    }

    /// Create mock S3 with intermittent failures (flaky network).
    ///
    /// Generates a random sequence of 100 successes/failures based on the failure rate.
    ///
    /// # Arguments
    ///
    /// - `failure_rate`: Probability of failure (0.0-1.0).
    ///                   Example: 0.3 = 30% failure rate
    ///
    /// # Example
    ///
    /// ```rust
    /// use akidb_storage::object_store::MockS3ObjectStore;
    ///
    /// // 60% failure rate (simulates unreliable network)
    /// let mock = MockS3ObjectStore::new_flaky(0.6);
    /// ```
    pub fn new_flaky(failure_rate: f64) -> Self {
        use rand::Rng;

        let mut rng = rand::thread_rng();
        let mut pattern = Vec::new();

        // Generate 100 random successes/failures
        for _ in 0..100 {
            if rng.gen::<f64>() < failure_rate {
                pattern.push(MockFailure::Transient("timeout"));
            } else {
                pattern.push(MockFailure::Ok);
            }
        }

        Self::new_with_failures(pattern)
    }

    /// Get call history for assertions.
    pub fn get_call_history(&self) -> Vec<CallHistoryEntry> {
        self.call_history.read().clone()
    }

    /// Clear call history.
    pub fn clear_history(&self) {
        self.call_history.write().clear();
    }

    /// Get number of successful puts.
    pub fn successful_puts(&self) -> usize {
        self.call_history
            .read()
            .iter()
            .filter(|entry| entry.operation == "put" && entry.success)
            .count()
    }

    /// Get number of failed puts.
    pub fn failed_puts(&self) -> usize {
        self.call_history
            .read()
            .iter()
            .filter(|entry| entry.operation == "put" && !entry.success)
            .count()
    }

    /// Get current storage size (number of objects).
    pub fn storage_size(&self) -> usize {
        self.storage.read().len()
    }

    /// Check if key exists in storage.
    pub fn contains_key(&self, key: &str) -> bool {
        self.storage.read().contains_key(key)
    }

    /// Reset storage and history (useful for test cleanup).
    pub fn reset(&self) {
        self.storage.write().clear();
        self.call_history.write().clear();
    }

    /// Simulate failure (pop from queue).
    fn check_failure(&self) -> Option<CoreError> {
        let mut queue = self.failure_queue.write();
        if let Some(failure) = queue.pop_front() {
            failure.to_error()
        } else {
            None // No more failures in queue, default to success
        }
    }

    /// Record call in history.
    fn record_call(&self, operation: &str, key: &str, success: bool) {
        if self.config.track_history {
            self.call_history.write().push(CallHistoryEntry {
                operation: operation.to_string(),
                key: key.to_string(),
                success,
                timestamp: Instant::now(),
            });
        }
    }
}

impl Default for MockS3ObjectStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ObjectStore for MockS3ObjectStore {
    async fn put(&self, key: &str, data: Bytes) -> CoreResult<()> {
        // Simulate network latency
        tokio::time::sleep(self.config.latency).await;

        // Check for simulated failure
        if let Some(error) = self.check_failure() {
            self.record_call("put", key, false);
            return Err(error);
        }

        // Success: store data
        self.storage.write().insert(key.to_string(), data);
        self.record_call("put", key, true);

        Ok(())
    }

    async fn get(&self, key: &str) -> CoreResult<Bytes> {
        // Simulate network latency
        tokio::time::sleep(self.config.latency).await;

        // Check for simulated failure
        if let Some(error) = self.check_failure() {
            self.record_call("get", key, false);
            return Err(error);
        }

        // Success: retrieve data
        match self.storage.read().get(key).cloned() {
            Some(data) => {
                self.record_call("get", key, true);
                Ok(data)
            }
            None => {
                self.record_call("get", key, false);
                Err(CoreError::NotFound {
                    entity: "object",
                    id: key.to_string(),
                })
            }
        }
    }

    async fn exists(&self, key: &str) -> CoreResult<bool> {
        // Simulate network latency
        tokio::time::sleep(self.config.latency).await;

        // Check for simulated failure
        if let Some(error) = self.check_failure() {
            return Err(error);
        }

        Ok(self.storage.read().contains_key(key))
    }

    async fn delete(&self, key: &str) -> CoreResult<()> {
        // Simulate network latency
        tokio::time::sleep(self.config.latency).await;

        // Check for simulated failure
        if let Some(error) = self.check_failure() {
            self.record_call("delete", key, false);
            return Err(error);
        }

        // Success: remove data (idempotent)
        self.storage.write().remove(key);
        self.record_call("delete", key, true);

        Ok(())
    }

    async fn list(&self, prefix: &str) -> CoreResult<Vec<ObjectMetadata>> {
        // Simulate network latency
        tokio::time::sleep(self.config.latency).await;

        // Check for simulated failure
        if let Some(error) = self.check_failure() {
            return Err(error);
        }

        // Success: list keys with prefix
        let storage = self.storage.read();
        let objects: Vec<ObjectMetadata> = storage
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| ObjectMetadata {
                key: k.clone(),
                size_bytes: v.len() as u64,
                last_modified: Utc::now(),
                etag: Some(format!("{:x}", md5::compute(v.as_ref()))),
            })
            .collect();

        Ok(objects)
    }

    async fn head(&self, key: &str) -> CoreResult<ObjectMetadata> {
        // Simulate network latency
        tokio::time::sleep(self.config.latency).await;

        // Check for simulated failure
        if let Some(error) = self.check_failure() {
            self.record_call("head", key, false);
            return Err(error);
        }

        // Success: get metadata without data
        match self.storage.read().get(key) {
            Some(data) => {
                self.record_call("head", key, true);
                Ok(ObjectMetadata {
                    key: key.to_string(),
                    size_bytes: data.len() as u64,
                    last_modified: Utc::now(),
                    etag: Some(format!("{:x}", md5::compute(data.as_ref()))),
                })
            }
            None => {
                self.record_call("head", key, false);
                Err(CoreError::NotFound {
                    entity: "object",
                    id: key.to_string(),
                })
            }
        }
    }

    async fn copy(&self, from_key: &str, to_key: &str) -> CoreResult<()> {
        // Simulate network latency
        tokio::time::sleep(self.config.latency).await;

        // Check for simulated failure
        if let Some(error) = self.check_failure() {
            self.record_call("copy", from_key, false);
            return Err(error);
        }

        // Success: copy data
        let mut storage = self.storage.write();
        match storage.get(from_key).cloned() {
            Some(data) => {
                storage.insert(to_key.to_string(), data);
                self.record_call("copy", from_key, true);
                Ok(())
            }
            None => {
                self.record_call("copy", from_key, false);
                Err(CoreError::NotFound {
                    entity: "object",
                    id: from_key.to_string(),
                })
            }
        }
    }

    async fn put_multipart(&self, key: &str, parts: Vec<Bytes>) -> CoreResult<()> {
        // Simulate network latency (longer for multipart)
        tokio::time::sleep(self.config.latency * parts.len() as u32).await;

        // Check for simulated failure
        if let Some(error) = self.check_failure() {
            self.record_call("put_multipart", key, false);
            return Err(error);
        }

        // Success: concatenate parts and store
        let mut combined = Vec::new();
        for part in parts {
            combined.extend_from_slice(&part);
        }

        self.storage
            .write()
            .insert(key.to_string(), Bytes::from(combined));
        self.record_call("put_multipart", key, true);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_s3_success() {
        let mock = MockS3ObjectStore::new();

        // Put
        mock.put("test-key", Bytes::from("data")).await.unwrap();
        assert_eq!(mock.storage_size(), 1);
        assert!(mock.contains_key("test-key"));

        // Get
        let data = mock.get("test-key").await.unwrap();
        assert_eq!(data, Bytes::from("data"));

        // Delete
        mock.delete("test-key").await.unwrap();
        assert_eq!(mock.storage_size(), 0);

        // Get after delete should fail
        assert!(mock.get("test-key").await.is_err());
    }

    #[tokio::test]
    async fn test_mock_s3_deterministic_failures() {
        let mock = MockS3ObjectStore::new_with_failures(vec![
            MockFailure::Transient("500 Internal Server Error"),
            MockFailure::Transient("503 Service Unavailable"),
            MockFailure::Ok,
        ]);

        // First put should fail with 500
        let result1 = mock.put("key1", Bytes::from("data1")).await;
        assert!(result1.is_err());
        assert!(result1.unwrap_err().to_string().contains("500"));

        // Second put should fail with 503
        let result2 = mock.put("key2", Bytes::from("data2")).await;
        assert!(result2.is_err());
        assert!(result2.unwrap_err().to_string().contains("503"));

        // Third put should succeed
        let result3 = mock.put("key3", Bytes::from("data3")).await;
        assert!(result3.is_ok());

        assert_eq!(mock.storage_size(), 1); // Only key3 stored
    }

    #[tokio::test]
    async fn test_mock_s3_call_history() {
        let mock = MockS3ObjectStore::new();

        mock.put("key1", Bytes::from("data1")).await.unwrap();
        mock.put("key2", Bytes::from("data2")).await.unwrap();
        mock.get("key1").await.unwrap();

        let history = mock.get_call_history();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].operation, "put");
        assert_eq!(history[1].operation, "put");
        assert_eq!(history[2].operation, "get");

        assert_eq!(mock.successful_puts(), 2);
        assert_eq!(mock.failed_puts(), 0);
    }

    #[tokio::test]
    async fn test_mock_s3_always_fail() {
        let mock = MockS3ObjectStore::new_always_fail("403 Forbidden", false);

        // All puts should fail
        for i in 0..10 {
            let result = mock.put(&format!("key{}", i), Bytes::from("data")).await;
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("403"));
        }

        assert_eq!(mock.storage_size(), 0); // Nothing stored
        assert_eq!(mock.failed_puts(), 10);
    }

    #[tokio::test]
    async fn test_mock_s3_list() {
        let mock = MockS3ObjectStore::new();

        // Add multiple objects
        mock.put("prefix/file1.txt", Bytes::from("data1"))
            .await
            .unwrap();
        mock.put("prefix/file2.txt", Bytes::from("data2"))
            .await
            .unwrap();
        mock.put("other/file3.txt", Bytes::from("data3"))
            .await
            .unwrap();

        // List with prefix
        let objects = mock.list("prefix/").await.unwrap();
        assert_eq!(objects.len(), 2);

        // List all
        let all_objects = mock.list("").await.unwrap();
        assert_eq!(all_objects.len(), 3);
    }

    #[tokio::test]
    async fn test_mock_s3_head() {
        let mock = MockS3ObjectStore::new();

        mock.put("test-key", Bytes::from("test data"))
            .await
            .unwrap();

        let metadata = mock.head("test-key").await.unwrap();
        assert_eq!(metadata.key, "test-key");
        assert_eq!(metadata.size_bytes, 9); // "test data" = 9 bytes
        assert!(metadata.etag.is_some());
    }

    #[tokio::test]
    async fn test_mock_s3_copy() {
        let mock = MockS3ObjectStore::new();

        mock.put("source", Bytes::from("data")).await.unwrap();
        mock.copy("source", "destination").await.unwrap();

        assert_eq!(mock.storage_size(), 2);
        assert!(mock.contains_key("source"));
        assert!(mock.contains_key("destination"));

        let dest_data = mock.get("destination").await.unwrap();
        assert_eq!(dest_data, Bytes::from("data"));
    }

    #[tokio::test]
    async fn test_mock_s3_multipart() {
        let mock = MockS3ObjectStore::new();

        let parts = vec![
            Bytes::from("part1"),
            Bytes::from("part2"),
            Bytes::from("part3"),
        ];

        mock.put_multipart("multipart-key", parts).await.unwrap();

        let data = mock.get("multipart-key").await.unwrap();
        assert_eq!(data, Bytes::from("part1part2part3"));
    }
}
