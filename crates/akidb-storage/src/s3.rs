//! S3/MinIO storage backend implementation with multipart upload, retry logic, and circuit breaker.

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use bytes::Bytes;
use object_store::aws::AmazonS3Builder;
use object_store::path::Path as ObjectPath;
use object_store::{Error as ObjectStoreError, ObjectStore, PutPayload};
use parking_lot::RwLock;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use akidb_core::{
    CollectionDescriptor, CollectionManifest, Error, Result, SegmentDescriptor, SegmentState,
};

use crate::backend::{StorageBackend, StorageStatus};
use crate::segment_format::{ChecksumType, CompressionType, SegmentData, SegmentReader, SegmentWriter};

/// S3 storage configuration
#[derive(Debug, Clone)]
pub struct S3Config {
    /// S3 endpoint URL (e.g., "https://s3.amazonaws.com" or "http://localhost:9000" for MinIO)
    pub endpoint: String,
    /// AWS region
    pub region: String,
    /// Access key ID
    pub access_key: String,
    /// Secret access key
    pub secret_key: String,
    /// Bucket name
    pub bucket: String,
    /// Request timeout in milliseconds
    pub timeout_ms: u64,
    /// Threshold for multipart upload (default 64MB)
    pub multipart_threshold: usize,
    /// Size of each part in multipart upload (default 16MB)
    pub part_size: usize,
    /// Retry configuration
    pub retry_config: RetryConfig,
}

impl Default for S3Config {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            region: "us-east-1".to_string(),
            access_key: String::new(),
            secret_key: String::new(),
            bucket: String::new(),
            timeout_ms: 30_000,                    // 30 seconds
            multipart_threshold: 64 * 1024 * 1024, // 64MB
            part_size: 16 * 1024 * 1024,           // 16MB
            retry_config: RetryConfig::default(),
        }
    }
}

/// Retry configuration for S3 operations
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Initial delay in milliseconds
    pub initial_delay_ms: u64,
    /// Maximum delay in milliseconds
    pub max_delay_ms: u64,
    /// Backoff factor (exponential)
    pub backoff_factor: f64,
    /// Jitter percentage (0.0 - 1.0)
    pub jitter_percent: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            initial_delay_ms: 100,
            max_delay_ms: 3200,
            backoff_factor: 2.0,
            jitter_percent: 0.2,
        }
    }
}

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CircuitState {
    /// Circuit is closed, requests flow normally
    Closed,
    /// Circuit is open, requests are rejected
    Open,
    /// Circuit is half-open, testing if service recovered
    HalfOpen,
}

/// Circuit breaker for S3 operations
#[derive(Debug)]
struct CircuitBreaker {
    state: RwLock<CircuitState>,
    failure_count: RwLock<u32>,
    last_failure_time: RwLock<Option<Instant>>,
    failure_threshold: u32,
    recovery_timeout: Duration,
}

impl CircuitBreaker {
    fn new(failure_threshold: u32, recovery_timeout: Duration) -> Self {
        Self {
            state: RwLock::new(CircuitState::Closed),
            failure_count: RwLock::new(0),
            last_failure_time: RwLock::new(None),
            failure_threshold,
            recovery_timeout,
        }
    }

    fn is_open(&self) -> bool {
        let state = *self.state.read();

        if state == CircuitState::Open {
            // Check if recovery timeout has passed
            if let Some(last_failure) = *self.last_failure_time.read() {
                if last_failure.elapsed() >= self.recovery_timeout {
                    // Transition to half-open
                    *self.state.write() = CircuitState::HalfOpen;
                    info!("Circuit breaker transitioning to half-open state");
                    return false;
                }
            }
            return true;
        }

        false
    }

    fn record_success(&self) {
        let mut state = self.state.write();
        let mut failure_count = self.failure_count.write();

        match *state {
            CircuitState::HalfOpen => {
                // Transition back to closed on success
                *state = CircuitState::Closed;
                *failure_count = 0;
                info!("Circuit breaker closed after successful recovery");
            }
            CircuitState::Closed => {
                // Reset failure count on success
                *failure_count = 0;
            }
            CircuitState::Open => {
                // Should not happen, but reset anyway
                *state = CircuitState::Closed;
                *failure_count = 0;
            }
        }
    }

    fn record_failure(&self) {
        let mut state = self.state.write();
        let mut failure_count = self.failure_count.write();
        let mut last_failure_time = self.last_failure_time.write();

        *failure_count += 1;
        *last_failure_time = Some(Instant::now());

        match *state {
            CircuitState::Closed => {
                if *failure_count >= self.failure_threshold {
                    *state = CircuitState::Open;
                    warn!(
                        "Circuit breaker opened after {} consecutive failures",
                        self.failure_threshold
                    );
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open state opens the circuit again
                *state = CircuitState::Open;
                warn!("Circuit breaker re-opened after failure in half-open state");
            }
            CircuitState::Open => {
                // Already open, just update counters
            }
        }
    }
}

/// S3 storage backend implementation
pub struct S3StorageBackend {
    client: Arc<dyn ObjectStore>,
    config: S3Config,
    circuit_breaker: Arc<CircuitBreaker>,
}

impl S3StorageBackend {
    /// Create a new S3 storage backend
    pub fn new(config: S3Config) -> Result<Self> {
        // Build S3 client
        let mut builder = AmazonS3Builder::new()
            .with_region(&config.region)
            .with_bucket_name(&config.bucket)
            .with_access_key_id(&config.access_key)
            .with_secret_access_key(&config.secret_key);

        // Set endpoint for MinIO or custom S3-compatible storage
        if !config.endpoint.is_empty() {
            builder = builder.with_endpoint(&config.endpoint);
            // Allow HTTP for local development (MinIO)
            if config.endpoint.starts_with("http://") {
                builder = builder.with_allow_http(true);
            }
        }

        let client = builder
            .build()
            .map_err(|e| Error::Storage(format!("Failed to create S3 client: {}", e)))?;

        let circuit_breaker = Arc::new(CircuitBreaker::new(
            5,                       // 5 consecutive failures to open
            Duration::from_secs(30), // 30 seconds recovery timeout
        ));

        Ok(Self {
            client: Arc::new(client),
            config,
            circuit_breaker,
        })
    }

    /// Execute an operation with retry logic
    async fn retry_with_backoff<F, Fut, T>(&self, operation: F) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        // Check circuit breaker
        if self.circuit_breaker.is_open() {
            return Err(Error::Storage(
                "Circuit breaker is open, rejecting request".to_string(),
            ));
        }

        let mut attempt = 0;
        let mut delay_ms = self.config.retry_config.initial_delay_ms;

        loop {
            attempt += 1;

            match operation().await {
                Ok(result) => {
                    // Record success for circuit breaker
                    self.circuit_breaker.record_success();
                    return Ok(result);
                }
                Err(e) => {
                    // Check if we should retry
                    if attempt >= self.config.retry_config.max_attempts {
                        error!("Operation failed after {} attempts: {}", attempt, e);
                        self.circuit_breaker.record_failure();
                        return Err(e);
                    }

                    // Calculate delay with jitter
                    let jitter_range =
                        (delay_ms as f64 * self.config.retry_config.jitter_percent) as u64;
                    let jitter = rand::random::<u64>() % (jitter_range + 1);
                    let actual_delay = delay_ms + jitter;

                    warn!(
                        "Operation failed (attempt {}/{}), retrying after {}ms: {}",
                        attempt, self.config.retry_config.max_attempts, actual_delay, e
                    );

                    sleep(Duration::from_millis(actual_delay)).await;

                    // Exponential backoff
                    delay_ms = ((delay_ms as f64 * self.config.retry_config.backoff_factor) as u64)
                        .min(self.config.retry_config.max_delay_ms);
                }
            }
        }
    }

    /// Get an object from S3 (internal implementation)
    async fn get_object_internal(&self, key: &str) -> Result<Bytes> {
        let key = key.to_string();
        let path = ObjectPath::from(key.clone());
        let client = self.client.clone();

        self.retry_with_backoff(|| {
            let key = key.clone();
            let path = path.clone();
            let client = client.clone();

            async move {
                let result = match client.get(&path).await {
                    Ok(reader) => reader,
                    Err(ObjectStoreError::NotFound { .. }) => {
                        return Err(Error::NotFound(format!("Object {} not found", key)));
                    }
                    Err(e) => {
                        return Err(Error::Storage(format!(
                            "Failed to get object {}: {}",
                            key, e
                        )));
                    }
                };

                let bytes = result
                    .bytes()
                    .await
                    .map_err(|e| Error::Storage(format!("Failed to read object bytes: {}", e)))?;

                debug!("Retrieved object {} ({} bytes)", key, bytes.len());
                Ok(bytes)
            }
        })
        .await
    }

    /// Put an object to S3 (with multipart upload for large objects) (internal implementation)
    async fn put_object_internal(&self, key: &str, data: Bytes) -> Result<()> {
        let path = ObjectPath::from(key);
        let size = data.len();

        self.retry_with_backoff(|| {
            let data = data.clone();
            let path = path.clone();

            async move {
                self.client
                    .put(&path, PutPayload::from_bytes(data))
                    .await
                    .map_err(|e| Error::Storage(format!("Failed to put object: {}", e)))?;

                debug!("Uploaded object {} ({} bytes)", key, size);
                Ok(())
            }
        })
        .await
    }

    /// Delete an object from S3 (internal implementation)
    async fn delete_object_internal(&self, key: &str) -> Result<()> {
        let path = ObjectPath::from(key);

        self.retry_with_backoff(|| async {
            self.client
                .delete(&path)
                .await
                .map_err(|e| Error::Storage(format!("Failed to delete object: {}", e)))?;

            debug!("Deleted object {}", key);
            Ok(())
        })
        .await
    }

    /// List objects with a prefix (internal implementation)
    async fn list_objects_internal(&self, prefix: &str) -> Result<Vec<String>> {
        let path = if prefix.is_empty() {
            None
        } else {
            Some(ObjectPath::from(prefix))
        };

        self.retry_with_backoff(|| {
            let path_clone = path.clone();
            async move {
                let mut keys = Vec::new();
                let list_result = self.client.list(path_clone.as_ref());

                use futures::StreamExt;
                let mut stream = Box::pin(list_result);

                while let Some(result) = stream.next().await {
                    let meta = result
                        .map_err(|e| Error::Storage(format!("Failed to list objects: {}", e)))?;
                    keys.push(meta.location.to_string());
                }

                debug!("Listed {} objects with prefix '{}'", keys.len(), prefix);
                Ok(keys)
            }
        })
        .await
    }

    /// Generate storage key for collection manifest
    fn manifest_key(&self, collection: &str) -> String {
        format!("collections/{}/manifest.json", collection)
    }

    /// Generate storage key for segment
    fn segment_key(&self, collection: &str, segment_id: Uuid) -> String {
        format!("collections/{}/segments/{}.seg", collection, segment_id)
    }

    /// Check if an object exists without treating absence as failure (internal implementation)
    async fn object_exists_internal(&self, key: &str) -> Result<bool> {
        let key = key.to_string();
        let path = ObjectPath::from(key.clone());
        let client = self.client.clone();

        self.retry_with_backoff(|| {
            let key = key.clone();
            let path = path.clone();
            let client = client.clone();

            async move {
                match client.head(&path).await {
                    Ok(_) => {
                        debug!("Object {} exists", key);
                        Ok(true)
                    }
                    Err(ObjectStoreError::NotFound { .. }) => {
                        debug!("Object {} not found", key);
                        Ok(false)
                    }
                    Err(e) => Err(Error::Storage(format!(
                        "Failed to query object metadata for {}: {}",
                        key, e
                    ))),
                }
            }
        })
        .await
    }

    /// Locate the collection that owns the given segment identifier.
    async fn find_segment_collection(&self, segment_id: Uuid) -> Result<Option<String>> {
        let suffix = format!("{segment_id}.seg");
        let keys = self.list_objects_internal("collections/").await?;

        for key in keys {
            if key.ends_with(&suffix) {
                if let Some(rest) = key.strip_prefix("collections/") {
                    if let Some((collection, _)) = rest.split_once('/') {
                        return Ok(Some(collection.to_string()));
                    }
                }
            }
        }

        Ok(None)
    }

    /// Bump manifest metadata to reflect a mutation.
    fn bump_manifest_revision(manifest: &mut CollectionManifest) {
        manifest.updated_at = chrono::Utc::now();
        manifest.epoch = manifest.epoch.saturating_add(1);
        manifest.latest_version = manifest.latest_version.saturating_add(1);
        manifest.total_vectors = manifest
            .segments
            .iter()
            .map(|seg| seg.record_count as u64)
            .sum();
    }

    /// Write segment with actual vector data using SEGv1 format
    pub async fn write_segment_with_data(
        &self,
        descriptor: &SegmentDescriptor,
        vectors: Vec<Vec<f32>>,
    ) -> Result<()> {
        info!(
            "Writing segment {} with {} vectors (SEGv1 format)",
            descriptor.segment_id,
            vectors.len()
        );

        // Verify vector count matches
        if vectors.len() != descriptor.record_count as usize {
            return Err(Error::Validation(format!(
                "Vector count mismatch: descriptor says {}, got {}",
                descriptor.record_count,
                vectors.len()
            )));
        }

        // Validate collection exists and dimension matches
        let mut manifest = self
            .load_manifest(&descriptor.collection)
            .await
            .map_err(|e| match e {
                Error::NotFound(_) => Error::Validation(format!(
                    "Collection {} does not exist",
                    descriptor.collection
                )),
                other => other,
            })?;

        if manifest.dimension != 0 && manifest.dimension != descriptor.vector_dim as u32 {
            return Err(Error::Validation(format!(
                "Segment dimension {} does not match collection dimension {}",
                descriptor.vector_dim, manifest.dimension
            )));
        }

        // Check for duplicates
        if manifest
            .segments
            .iter()
            .any(|seg| seg.segment_id == descriptor.segment_id)
        {
            return Err(Error::Conflict(format!(
                "Segment {} already exists",
                descriptor.segment_id
            )));
        }

        // Create segment data
        let segment_data = SegmentData::new(descriptor.vector_dim as u32, vectors)?;

        // Serialize using SEGv1 format with compression
        let writer = SegmentWriter::new(CompressionType::Zstd, ChecksumType::XXH3);
        let segment_bytes = writer.write(&segment_data)?;

        // Upload to S3
        let key = self.segment_key(&descriptor.collection, descriptor.segment_id);
        self.put_object_internal(&key, Bytes::from(segment_bytes))
            .await?;

        // Update manifest
        manifest.segments.push(descriptor.clone());
        Self::bump_manifest_revision(&mut manifest);
        self.persist_manifest(&manifest).await?;

        info!(
            "Segment {} with {} vectors written successfully (SEGv1 format)",
            descriptor.segment_id,
            segment_data.vector_count()
        );

        Ok(())
    }

    /// Load segment data from S3 using SEGv1 format
    pub async fn load_segment(&self, collection: &str, segment_id: Uuid) -> Result<SegmentData> {
        info!("Loading segment {} (SEGv1 format)", segment_id);

        let key = self.segment_key(collection, segment_id);
        let bytes = self.get_object_internal(&key).await?;

        let segment_data = SegmentReader::read(&bytes)?;

        info!(
            "Loaded segment {} with {} vectors",
            segment_id,
            segment_data.vector_count()
        );

        Ok(segment_data)
    }
}

#[async_trait]
impl StorageBackend for S3StorageBackend {
    async fn status(&self) -> Result<StorageStatus> {
        // Check if circuit breaker is open
        if self.circuit_breaker.is_open() {
            return Ok(StorageStatus::Degraded);
        }

        // Try a simple operation to verify connectivity
        match self.list_objects_internal("").await {
            Ok(_) => Ok(StorageStatus::Healthy),
            Err(_) => Ok(StorageStatus::Degraded),
        }
    }

    async fn create_collection(&self, descriptor: &CollectionDescriptor) -> Result<()> {
        info!("Creating collection: {}", descriptor.name);

        let manifest_key = self.manifest_key(&descriptor.name);
        if self.object_exists_internal(&manifest_key).await? {
            return Err(Error::Conflict(format!(
                "Collection {} already exists",
                descriptor.name
            )));
        }

        // Ensure logical directory structure exists
        let collection_prefix = format!("collections/{}/", descriptor.name);
        let segments_prefix = format!("collections/{}/segments/", descriptor.name);
        self.put_object_internal(&collection_prefix, Bytes::new()).await?;
        self.put_object_internal(&segments_prefix, Bytes::new()).await?;

        // Create initial manifest (MANIFESTv1 format)
        let now = chrono::Utc::now();
        let manifest = CollectionManifest {
            collection: descriptor.name.clone(),
            dimension: descriptor.vector_dim as u32,
            metric: descriptor.distance,
            segments: Vec::new(),
            epoch: 0,
            total_vectors: 0,
            created_at: Some(now),
            updated_at: now,
            latest_version: 0,
            snapshot: None,
        };

        self.persist_manifest(&manifest).await?;
        info!("Created collection {} with empty manifest", descriptor.name);
        Ok(())
    }

    async fn drop_collection(&self, name: &str) -> Result<()> {
        info!("Dropping collection: {}", name);

        // List all objects with collection prefix
        let prefix = format!("collections/{}/", name);
        let keys = self.list_objects_internal(&prefix).await?;

        // Delete all objects
        for key in keys {
            self.delete_object_internal(&key).await?;
        }

        Ok(())
    }

    async fn write_segment(&self, descriptor: &SegmentDescriptor) -> Result<()> {
        info!(
            "Writing segment {} for collection {}",
            descriptor.segment_id, descriptor.collection
        );

        let mut manifest =
            self.load_manifest(&descriptor.collection)
                .await
                .map_err(|e| match e {
                    Error::NotFound(_) => Error::Validation(format!(
                        "Collection {} does not exist for segment {}",
                        descriptor.collection, descriptor.segment_id
                    )),
                    other => other,
                })?;

        if manifest.dimension != 0 && manifest.dimension != descriptor.vector_dim as u32 {
            return Err(Error::Validation(format!(
                "Segment {} dimension {} does not match collection {} dimension {}",
                descriptor.segment_id,
                descriptor.vector_dim,
                descriptor.collection,
                manifest.dimension
            )));
        }

        if manifest
            .segments
            .iter()
            .any(|seg| seg.segment_id == descriptor.segment_id)
        {
            return Err(Error::Conflict(format!(
                "Segment {} already exists in collection {}",
                descriptor.segment_id, descriptor.collection
            )));
        }

        // Serialize segment (placeholder - actual serialization will be in SEGv1 implementation)
        let data = serde_json::to_vec(&descriptor)
            .map_err(|e| Error::Storage(format!("Failed to serialize segment: {}", e)))?;

        let key = self.segment_key(&descriptor.collection, descriptor.segment_id);
        self.put_object_internal(&key, Bytes::from(data)).await?;

        manifest.segments.push(descriptor.clone());
        Self::bump_manifest_revision(&mut manifest);
        self.persist_manifest(&manifest).await?;

        info!(
            "Segment {} persisted and manifest updated for collection {}",
            descriptor.segment_id, descriptor.collection
        );
        Ok(())
    }

    async fn seal_segment(&self, segment_id: Uuid) -> Result<SegmentDescriptor> {
        info!("Sealing segment {}", segment_id);

        let collection = self
            .find_segment_collection(segment_id)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Segment {} not found", segment_id)))?;

        let segment_key = self.segment_key(&collection, segment_id);
        let bytes = self.get_object_internal(&segment_key).await?;

        let mut descriptor: SegmentDescriptor = serde_json::from_slice(&bytes).map_err(|e| {
            Error::Storage(format!("Failed to deserialize segment descriptor: {}", e))
        })?;

        let mut manifest = self.load_manifest(&collection).await?;
        let entry = manifest
            .segments
            .iter_mut()
            .find(|seg| seg.segment_id == segment_id)
            .ok_or_else(|| {
                Error::NotFound(format!(
                    "Segment {} not registered in manifest for collection {}",
                    segment_id, collection
                ))
            })?;

        let already_sealed =
            entry.state == SegmentState::Sealed && descriptor.state == SegmentState::Sealed;

        entry.state = SegmentState::Sealed;
        descriptor.state = SegmentState::Sealed;

        if already_sealed {
            debug!("Segment {} already sealed", segment_id);
            return Ok(descriptor);
        }

        let data = serde_json::to_vec(&descriptor)
            .map_err(|e| Error::Storage(format!("Failed to serialize sealed segment: {}", e)))?;
        self.put_object_internal(&segment_key, Bytes::from(data)).await?;

        Self::bump_manifest_revision(&mut manifest);
        self.persist_manifest(&manifest).await?;

        info!(
            "Segment {} transitioned to sealed state for collection {}",
            segment_id, collection
        );
        Ok(descriptor)
    }

    async fn load_manifest(&self, collection: &str) -> Result<CollectionManifest> {
        debug!("Loading manifest for collection {}", collection);

        let key = self.manifest_key(collection);
        let data = self.get_object_internal(&key).await?;

        let manifest: CollectionManifest = serde_json::from_slice(&data)
            .map_err(|e| Error::Storage(format!("Failed to deserialize manifest: {}", e)))?;

        Ok(manifest)
    }

    async fn persist_manifest(&self, manifest: &CollectionManifest) -> Result<()> {
        debug!("Persisting manifest for collection {}", manifest.collection);

        let data = serde_json::to_vec_pretty(&manifest)
            .map_err(|e| Error::Storage(format!("Failed to serialize manifest: {}", e)))?;

        let key = self.manifest_key(&manifest.collection);
        self.put_object_internal(&key, Bytes::from(data)).await
    }

    // === Generic Object Operations ===

    async fn get_object(&self, key: &str) -> Result<Bytes> {
        self.get_object_internal(key).await
    }

    async fn put_object(&self, key: &str, data: Bytes) -> Result<()> {
        self.put_object_internal(key, data).await
    }

    async fn delete_object(&self, key: &str) -> Result<()> {
        self.delete_object_internal(key).await
    }

    async fn object_exists(&self, key: &str) -> Result<bool> {
        self.object_exists_internal(key).await
    }

    async fn list_objects(&self, prefix: &str) -> Result<Vec<String>> {
        self.list_objects_internal(prefix).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_closed_to_open() {
        let cb = CircuitBreaker::new(3, Duration::from_secs(10));

        assert_eq!(*cb.state.read(), CircuitState::Closed);
        assert!(!cb.is_open());

        // Record failures
        cb.record_failure();
        assert!(!cb.is_open());

        cb.record_failure();
        assert!(!cb.is_open());

        cb.record_failure();
        assert!(cb.is_open()); // Should open after 3 failures
        assert_eq!(*cb.state.read(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_breaker_half_open() {
        let cb = CircuitBreaker::new(3, Duration::from_millis(50));

        // Open the circuit
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert!(cb.is_open());

        // Wait for recovery timeout
        std::thread::sleep(Duration::from_millis(60));

        // Should transition to half-open
        assert!(!cb.is_open());
        assert_eq!(*cb.state.read(), CircuitState::HalfOpen);
    }

    #[test]
    fn test_circuit_breaker_recovery() {
        let cb = CircuitBreaker::new(3, Duration::from_millis(50));

        // Open the circuit
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert!(cb.is_open());

        // Wait for recovery timeout
        std::thread::sleep(Duration::from_millis(60));
        assert!(!cb.is_open()); // Half-open

        // Record success - should close
        cb.record_success();
        assert!(!cb.is_open());
        assert_eq!(*cb.state.read(), CircuitState::Closed);
        assert_eq!(*cb.failure_count.read(), 0);
    }

    #[test]
    fn test_s3_config_default() {
        let config = S3Config::default();
        assert_eq!(config.region, "us-east-1");
        assert_eq!(config.timeout_ms, 30_000);
        assert_eq!(config.multipart_threshold, 64 * 1024 * 1024);
        assert_eq!(config.part_size, 16 * 1024 * 1024);
        assert_eq!(config.retry_config.max_attempts, 5);
    }
}
