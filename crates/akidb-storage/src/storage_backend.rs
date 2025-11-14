//! Storage backend - Unified interface integrating WAL, ObjectStore, and Snapshotter
//!
//! Provides three tiering policies for different performance/cost trade-offs.

use crate::dlq::DeadLetterQueue;
use crate::object_store::{LocalObjectStore, ObjectStore, S3Config, S3ObjectStore};
use crate::snapshotter::{JsonSnapshotter, Snapshotter};
use crate::tiering::{StorageConfig, TieringPolicy};
use crate::wal::{FileWAL, FileWALConfig, LogEntry, LogSequenceNumber, WriteAheadLog};
use akidb_core::{CollectionId, CoreResult, DocumentId, VectorDocument};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use std::collections::{HashMap, VecDeque};
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

/// Task for background S3 upload
#[derive(Clone, Debug)]
struct S3UploadTask {
    collection_id: CollectionId,
    doc: VectorDocument,
}

/// Metadata for a failed S3 upload awaiting retry.
#[derive(Clone, Debug)]
struct S3RetryTask {
    /// Original upload task
    task: S3UploadTask,

    /// Number of retry attempts so far (0-indexed)
    attempt: u32,

    /// Next retry time (exponential backoff)
    next_retry_at: std::time::Instant,

    /// Last error message
    last_error: String,
}

/// Configuration for S3 retry behavior.
#[derive(Clone, Debug)]
pub struct RetryConfig {
    /// Maximum retry attempts before DLQ (default: 5)
    pub max_retries: u32,

    /// Base backoff duration (default: 1s)
    pub base_backoff: std::time::Duration,

    /// Maximum backoff duration (default: 64s)
    pub max_backoff: std::time::Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            base_backoff: std::time::Duration::from_secs(1),
            max_backoff: std::time::Duration::from_secs(64),
        }
    }
}

// DLQEntry moved to dlq.rs module
pub use crate::dlq::DLQEntry;

/// Classify S3 error as transient (retry) or permanent (DLQ).
#[derive(Debug, PartialEq)]
enum ErrorClass {
    Transient, // Retry with backoff
    Permanent, // Move to DLQ
}

/// Storage metrics for monitoring and debugging
#[derive(Debug, Clone, Default)]
pub struct StorageMetrics {
    /// Total insert operations
    pub inserts: u64,
    /// Total query operations
    pub queries: u64,
    /// Total delete operations
    pub deletes: u64,
    /// S3 uploads completed
    pub s3_uploads: u64,
    /// S3 downloads completed
    pub s3_downloads: u64,
    /// Cache hits (S3Only policy)
    pub cache_hits: u64,
    /// Cache misses (S3Only policy)
    pub cache_misses: u64,
    /// Current WAL size in bytes
    pub wal_size_bytes: u64,
    /// Last snapshot timestamp
    pub last_snapshot_at: Option<DateTime<Utc>>,
    /// Number of compactions performed
    pub compactions: u64,
    /// Day 4: Retry metrics
    /// Number of successful retries
    pub s3_retries: u64,
    /// Number of permanent failures
    pub s3_permanent_failures: u64,
    /// Current DLQ size
    pub dlq_size: usize,
    /// Phase 7 Week 1: Circuit breaker metrics
    /// Circuit breaker state (0=closed, 1=open, 2=half-open)
    pub circuit_breaker_state: u8,
    /// Circuit breaker error rate (0.0-1.0)
    pub circuit_breaker_error_rate: f64,
}

impl StorageMetrics {
    /// Calculate cache hit rate (0.0 = 0%, 1.0 = 100%)
    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            self.cache_hits as f64 / total as f64
        }
    }

    /// Exports storage metrics in Prometheus text format.
    ///
    /// Returns metrics for S3 operations, DLQ, circuit breaker, and system resources.
    /// Compatible with Prometheus scraping (text format v0.0.4).
    ///
    /// # Example
    ///
    /// ```rust
    /// use akidb_storage::StorageMetrics;
    ///
    /// let metrics = StorageMetrics::default();
    /// let prometheus_output = metrics.export_prometheus();
    /// assert!(prometheus_output.contains("akidb_s3_uploads_total"));
    /// ```
    pub fn export_prometheus(&self) -> String {
        let mut output = String::new();

        // === Storage Operations ===
        output.push_str("# HELP akidb_storage_inserts_total Total insert operations\n");
        output.push_str("# TYPE akidb_storage_inserts_total counter\n");
        output.push_str(&format!("akidb_storage_inserts_total {}\n", self.inserts));

        output.push_str("# HELP akidb_storage_queries_total Total query operations\n");
        output.push_str("# TYPE akidb_storage_queries_total counter\n");
        output.push_str(&format!("akidb_storage_queries_total {}\n", self.queries));

        output.push_str("# HELP akidb_storage_deletes_total Total delete operations\n");
        output.push_str("# TYPE akidb_storage_deletes_total counter\n");
        output.push_str(&format!("akidb_storage_deletes_total {}\n", self.deletes));

        // === S3 Operations ===
        output.push_str("# HELP akidb_s3_uploads_total Total S3 uploads completed\n");
        output.push_str("# TYPE akidb_s3_uploads_total counter\n");
        output.push_str(&format!("akidb_s3_uploads_total {}\n", self.s3_uploads));

        output.push_str("# HELP akidb_s3_downloads_total Total S3 downloads completed\n");
        output.push_str("# TYPE akidb_s3_downloads_total counter\n");
        output.push_str(&format!("akidb_s3_downloads_total {}\n", self.s3_downloads));

        output.push_str("# HELP akidb_s3_retries_total Total S3 retry attempts\n");
        output.push_str("# TYPE akidb_s3_retries_total counter\n");
        output.push_str(&format!("akidb_s3_retries_total {}\n", self.s3_retries));

        output.push_str("# HELP akidb_s3_permanent_failures_total Total S3 permanent failures\n");
        output.push_str("# TYPE akidb_s3_permanent_failures_total counter\n");
        output.push_str(&format!(
            "akidb_s3_permanent_failures_total {}\n",
            self.s3_permanent_failures
        ));

        // === DLQ Metrics ===
        output.push_str("# HELP akidb_dlq_size Current Dead Letter Queue size\n");
        output.push_str("# TYPE akidb_dlq_size gauge\n");
        output.push_str(&format!("akidb_dlq_size {}\n", self.dlq_size));

        // === Circuit Breaker Metrics ===
        output.push_str("# HELP akidb_circuit_breaker_state Circuit breaker state (0=Closed, 1=Open, 2=HalfOpen)\n");
        output.push_str("# TYPE akidb_circuit_breaker_state gauge\n");
        output.push_str(&format!(
            "akidb_circuit_breaker_state {}\n",
            self.circuit_breaker_state
        ));

        output.push_str("# HELP akidb_circuit_breaker_error_rate Current error rate (0.0-1.0)\n");
        output.push_str("# TYPE akidb_circuit_breaker_error_rate gauge\n");
        output.push_str(&format!(
            "akidb_circuit_breaker_error_rate {:.4}\n",
            self.circuit_breaker_error_rate
        ));

        // === Cache Metrics ===
        output.push_str("# HELP akidb_cache_hits_total Total cache hits (S3Only policy)\n");
        output.push_str("# TYPE akidb_cache_hits_total counter\n");
        output.push_str(&format!("akidb_cache_hits_total {}\n", self.cache_hits));

        output.push_str("# HELP akidb_cache_misses_total Total cache misses (S3Only policy)\n");
        output.push_str("# TYPE akidb_cache_misses_total counter\n");
        output.push_str(&format!("akidb_cache_misses_total {}\n", self.cache_misses));

        // === WAL Metrics ===
        output.push_str("# HELP akidb_wal_size_bytes Current WAL size in bytes\n");
        output.push_str("# TYPE akidb_wal_size_bytes gauge\n");
        output.push_str(&format!("akidb_wal_size_bytes {}\n", self.wal_size_bytes));

        output.push_str("# HELP akidb_compactions_total Total compactions performed\n");
        output.push_str("# TYPE akidb_compactions_total counter\n");
        output.push_str(&format!("akidb_compactions_total {}\n", self.compactions));

        output
    }
}

/// Cache statistics for S3Only policy
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Current cache size in bytes
    pub size: usize,
    /// Maximum cache capacity in bytes
    pub capacity: usize,
    /// Cache hit rate (0.0 - 1.0)
    pub hit_rate: f64,
    /// Total number of cache hits
    pub hits: u64,
    /// Total number of cache misses
    pub misses: u64,
}

/// Storage backend integrating WAL, Snapshotter, and ObjectStore
///
/// Provides unified vector storage with three tiering policies:
/// - **Memory:** Fastest, ephemeral (dev/test)
/// - **MemoryS3:** Fast + durable (production)
/// - **S3Only:** Cost-optimized (cold storage)
///
/// # Examples
///
/// ## Memory Policy (Dev/Test)
///
/// ```rust,no_run
/// use akidb_storage::{StorageBackend, StorageConfig, TieringPolicy};
///
/// #[tokio::main]
/// async fn main() -> akidb_core::CoreResult<()> {
///     let config = StorageConfig::memory("./test.wal");
///     let backend = StorageBackend::new(config).await?;
///
///     // Insert, query, delete operations...
///     Ok(())
/// }
/// ```
///
/// ## MemoryS3 Policy (Production)
///
/// ```rust,no_run
/// use akidb_storage::{StorageBackend, StorageConfig};
///
/// #[tokio::main]
/// async fn main() -> akidb_core::CoreResult<()> {
///     let config = StorageConfig::memory_s3(
///         "./collection.wal",
///         "./snapshots",
///         "akidb-vectors".to_string(),
///     );
///
///     let backend = StorageBackend::new(config).await?;
///     Ok(())
/// }
/// ```
pub struct StorageBackend {
    /// FIX BUG #16: Store collection_id to use in WAL entries and S3 keys
    /// Without this, every operation generates a random CollectionId
    collection_id: CollectionId,

    config: StorageConfig,
    wal: Arc<FileWAL>,
    snapshotter: Arc<JsonSnapshotter>,
    object_store: Option<Arc<dyn ObjectStore>>,

    // In-memory vector storage (Memory and MemoryS3 policies)
    vector_store: Arc<RwLock<HashMap<DocumentId, VectorDocument>>>,

    // For S3Only policy: LRU cache of recently accessed vectors
    pub(crate) vector_cache: Option<Arc<RwLock<lru::LruCache<DocumentId, VectorDocument>>>>,

    // Metrics
    metrics: Arc<RwLock<StorageMetrics>>,

    // S3 upload coordination for MemoryS3 policy
    s3_upload_queue: Arc<RwLock<VecDeque<S3UploadTask>>>,
    s3_upload_notify: Arc<Notify>,
    s3_uploader_handle: Option<JoinHandle<()>>,

    // Day 3: Background compaction worker
    compaction_notify: Arc<Notify>,
    compaction_handle: Option<JoinHandle<()>>,

    // Day 4: Retry logic
    retry_queue: Arc<RwLock<VecDeque<S3RetryTask>>>,
    #[allow(dead_code)] // Used in retry_worker background task
    retry_notify: Arc<Notify>,
    retry_handle: Option<JoinHandle<()>>,
    dead_letter_queue: Arc<DeadLetterQueue>,
    #[allow(dead_code)] // Used in retry_worker background task
    retry_config: RetryConfig,

    // Phase 7 Week 1: Circuit breaker
    circuit_breaker: Option<Arc<crate::circuit_breaker::CircuitBreaker>>,

    // Phase 7 Week 1 Days 3-4: DLQ cleanup worker
    dlq_cleanup_handle: Option<JoinHandle<()>>,
}

/// Classify S3 error as transient (retry) or permanent (DLQ).
///
/// **Transient Errors (retry):**
/// - 5xx (server errors)
/// - 429 (rate limit)
/// - Network timeouts
/// - Connection resets
///
/// **Permanent Errors (DLQ):**
/// - 4xx (client errors, except 429)
/// - Invalid credentials
/// - Bucket not found
fn classify_s3_error(error: &str) -> ErrorClass {
    // Check for HTTP status codes
    if error.contains("500") || error.contains("503") || error.contains("504") {
        return ErrorClass::Transient;
    }

    if error.contains("429") {
        // Too Many Requests
        return ErrorClass::Transient;
    }

    if error.contains("timeout") || error.contains("connection reset") {
        return ErrorClass::Transient;
    }

    if error.contains("403") || error.contains("404") || error.contains("400") {
        return ErrorClass::Permanent;
    }

    // Default: treat as transient (safer)
    ErrorClass::Transient
}

/// Calculate exponential backoff delay.
///
/// Formula: min(base * 2^attempt, max_backoff)
///
/// # Bug Fix (Bug #10 - ULTRATHINK)
///
/// Clamps attempt to max 30 to prevent integer overflow.
/// - 2^30 = 1,073,741,824 seconds = ~34 years
/// - 2^64 would overflow u64
/// Uses saturating arithmetic for safety.
pub(crate) fn calculate_backoff(
    attempt: u32,
    base: std::time::Duration,
    max: std::time::Duration,
) -> std::time::Duration {
    // Clamp attempt to prevent overflow (2^30 = 1B seconds = 34 years is reasonable max)
    const MAX_ATTEMPT: u32 = 30;
    let clamped_attempt = attempt.min(MAX_ATTEMPT);

    // Use saturating arithmetic to prevent overflow
    let power = 2u64.saturating_pow(clamped_attempt);
    let exponential = base.as_secs().saturating_mul(power);

    std::time::Duration::from_secs(exponential.min(max.as_secs()))
}

/// Move failed task to Dead Letter Queue.
async fn move_to_dlq(
    task: S3RetryTask,
    dlq: &Arc<DeadLetterQueue>,
    metrics: &Arc<RwLock<StorageMetrics>>,
    ttl_seconds: i64,
) {
    // Serialize document data
    let data = match serde_json::to_vec(&task.task.doc) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to serialize document for DLQ: {}", e);
            return;
        }
    };

    let entry = crate::dlq::DLQEntry::new(
        task.task.doc.doc_id.clone(),
        task.task.collection_id,
        task.last_error,
        data,
        ttl_seconds,
    );

    if let Err(e) = dlq.add_entry(entry).await {
        tracing::error!("Failed to add entry to DLQ: {}", e);
    }

    let mut m = metrics.write();
    m.s3_permanent_failures += 1;
    m.dlq_size = dlq.size();
}

impl StorageBackend {
    /// Create new storage backend
    ///
    /// This will:
    /// 1. Validate configuration
    /// 2. Open WAL (create if doesn't exist)
    /// 3. Initialize snapshotter
    /// 4. Connect to S3 (if policy requires it)
    /// 5. Create LRU cache (for S3Only policy)
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Configuration validation fails
    /// - WAL cannot be opened
    /// - S3 connection fails (for MemoryS3/S3Only)
    /// - Required directories don't exist
    pub async fn new(config: StorageConfig) -> CoreResult<Self> {
        // Validate configuration
        config.validate()?;

        // Ensure WAL parent directory exists
        if let Some(parent) = config.wal_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    akidb_core::CoreError::StorageError(format!(
                        "Failed to create WAL directory: {}",
                        e
                    ))
                })?;
            }
        }

        // Open WAL
        let wal_config = FileWALConfig {
            max_file_size_bytes: 100 * 1024 * 1024, // 100MB
            sync_on_write: true,
            ..Default::default()
        };
        let wal = Arc::new(FileWAL::new(&config.wal_path, wal_config).await?);

        // Create object store (if needed)
        let object_store = if config.tiering_policy.requires_s3() {
            let bucket = config.s3_bucket.as_ref().unwrap();

            // Check if bucket is a file:// URL (for testing with LocalObjectStore)
            let store: Arc<dyn ObjectStore> = if bucket.starts_with("file://") {
                let path = bucket.strip_prefix("file://").unwrap();
                Arc::new(LocalObjectStore::new(path).await?)
            } else if let (Some(endpoint), Some(access_key), Some(secret_key)) = (
                &config.s3_endpoint,
                &config.s3_access_key,
                &config.s3_secret_key,
            ) {
                // MinIO or custom S3 endpoint
                let s3_config =
                    S3Config::custom(bucket, &config.s3_region, endpoint, access_key, secret_key);
                Arc::new(S3ObjectStore::new(s3_config).await?)
            } else {
                // Standard AWS S3
                let s3_config = S3Config::aws(bucket, &config.s3_region);
                Arc::new(S3ObjectStore::new(s3_config).await?)
            };

            Some(store)
        } else {
            None
        };

        // Create snapshotter
        let snapshotter_store: Arc<dyn ObjectStore> = if let Some(store) = &object_store {
            store.clone()
        } else {
            // Use local filesystem for snapshots (Memory policy)
            Arc::new(LocalObjectStore::new(&config.snapshot_dir).await?)
        };

        let compression = match config.compression {
            crate::tiering::CompressionType::None => crate::snapshotter::CompressionCodec::None,
            crate::tiering::CompressionType::Snappy => crate::snapshotter::CompressionCodec::Snappy,
            crate::tiering::CompressionType::Zstd => crate::snapshotter::CompressionCodec::Zstd,
            crate::tiering::CompressionType::Lz4 => crate::snapshotter::CompressionCodec::Lz4,
        };

        let snapshotter = Arc::new(JsonSnapshotter::new(snapshotter_store, compression));

        // Create vector cache (for S3Only policy)
        let vector_cache = if config.tiering_policy == TieringPolicy::S3Only {
            Some(Arc::new(RwLock::new(lru::LruCache::new(
                NonZeroUsize::new(config.cache_size).unwrap_or(NonZeroUsize::new(10_000).unwrap()),
            ))))
        } else {
            None
        };

        // Create S3 upload queue
        let s3_upload_queue = Arc::new(RwLock::new(VecDeque::new()));
        let s3_upload_notify = Arc::new(Notify::new());

        // Create compaction notification channel
        let compaction_notify = Arc::new(Notify::new());

        // Day 4: Create retry queue and DLQ
        let retry_queue = Arc::new(RwLock::new(VecDeque::new()));
        let retry_notify = Arc::new(Notify::new());
        let dead_letter_queue = Arc::new(DeadLetterQueue::new(config.dlq_config.clone()));
        let retry_config = config.retry_config.clone().unwrap_or_default();

        // Phase 7 Week 1 Days 3-4: Load DLQ from disk if persistence file exists
        if config.dlq_config.persistence_path.exists() {
            if let Err(e) = dead_letter_queue.load_from_disk().await {
                tracing::warn!("Failed to load DLQ from disk: {}", e);
            }
        }

        // Phase 7 Week 1: Create circuit breaker
        let circuit_breaker = if config.circuit_breaker_enabled {
            config
                .circuit_breaker_config
                .as_ref()
                .map(|cfg| Arc::new(crate::circuit_breaker::CircuitBreaker::new(cfg.clone())))
        } else {
            None
        };

        let wal_ref = wal.clone();
        let snapshotter_ref = snapshotter.clone();
        let metrics_ref = Arc::new(RwLock::new(StorageMetrics::default()));
        let vector_store_ref = Arc::new(RwLock::new(HashMap::new()));

        // FIX BUG #16: Extract collection_id from config BEFORE consuming it
        let collection_id = config.collection_id;

        let mut backend = Self {
            collection_id, // Store the real collection_id instead of generating random ones
            config: config.clone(),
            wal,
            snapshotter,
            object_store: object_store.clone(),
            vector_store: vector_store_ref.clone(),
            vector_cache,
            metrics: metrics_ref.clone(),
            s3_upload_queue: s3_upload_queue.clone(),
            s3_upload_notify: s3_upload_notify.clone(),
            s3_uploader_handle: None,
            compaction_notify: compaction_notify.clone(),
            compaction_handle: None,
            retry_queue: retry_queue.clone(),
            retry_notify: retry_notify.clone(),
            retry_handle: None,
            dead_letter_queue: dead_letter_queue.clone(),
            retry_config: retry_config.clone(),
            circuit_breaker: circuit_breaker.clone(),
            dlq_cleanup_handle: None,
        };

        // Recover from WAL on startup
        backend.recover().await?;

        // Spawn S3 uploader background task (only for MemoryS3 policy)
        if matches!(config.tiering_policy, TieringPolicy::MemoryS3) {
            if let Some(store) = object_store.clone() {
                let queue = s3_upload_queue.clone();
                let notify = s3_upload_notify.clone();
                let retry_q = retry_queue.clone();
                let retry_n = retry_notify.clone();
                let metrics = metrics_ref.clone();

                backend.s3_uploader_handle = Some(tokio::spawn(async move {
                    Self::s3_uploader_worker(queue, notify, store, retry_q, retry_n, metrics).await;
                }));

                tracing::info!("S3 uploader background worker started for MemoryS3 policy");
            }
        }

        // Spawn retry worker (for MemoryS3 policy)
        if matches!(config.tiering_policy, TieringPolicy::MemoryS3) {
            if let Some(store) = object_store.clone() {
                let retry_q = retry_queue.clone();
                let retry_n = retry_notify.clone();
                let dlq = dead_letter_queue.clone();
                let metrics = metrics_ref.clone();
                let retry_cfg = retry_config.clone();
                let cb = circuit_breaker.clone();

                backend.retry_handle = Some(tokio::spawn(async move {
                    Self::retry_worker(retry_q, retry_n, store, dlq, metrics, retry_cfg, cb).await;
                }));

                tracing::info!("S3 retry worker started for MemoryS3 policy");
            }
        }

        // Spawn background compaction worker (if enabled)
        if config.enable_background_compaction {
            let wal_clone = wal_ref;
            let snapshotter_clone = snapshotter_ref;
            let vector_store_clone = vector_store_ref;
            let notify_clone = compaction_notify;
            let metrics_clone = metrics_ref;
            let compaction_config = config.compaction_config.clone();
            // FIX BUG #16: Clone collection_id for background worker
            let coll_id = collection_id;

            backend.compaction_handle = Some(tokio::spawn(async move {
                Self::compaction_worker(
                    wal_clone,
                    snapshotter_clone,
                    vector_store_clone,
                    notify_clone,
                    metrics_clone,
                    compaction_config,
                    coll_id, // FIX BUG #16: Pass collection_id
                )
                .await;
            }));

            tracing::info!("Background compaction worker started");
        }

        // Phase 7 Week 1 Days 3-4: Spawn DLQ cleanup worker
        {
            let dlq_clone = dead_letter_queue.clone();
            let cleanup_interval =
                std::time::Duration::from_secs(config.dlq_config.cleanup_interval_seconds);

            backend.dlq_cleanup_handle = Some(tokio::spawn(async move {
                Self::dlq_cleanup_worker(dlq_clone, cleanup_interval).await;
            }));

            tracing::info!(
                "DLQ cleanup worker started (interval: {:?})",
                cleanup_interval
            );
        }

        Ok(backend)
    }

    /// Create StorageBackend with mock S3 (test-only constructor).
    ///
    /// This allows injecting a mock ObjectStore for testing failure scenarios
    /// without real AWS/MinIO dependencies.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use akidb_storage::object_store::{MockS3ObjectStore, MockFailure};
    ///
    /// let mock_s3 = MockS3ObjectStore::new_with_failures(vec![
    ///     MockFailure::Transient("500 Internal Server Error"),
    ///     MockFailure::Ok,
    /// ]);
    ///
    /// let backend = StorageBackend::new_with_mock_s3(
    ///     StorageConfig::default(),
    ///     Arc::new(mock_s3)
    /// ).await.unwrap();
    /// ```
    pub async fn new_with_mock_s3(
        config: StorageConfig,
        mock_s3: Arc<dyn ObjectStore>,
    ) -> CoreResult<Self> {
        // Validate configuration
        config.validate()?;

        // Ensure WAL parent directory exists
        if let Some(parent) = config.wal_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    akidb_core::CoreError::StorageError(format!(
                        "Failed to create WAL directory: {}",
                        e
                    ))
                })?;
            }
        }

        // Open WAL
        let wal_config = FileWALConfig {
            max_file_size_bytes: 100 * 1024 * 1024, // 100MB
            sync_on_write: true,
            ..Default::default()
        };
        let wal = Arc::new(FileWAL::new(&config.wal_path, wal_config).await?);

        // Use injected mock S3
        let object_store = Some(mock_s3.clone());

        // Create snapshotter with mock S3
        let compression = match config.compression {
            crate::tiering::CompressionType::None => crate::snapshotter::CompressionCodec::None,
            crate::tiering::CompressionType::Snappy => crate::snapshotter::CompressionCodec::Snappy,
            crate::tiering::CompressionType::Zstd => crate::snapshotter::CompressionCodec::Zstd,
            crate::tiering::CompressionType::Lz4 => crate::snapshotter::CompressionCodec::Lz4,
        };

        let snapshotter = Arc::new(JsonSnapshotter::new(mock_s3.clone(), compression));

        // Create vector cache (for S3Only policy)
        let vector_cache = if config.tiering_policy == TieringPolicy::S3Only {
            Some(Arc::new(RwLock::new(lru::LruCache::new(
                NonZeroUsize::new(config.cache_size).unwrap_or(NonZeroUsize::new(10_000).unwrap()),
            ))))
        } else {
            None
        };

        // Create S3 upload queue
        let s3_upload_queue = Arc::new(RwLock::new(VecDeque::new()));
        let s3_upload_notify = Arc::new(Notify::new());

        // Create compaction notification channel
        let compaction_notify = Arc::new(Notify::new());

        // Create retry queue and DLQ
        let retry_queue = Arc::new(RwLock::new(VecDeque::new()));
        let retry_notify = Arc::new(Notify::new());
        let dead_letter_queue = Arc::new(DeadLetterQueue::new(config.dlq_config.clone()));
        let retry_config = config.retry_config.clone().unwrap_or_default();

        // Create circuit breaker
        let circuit_breaker = if config.circuit_breaker_enabled {
            config
                .circuit_breaker_config
                .as_ref()
                .map(|cfg| Arc::new(crate::circuit_breaker::CircuitBreaker::new(cfg.clone())))
        } else {
            None
        };

        let wal_ref = wal.clone();
        let snapshotter_ref = snapshotter.clone();
        let metrics_ref = Arc::new(RwLock::new(StorageMetrics::default()));
        let vector_store_ref = Arc::new(RwLock::new(HashMap::new()));

        // FIX BUG #16: Extract collection_id from config BEFORE consuming it
        let collection_id = config.collection_id;

        let mut backend = Self {
            collection_id, // Store the real collection_id instead of generating random ones
            config: config.clone(),
            wal,
            snapshotter,
            object_store: object_store.clone(),
            vector_store: vector_store_ref.clone(),
            vector_cache,
            metrics: metrics_ref.clone(),
            s3_upload_queue: s3_upload_queue.clone(),
            s3_upload_notify: s3_upload_notify.clone(),
            s3_uploader_handle: None,
            compaction_notify: compaction_notify.clone(),
            compaction_handle: None,
            retry_queue: retry_queue.clone(),
            retry_notify: retry_notify.clone(),
            retry_handle: None,
            dead_letter_queue: dead_letter_queue.clone(),
            retry_config: retry_config.clone(),
            circuit_breaker: circuit_breaker.clone(),
            dlq_cleanup_handle: None,
        };

        // Recover from WAL on startup
        backend.recover().await?;

        // Spawn S3 uploader background task (only for MemoryS3 policy)
        if matches!(config.tiering_policy, TieringPolicy::MemoryS3) {
            if let Some(store) = object_store.clone() {
                let queue = s3_upload_queue.clone();
                let notify = s3_upload_notify.clone();
                let retry_q = retry_queue.clone();
                let retry_n = retry_notify.clone();
                let metrics = metrics_ref.clone();

                backend.s3_uploader_handle = Some(tokio::spawn(async move {
                    Self::s3_uploader_worker(queue, notify, store, retry_q, retry_n, metrics).await;
                }));

                tracing::info!("S3 uploader background worker started (with mock S3)");
            }
        }

        // Spawn retry worker (for MemoryS3 policy)
        if matches!(config.tiering_policy, TieringPolicy::MemoryS3) {
            if let Some(store) = object_store.clone() {
                let retry_q = retry_queue.clone();
                let retry_n = retry_notify.clone();
                let dlq = dead_letter_queue.clone();
                let metrics = metrics_ref.clone();
                let retry_cfg = retry_config.clone();
                let cb = circuit_breaker.clone();

                backend.retry_handle = Some(tokio::spawn(async move {
                    Self::retry_worker(retry_q, retry_n, store, dlq, metrics, retry_cfg, cb).await;
                }));

                tracing::info!("S3 retry worker started (with mock S3)");
            }
        }

        // Spawn compaction worker
        if config.enable_background_compaction {
            let wal_clone = wal_ref;
            let snapshotter_clone = snapshotter_ref;
            let vector_store_clone = vector_store_ref;
            let notify_clone = compaction_notify;
            let metrics_clone = metrics_ref;
            let compaction_config = config.compaction_config.clone();
            // FIX BUG #16: Clone collection_id for background worker
            let coll_id = collection_id;

            backend.compaction_handle = Some(tokio::spawn(async move {
                Self::compaction_worker(
                    wal_clone,
                    snapshotter_clone,
                    vector_store_clone,
                    notify_clone,
                    metrics_clone,
                    compaction_config,
                    coll_id, // FIX BUG #16: Pass collection_id
                )
                .await;
            }));

            tracing::info!("Compaction worker started (with mock S3)");
        }

        // Spawn DLQ cleanup worker
        {
            let dlq_clone = dead_letter_queue.clone();
            let cleanup_interval =
                std::time::Duration::from_secs(config.dlq_config.cleanup_interval_seconds);

            backend.dlq_cleanup_handle = Some(tokio::spawn(async move {
                Self::dlq_cleanup_worker(dlq_clone, cleanup_interval).await;
            }));

            tracing::info!("DLQ cleanup worker started (with mock S3)");
        }

        Ok(backend)
    }

    /// DLQ cleanup worker - periodically removes expired entries and persists to disk
    ///
    /// **Behavior:**
    /// - Runs on interval (default: 1 hour)
    /// - Removes expired entries based on TTL
    /// - Persists DLQ to disk after cleanup
    /// - Updates metrics
    async fn dlq_cleanup_worker(dlq: Arc<DeadLetterQueue>, interval: std::time::Duration) {
        tracing::info!("DLQ cleanup worker started");

        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;

            // Cleanup expired entries
            if let Err(e) = dlq.cleanup_expired().await {
                tracing::error!("DLQ cleanup failed: {}", e);
            }

            // Persist after cleanup
            if let Err(e) = dlq.persist().await {
                tracing::error!("DLQ persistence failed: {}", e);
            }
        }
    }

    /// Background worker that retries failed S3 uploads.
    ///
    /// **Behavior:**
    /// - Polls retry queue every 1 second
    /// - Retries tasks whose `next_retry_at` has passed
    /// - Uses exponential backoff (1s → 2s → 4s → ... → 64s)
    /// - Moves to DLQ after max retries exceeded
    async fn retry_worker(
        retry_queue: Arc<RwLock<VecDeque<S3RetryTask>>>,
        retry_notify: Arc<Notify>,
        object_store: Arc<dyn ObjectStore>,
        dead_letter_queue: Arc<DeadLetterQueue>,
        metrics: Arc<RwLock<StorageMetrics>>,
        retry_config: RetryConfig,
        circuit_breaker: Option<Arc<crate::circuit_breaker::CircuitBreaker>>,
    ) {
        tracing::info!("S3 retry worker started");

        loop {
            // Wait for notification or 1-second timeout
            tokio::select! {
                _ = retry_notify.notified() => {}
                _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {}
            }

            // Find tasks ready for retry
            let now = std::time::Instant::now();
            let ready_tasks: Vec<S3RetryTask> = {
                let mut queue = retry_queue.write();

                // Separate ready and not-ready tasks
                let (ready, not_ready): (Vec<_>, Vec<_>) =
                    queue.drain(..).partition(|task| task.next_retry_at <= now);

                // Put not-ready tasks back
                queue.extend(not_ready);

                ready
            };

            for mut task in ready_tasks {
                // Phase 7 Week 1: Check circuit breaker before retrying
                if let Some(cb) = &circuit_breaker {
                    if !cb.should_allow_request() {
                        tracing::debug!(
                            "Circuit breaker open, skipping retry for {}",
                            task.task.doc.doc_id
                        );

                        // Re-enqueue for later (circuit may close)
                        task.next_retry_at =
                            std::time::Instant::now() + std::time::Duration::from_secs(10);
                        retry_queue.write().push_back(task);
                        continue;
                    }
                }

                let key = format!(
                    "vectors/{}/{}",
                    task.task.collection_id, task.task.doc.doc_id
                );
                let data = match serde_json::to_vec(&task.task.doc) {
                    Ok(d) => d,
                    Err(e) => {
                        tracing::error!("Failed to serialize document: {}", e);
                        continue;
                    }
                };

                // Attempt S3 upload
                match object_store.put(&key, Bytes::from(data)).await {
                    Ok(_) => {
                        tracing::info!(
                            "Retry successful after {} attempts: {}",
                            task.attempt,
                            task.task.doc.doc_id
                        );

                        metrics.write().s3_retries += 1;

                        // Phase 7 Week 1: Record success with circuit breaker
                        if let Some(cb) = &circuit_breaker {
                            cb.record_result(true);
                        }

                        // Task succeeded, don't re-enqueue
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        tracing::warn!(
                            "Retry attempt {} failed for {}: {}",
                            task.attempt + 1,
                            task.task.doc.doc_id,
                            error_msg
                        );

                        // Phase 7 Week 1: Record failure with circuit breaker
                        if let Some(cb) = &circuit_breaker {
                            cb.record_result(false);
                        }

                        task.attempt += 1;
                        task.last_error = error_msg.clone();

                        // Check error classification
                        if classify_s3_error(&error_msg) == ErrorClass::Permanent {
                            tracing::error!(
                                "Permanent error detected, moving to DLQ: {}",
                                error_msg
                            );
                            move_to_dlq(task, &dead_letter_queue, &metrics, 604_800).await;
                            continue;
                        }

                        // Check retry limit
                        if task.attempt >= retry_config.max_retries {
                            tracing::error!(
                                "Max retries exceeded for {}, moving to DLQ",
                                task.task.doc.doc_id
                            );
                            move_to_dlq(task, &dead_letter_queue, &metrics, 604_800).await;
                            continue;
                        }

                        // Calculate exponential backoff
                        let backoff = calculate_backoff(
                            task.attempt,
                            retry_config.base_backoff,
                            retry_config.max_backoff,
                        );
                        task.next_retry_at = std::time::Instant::now() + backoff;

                        // Re-enqueue for next retry
                        retry_queue.write().push_back(task);
                    }
                }
            }
        }
    }

    /// Background worker for S3 uploads (MemoryS3 policy)
    ///
    /// This worker runs in the background, draining upload tasks from the queue
    /// and uploading them to S3 in batches of up to 10 documents at a time.
    ///
    /// The worker is automatically spawned for MemoryS3 policy during `new()`.
    async fn s3_uploader_worker(
        queue: Arc<RwLock<VecDeque<S3UploadTask>>>,
        notify: Arc<Notify>,
        object_store: Arc<dyn ObjectStore>,
        retry_queue: Arc<RwLock<VecDeque<S3RetryTask>>>,
        retry_notify: Arc<Notify>,
        metrics: Arc<RwLock<StorageMetrics>>,
    ) {
        tracing::info!("S3 uploader worker started");

        loop {
            // Wait for notification or timeout (max 1 second idle)
            tokio::select! {
                () = notify.notified() => {
                    // New upload task available
                }
                () = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {
                    // Periodic check
                }
            }

            // Drain up to 10 tasks from queue
            let batch = {
                let mut q = queue.write();
                let batch_size = std::cmp::min(q.len(), 10);
                q.drain(..batch_size).collect::<Vec<_>>()
            };

            if batch.is_empty() {
                continue;
            }

            tracing::debug!("S3 uploader processing {} tasks", batch.len());

            // Upload each task
            for task in batch {
                let key = format!("vectors/{}/{}", task.collection_id, task.doc.doc_id);

                match serde_json::to_vec(&task.doc) {
                    Ok(data) => {
                        match object_store.put(&key, Bytes::from(data)).await {
                            Ok(()) => {
                                tracing::trace!("S3 upload succeeded: {}", key);
                                metrics.write().s3_uploads += 1;
                            }
                            Err(e) => {
                                // NEW: Add to retry queue instead of panicking
                                tracing::warn!("S3 upload failed, enqueueing for retry: {}", e);

                                let retry_task = S3RetryTask {
                                    task: task.clone(),
                                    attempt: 0,
                                    next_retry_at: std::time::Instant::now()
                                        + std::time::Duration::from_secs(1),
                                    last_error: e.to_string(),
                                };

                                retry_queue.write().push_back(retry_task);
                                retry_notify.notify_one(); // Wake retry worker
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to serialize vector {}: {}", task.doc.doc_id, e);
                    }
                }
            }
        }
    }

    /// Background worker that performs compaction asynchronously.
    ///
    /// **Trigger Conditions:**
    /// - Manual notification via `compaction_notify.notify_one()`
    /// - Periodic check every 5 minutes (fallback)
    ///
    /// **Compaction Logic:**
    /// - Check if compaction needed based on metrics
    /// - Call `perform_compaction()` to create snapshot and checkpoint WAL
    /// - Update metrics
    async fn compaction_worker(
        wal: Arc<FileWAL>,
        snapshotter: Arc<JsonSnapshotter>,
        vector_store: Arc<RwLock<HashMap<DocumentId, VectorDocument>>>,
        notify: Arc<Notify>,
        metrics: Arc<RwLock<StorageMetrics>>,
        compaction_config: crate::tiering::CompactionConfig,
        collection_id: CollectionId, // FIX BUG #16: Pass collection_id for snapshots
    ) {
        use std::time::Duration;

        tracing::info!("Compaction worker started");

        loop {
            // Wait for notification OR 5-minute timeout
            tokio::select! {
                _ = notify.notified() => {
                    tracing::debug!("Compaction triggered by notification");
                }
                _ = tokio::time::sleep(Duration::from_secs(300)) => {
                    tracing::debug!("Compaction triggered by periodic timer");
                }
            }

            // Check if compaction is actually needed
            let should_compact = {
                let m = metrics.read();
                m.wal_size_bytes >= compaction_config.threshold_bytes
                    || m.inserts >= compaction_config.threshold_ops
            };

            if !should_compact {
                tracing::debug!("Compaction check: not needed yet");
                continue;
            }

            // Perform compaction (non-blocking to insert path)
            tracing::info!("Starting background compaction");
            let start = std::time::Instant::now();

            // FIX BUG #16: Pass collection_id to perform_compaction
            match Self::perform_compaction(&wal, &snapshotter, &vector_store, collection_id).await {
                Ok(()) => {
                    let elapsed = start.elapsed();
                    tracing::info!("Compaction complete in {:?}", elapsed);

                    // FIX BUG #18: Reset insert counter after compaction
                    // Without this, once inserts >= threshold, compaction runs continuously every 1s
                    let mut m = metrics.write();
                    m.compactions += 1;
                    m.last_snapshot_at = Some(Utc::now());
                    m.inserts = 0; // Reset counter to prevent continuous compaction
                }
                Err(e) => {
                    tracing::error!("Compaction failed: {}", e);
                    // Continue running (don't crash worker)
                }
            }
        }
    }

    /// Helper function to perform actual compaction.
    async fn perform_compaction(
        wal: &Arc<FileWAL>,
        snapshotter: &Arc<JsonSnapshotter>,
        vector_store: &Arc<RwLock<HashMap<DocumentId, VectorDocument>>>,
        collection_id: CollectionId, // FIX BUG #16: Use real collection_id
    ) -> CoreResult<()> {
        // 1. Collect current vector state
        let vectors: Vec<VectorDocument> = vector_store.read().values().cloned().collect();

        // 2. Create snapshot
        // FIX BUG #16: Use the real collection_id passed as parameter
        snapshotter.create_snapshot(collection_id, vectors).await?;

        // 3. Create checkpoint in WAL
        let current_lsn = wal.current_lsn().await?;
        let checkpoint_entry = LogEntry::Checkpoint {
            lsn: current_lsn,
            timestamp: Utc::now(),
        };

        wal.append(checkpoint_entry).await?;
        wal.flush().await?;

        // 4. Mark checkpoint (this allows WAL to discard old entries)
        wal.checkpoint(current_lsn).await?;

        Ok(())
    }

    /// Insert a vector document
    ///
    /// Behavior depends on tiering policy:
    /// - **Memory:** WAL + HashMap
    /// - **MemoryS3:** WAL + HashMap + async S3 upload
    /// - **S3Only:** WAL + immediate S3 upload + cache
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - WAL append fails
    /// - S3 upload fails (S3Only policy only, MemoryS3 fails silently)
    pub async fn insert(&self, doc: VectorDocument) -> CoreResult<()> {
        // 1. Append to WAL (all policies)
        // FIX BUG #16: Use real collection_id instead of generating random ones
        let log_entry = LogEntry::Upsert {
            collection_id: self.collection_id, // Now using the real collection_id!
            doc_id: doc.doc_id.clone(),
            vector: doc.vector.clone(),
            external_id: doc.external_id.clone(),
            metadata: doc.metadata.clone(),
            timestamp: doc.inserted_at,
        };

        // FIX BUG #6: Track WAL size for compaction threshold
        // Estimate entry size: UUID (16) + vector (dim * 4) + metadata overhead (~100)
        let entry_size_bytes = 16 + (doc.vector.len() * 4) + 100
            + doc.external_id.as_ref().map_or(0, |s| s.len())
            + doc.metadata.as_ref().map_or(0, |_| 200); // JSON metadata estimate

        self.wal.append(log_entry).await?;
        self.wal.flush().await?;

        // Track WAL size
        self.metrics.write().wal_size_bytes += entry_size_bytes as u64;

        // 2. Handle tiering policy
        match self.config.tiering_policy {
            TieringPolicy::Memory => {
                // Store in HashMap
                self.vector_store.write().insert(doc.doc_id.clone(), doc);
            }

            TieringPolicy::MemoryS3 => {
                // Store in HashMap
                self.vector_store
                    .write()
                    .insert(doc.doc_id.clone(), doc.clone());

                // Enqueue S3 upload task (non-blocking)
                // FIX BUG #16: Use real collection_id
                let collection_id = self.collection_id;
                self.s3_upload_queue
                    .write()
                    .push_back(S3UploadTask { collection_id, doc });
                self.s3_upload_notify.notify_one();

                tracing::trace!("Enqueued S3 upload task");
            }

            TieringPolicy::S3Only => {
                // S3Only: Upload to S3 FIRST (source of truth), then cache
                // TODO: Pass real collection_id from caller, for now use doc_id as key
                let key = format!("vectors/{}.json", doc.doc_id);
                let data = serde_json::to_vec(&doc)
                    .map_err(|e| akidb_core::CoreError::StorageError(e.to_string()))?;

                // BLOCKING S3 upload (S3 is source of truth)
                if let Some(store) = &self.object_store {
                    store.put(&key, Bytes::from(data)).await?;

                    tracing::debug!("S3Only: Uploaded {} to S3", doc.doc_id);

                    // Cache in LRU for fast reads
                    if let Some(cache) = &self.vector_cache {
                        cache.write().put(doc.doc_id.clone(), doc);
                    }

                    // Update metrics
                    self.metrics.write().s3_uploads += 1;
                }
            }
        }

        // Update metrics
        self.metrics.write().inserts += 1;

        Ok(())
    }

    /// Get a vector document by ID
    ///
    /// # Errors
    ///
    /// Returns error if document not found or S3 download fails
    pub async fn get(&self, doc_id: &DocumentId) -> CoreResult<Option<VectorDocument>> {
        // FIX BUG #19: Increment queries counter for monitoring/dashboards
        // Without this, Prometheus/Grafana dashboards show 0 queries forever
        {
            let mut m = self.metrics.write();
            m.queries += 1;
        }

        match self.config.tiering_policy {
            TieringPolicy::Memory | TieringPolicy::MemoryS3 => {
                // Get from HashMap
                Ok(self.vector_store.read().get(doc_id).cloned())
            }

            TieringPolicy::S3Only => {
                // S3Only: Check LRU cache first (fast path)

                // Try cache hit
                {
                    if let Some(cache) = &self.vector_cache {
                        if let Some(doc) = cache.write().get(doc_id) {
                            tracing::trace!("S3Only: Cache HIT for {}", doc_id);

                            // Update metrics
                            self.metrics.write().cache_hits += 1;

                            return Ok(Some(doc.clone()));
                        }
                    }
                }

                // Cache MISS: Download from S3 (slow path)
                tracing::debug!("S3Only: Cache MISS for {}, downloading from S3", doc_id);

                // TODO: Pass real collection_id from caller, for now use doc_id as key
                let key = format!("vectors/{}.json", doc_id);

                if let Some(store) = &self.object_store {
                    match store.get(&key).await {
                        Ok(data) => {
                            // Deserialize vector
                            let doc: VectorDocument = serde_json::from_slice(&data)
                                .map_err(|e| akidb_core::CoreError::StorageError(e.to_string()))?;

                            // Populate cache for future reads
                            if let Some(cache) = &self.vector_cache {
                                cache.write().put(doc_id.clone(), doc.clone());
                            }

                            // Update metrics
                            {
                                let mut m = self.metrics.write();
                                m.cache_misses += 1;
                                m.s3_downloads += 1;
                            }

                            tracing::trace!("S3Only: Downloaded and cached {}", doc_id);

                            Ok(Some(doc))
                        }
                        Err(akidb_core::CoreError::NotFound { .. }) => {
                            // Vector doesn't exist in S3
                            self.metrics.write().cache_misses += 1;
                            Ok(None)
                        }
                        Err(e) => {
                            // S3 error (network, permissions, etc.)
                            tracing::error!("S3 download failed for {}: {}", doc_id, e);
                            Err(e)
                        }
                    }
                } else {
                    Ok(None)
                }
            }
        }
    }

    /// Delete a vector document
    ///
    /// # Errors
    ///
    /// Returns error if WAL append fails or S3 delete fails
    pub async fn delete(&self, doc_id: &DocumentId) -> CoreResult<()> {
        // 1. Append to WAL
        // FIX BUG #16: Use real collection_id
        let log_entry = LogEntry::Delete {
            collection_id: self.collection_id, // Now using the real collection_id!
            doc_id: doc_id.clone(),
            timestamp: Utc::now(),
        };

        self.wal.append(log_entry).await?;
        self.wal.flush().await?;

        // 2. Delete from storage
        match self.config.tiering_policy {
            TieringPolicy::Memory | TieringPolicy::MemoryS3 => {
                self.vector_store.write().remove(doc_id);
            }

            TieringPolicy::S3Only => {
                // Delete from S3
                if let Some(store) = &self.object_store {
                    // FIX BUG #16: Use real collection_id in S3 key path
                    let key = format!("vectors/{}/{}.json", self.collection_id, doc_id);
                    let _ = store.delete(&key).await; // Ignore errors (idempotent)
                }

                // Delete from cache
                if let Some(cache) = &self.vector_cache {
                    cache.write().pop(doc_id);
                }
            }
        }

        // Update metrics
        self.metrics.write().deletes += 1;

        Ok(())
    }

    /// Get count of vectors in storage
    #[must_use]
    pub fn count(&self) -> usize {
        match self.config.tiering_policy {
            TieringPolicy::Memory | TieringPolicy::MemoryS3 => self.vector_store.read().len(),
            TieringPolicy::S3Only => {
                // For S3Only, count is not available without listing all objects
                // Return cache size as approximation
                self.vector_cache
                    .as_ref()
                    .map_or(0, |cache| cache.read().len())
            }
        }
    }

    /// Get all vectors from storage
    ///
    /// Returns a vector of all documents currently in storage.
    /// For Memory/MemoryS3 policies, returns all vectors from HashMap.
    /// For S3Only policy, only returns cached vectors (not full S3 state).
    ///
    /// # Note
    ///
    /// This method is primarily used for recovery and index rebuilding.
    /// For S3Only policy, consider loading from S3 snapshots instead.
    #[must_use]
    pub fn all_vectors(&self) -> Vec<VectorDocument> {
        match self.config.tiering_policy {
            TieringPolicy::Memory | TieringPolicy::MemoryS3 => {
                self.vector_store.read().values().cloned().collect()
            }
            TieringPolicy::S3Only => {
                // For S3Only, return cached vectors
                if let Some(cache) = &self.vector_cache {
                    cache.read().iter().map(|(_, v)| v.clone()).collect()
                } else {
                    Vec::new()
                }
            }
        }
    }

    /// Get current storage metrics
    #[must_use]
    pub fn metrics(&self) -> StorageMetrics {
        let mut metrics = self.metrics.read().clone();

        // Phase 7 Week 1: Update circuit breaker metrics
        if let Some(cb) = &self.circuit_breaker {
            metrics.circuit_breaker_state = cb.state().to_metric();
            metrics.circuit_breaker_error_rate = cb.error_rate();
        }

        metrics
    }

    /// Get storage configuration
    #[must_use]
    pub fn config(&self) -> &StorageConfig {
        &self.config
    }

    /// Check if compaction is needed
    ///
    /// Returns true if either:
    /// - WAL size exceeds threshold
    /// - WAL operation count exceeds threshold
    #[must_use]
    pub fn should_compact(&self) -> bool {
        let metrics = self.metrics.read();
        metrics.wal_size_bytes >= self.config.compaction_threshold_bytes
            || metrics.inserts >= self.config.compaction_threshold_ops
    }

    /// Recover from WAL by replaying all entries
    ///
    /// This method is called automatically during `new()` to restore
    /// in-memory state from the Write-Ahead Log.
    ///
    /// # Recovery Behavior
    ///
    /// - **Memory/MemoryS3:** Rebuild HashMap from WAL
    /// - **S3Only:** Populate LRU cache with recent vectors from WAL
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - WAL replay fails
    /// - Entry deserialization fails
    pub async fn recover(&self) -> CoreResult<()> {
        let entries = self.wal.replay(LogSequenceNumber::ZERO).await?;

        for (_lsn, entry) in entries {
            match entry {
                LogEntry::Upsert {
                    doc_id,
                    vector,
                    external_id,
                    metadata,
                    timestamp,
                    ..
                } => {
                    // Reconstruct VectorDocument
                    let mut doc = VectorDocument::new(doc_id.clone(), vector);
                    if let Some(ext_id) = external_id {
                        doc = doc.with_external_id(ext_id);
                    }
                    if let Some(meta) = metadata {
                        doc = doc.with_metadata(meta);
                    }
                    // Update timestamp
                    doc.inserted_at = timestamp;

                    // Apply to storage
                    match self.config.tiering_policy {
                        TieringPolicy::Memory | TieringPolicy::MemoryS3 => {
                            self.vector_store.write().insert(doc_id, doc);
                        }
                        TieringPolicy::S3Only => {
                            // For S3Only, only add to cache (S3 is source of truth)
                            if let Some(cache) = &self.vector_cache {
                                cache.write().put(doc_id, doc);
                            }
                        }
                    }
                }

                LogEntry::Delete { doc_id, .. } => {
                    // Apply deletion
                    match self.config.tiering_policy {
                        TieringPolicy::Memory | TieringPolicy::MemoryS3 => {
                            self.vector_store.write().remove(&doc_id);
                        }
                        TieringPolicy::S3Only => {
                            if let Some(cache) = &self.vector_cache {
                                cache.write().pop(&doc_id);
                            }
                        }
                    }
                }

                // Ignore collection-level operations for now
                LogEntry::CreateCollection { .. }
                | LogEntry::DeleteCollection { .. }
                | LogEntry::Checkpoint { .. } => {}
            }
        }

        Ok(())
    }

    /// Compact storage by creating snapshot and truncating WAL
    ///
    /// # Compaction Process
    ///
    /// 1. Create snapshot of current vector state
    /// 2. Upload snapshot to S3 (if policy requires)
    /// 3. Create checkpoint in WAL
    /// 4. Truncate WAL (remove entries before checkpoint)
    /// 5. Update metrics
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Snapshot creation fails
    /// - S3 upload fails (MemoryS3/S3Only policies)
    /// - WAL checkpoint fails
    pub async fn compact(&self) -> CoreResult<()> {
        // 1. Collect current vector state
        let vectors: Vec<VectorDocument> = match self.config.tiering_policy {
            TieringPolicy::Memory | TieringPolicy::MemoryS3 => {
                self.vector_store.read().values().cloned().collect()
            }
            TieringPolicy::S3Only => {
                // For S3Only, snapshot the cache (not full S3 state)
                if let Some(cache) = &self.vector_cache {
                    cache.read().iter().map(|(_, v)| v.clone()).collect()
                } else {
                    Vec::new()
                }
            }
        };

        // 2. Create snapshot
        // FIX BUG #4: Use real collection_id instead of random UUID
        // This ensures snapshots are saved under correct S3 prefix for backup/restore
        self.snapshotter
            .create_snapshot(self.collection_id, vectors)
            .await?;

        // 3. Create checkpoint in WAL
        let current_lsn = self.wal.current_lsn().await?;
        let checkpoint_entry = LogEntry::Checkpoint {
            lsn: current_lsn,
            timestamp: Utc::now(),
        };

        self.wal.append(checkpoint_entry).await?;
        self.wal.flush().await?;

        // 4. Mark checkpoint (this allows WAL to discard old entries)
        self.wal.checkpoint(current_lsn).await?;

        // 5. Update metrics
        {
            let mut metrics = self.metrics.write();
            metrics.compactions += 1;
            metrics.last_snapshot_at = Some(Utc::now());

            // FIX BUG #6: Reset compaction thresholds after successful compaction
            // Without this, should_compact() stays true forever once triggered
            metrics.wal_size_bytes = 0; // WAL is now in snapshot, reset counter
            metrics.inserts = 0; // Reset insert counter for next compaction cycle
        }

        Ok(())
    }

    /// Auto-compact if thresholds exceeded
    ///
    /// Checks `should_compact()` and automatically compacts if needed.
    /// Call this periodically (e.g., after every N inserts) to maintain WAL size.
    ///
    /// # Errors
    ///
    /// Returns error if compaction fails (see `compact()` errors)
    pub async fn auto_compact(&self) -> CoreResult<()> {
        if self.should_compact() {
            self.compact().await?;
        }
        Ok(())
    }

    /// Insert with auto-compaction
    ///
    /// Convenience method that inserts a document and triggers background
    /// compaction if needed (non-blocking).
    /// Recommended for production use to maintain WAL size automatically.
    ///
    /// # Errors
    ///
    /// Returns error if insert fails
    pub async fn insert_with_auto_compact(&self, doc: VectorDocument) -> CoreResult<()> {
        self.insert(doc).await?;

        // Check if compaction needed (non-blocking)
        if self.should_compact() {
            tracing::debug!("Compaction threshold reached, notifying worker");
            self.compaction_notify.notify_one(); // Signal worker (returns immediately)
        }

        Ok(())
    }

    /// Get current cache statistics (S3Only policy only)
    pub fn get_cache_stats(&self) -> Option<CacheStats> {
        if let Some(cache) = &self.vector_cache {
            let cache_read = cache.read();
            let metrics_read = self.metrics.read();

            Some(CacheStats {
                size: cache_read.len(),
                capacity: cache_read.cap().get(),
                hit_rate: metrics_read.cache_hit_rate(),
                hits: metrics_read.cache_hits,
                misses: metrics_read.cache_misses,
            })
        } else {
            None
        }
    }

    /// Clear the vector cache (S3Only policy only, for testing)
    #[doc(hidden)]
    pub fn clear_cache(&self) {
        if let Some(cache) = &self.vector_cache {
            cache.write().clear();
        }
    }

    /// Get current Dead Letter Queue entries.
    ///
    /// **Use Case:** Manual inspection of permanent failures.
    pub fn get_dead_letter_queue(&self) -> Vec<DLQEntry> {
        self.dead_letter_queue.all_entries()
    }

    /// Clear Dead Letter Queue (after manual intervention).
    pub fn clear_dead_letter_queue(&self) {
        self.dead_letter_queue.clear();
        self.metrics.write().dlq_size = 0;
    }

    /// Graceful shutdown - flush WAL and cancel background tasks
    ///
    /// This method should be called before dropping the StorageBackend to ensure
    /// that all pending operations are completed and resources are properly cleaned up.
    ///
    /// # Shutdown Sequence
    ///
    /// 1. Abort background S3 uploader task (if running)
    /// 2. Abort background compaction worker (if running)
    /// 3. Wait for compaction to finish (up to 30s timeout)
    /// 4. Flush WAL to ensure all entries are persisted
    /// 5. Log warning if there are pending S3 uploads
    ///
    /// # Errors
    /// Get the current circuit breaker state (for health checks)
    ///
    /// Returns the current state if circuit breaker is enabled, None otherwise.
    pub fn circuit_breaker_state(&self) -> Option<crate::circuit_breaker::CircuitBreakerState> {
        self.circuit_breaker.as_ref().map(|cb| cb.state())
    }

    ///
    /// Returns error if WAL flush fails
    /// Reset circuit breaker to Closed state (admin operation)
    ///
    /// This can be used to manually recover from a circuit breaker trip
    /// when the underlying issue has been resolved.
    pub fn reset_circuit_breaker(&self) {
        if let Some(cb) = &self.circuit_breaker {
            cb.reset();
        }
    }

    /// Gracefully shuts down the storage backend and all background workers.
    ///
    /// This will:
    /// - Abort S3 uploader tasks
    /// - Stop compaction worker
    /// - Stop retry worker
    /// - Flush pending WAL entries
    /// - Release all resources
    pub async fn shutdown(&self) -> CoreResult<()> {
        tracing::info!("StorageBackend shutting down");

        // Abort background S3 uploader task
        if let Some(handle) = &self.s3_uploader_handle {
            handle.abort();
            tracing::debug!("S3 uploader task aborted");
        }

        // Abort retry worker
        if let Some(handle) = &self.retry_handle {
            handle.abort();
            tracing::debug!("S3 retry worker aborted");
        }

        // Shutdown compaction worker
        if let Some(handle) = &self.compaction_handle {
            handle.abort();

            // Wait for cleanup (with timeout)
            match tokio::time::timeout(std::time::Duration::from_secs(30), async {
                while !handle.is_finished() {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            })
            .await
            {
                Ok(_) => tracing::info!("Compaction worker shutdown cleanly"),
                Err(_) => tracing::warn!("Compaction worker shutdown timeout"),
            }
        }

        // Phase 7 Week 1 Days 3-4: Shutdown DLQ cleanup worker and persist
        if let Some(handle) = &self.dlq_cleanup_handle {
            handle.abort();
            tracing::debug!("DLQ cleanup worker aborted");
        }

        // Persist DLQ before shutdown
        if let Err(e) = self.dead_letter_queue.persist().await {
            tracing::error!("Failed to persist DLQ on shutdown: {}", e);
        } else {
            tracing::info!("DLQ persisted on shutdown");
        }

        // Flush WAL
        self.wal.flush().await?;
        tracing::debug!("WAL flushed");

        let queue_size = self.s3_upload_queue.read().len();
        if queue_size > 0 {
            tracing::warn!("Shutting down with {} pending S3 uploads", queue_size);
        }

        let retry_queue_size = self.retry_queue.read().len();
        if retry_queue_size > 0 {
            tracing::warn!("Shutting down with {} pending retries", retry_queue_size);
        }

        let dlq_size = self.dead_letter_queue.size();
        if dlq_size > 0 {
            tracing::warn!("Shutting down with {} DLQ entries", dlq_size);
        }

        Ok(())
    }
}

impl Drop for StorageBackend {
    fn drop(&mut self) {
        // Best-effort cleanup (blocking)
        if let Some(handle) = &self.s3_uploader_handle {
            handle.abort();
        }

        if let Some(handle) = &self.retry_handle {
            handle.abort();
        }

        if let Some(handle) = &self.compaction_handle {
            handle.abort();
        }

        if let Some(handle) = &self.dlq_cleanup_handle {
            handle.abort();
        }

        tracing::debug!("StorageBackend dropped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_storage_backend_creation_memory() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");
        let snapshot_dir = temp_dir.path().join("snapshots");
        std::fs::create_dir_all(&snapshot_dir).unwrap();

        let config = StorageConfig::memory(&wal_path);
        let mut config = config;
        config.snapshot_dir = snapshot_dir;

        let backend = StorageBackend::new(config).await.unwrap();

        // Verify backend created
        assert_eq!(backend.config().tiering_policy, TieringPolicy::Memory);
        assert!(backend.object_store.is_none());
        assert!(backend.vector_cache.is_none());

        // Verify metrics initialized
        let metrics = backend.metrics();
        assert_eq!(metrics.inserts, 0);
        assert_eq!(metrics.queries, 0);
    }

    #[tokio::test]
    async fn test_storage_backend_should_compact_default() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");
        let snapshot_dir = temp_dir.path().join("snapshots");
        std::fs::create_dir_all(&snapshot_dir).unwrap();

        let config = StorageConfig::memory(&wal_path);
        let mut config = config;
        config.snapshot_dir = snapshot_dir;

        let backend = StorageBackend::new(config).await.unwrap();

        // Should not need compaction initially
        assert!(!backend.should_compact());
    }

    #[tokio::test]
    async fn test_storage_backend_config_validation() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");
        let snapshot_dir = temp_dir.path().join("snapshots");

        // Don't create snapshot directory
        // std::fs::create_dir_all(&snapshot_dir).unwrap();

        // Memory policy with missing snapshot dir should still work
        // (snapshot dir only required for S3 policies)
        let config = StorageConfig::memory(&wal_path);
        let backend_result = StorageBackend::new(config).await;
        assert!(backend_result.is_ok());

        // MemoryS3 policy with missing snapshot dir should fail
        std::fs::create_dir_all(temp_dir.path().join("snapshots2")).unwrap();
        let config = StorageConfig::memory_s3(
            &wal_path,
            &snapshot_dir, // This doesn't exist
            "test-bucket".to_string(),
        );
        let backend_result = StorageBackend::new(config).await;
        assert!(backend_result.is_err());
    }

    #[tokio::test]
    async fn test_insert_memory_policy() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");
        let snapshot_dir = temp_dir.path().join("snapshots");
        std::fs::create_dir_all(&snapshot_dir).unwrap();

        let config = StorageConfig::memory(&wal_path);
        let mut config = config;
        config.snapshot_dir = snapshot_dir;

        let backend = StorageBackend::new(config).await.unwrap();

        let doc = VectorDocument::new(DocumentId::new(), vec![1.0, 2.0, 3.0, 4.0]);

        let doc_id = doc.doc_id.clone();

        backend.insert(doc).await.unwrap();

        // Verify metrics
        assert_eq!(backend.metrics().inserts, 1);
        assert_eq!(backend.count(), 1);

        // Verify can retrieve
        let retrieved = backend.get(&doc_id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().vector, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[tokio::test]
    async fn test_insert_and_get() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");
        let snapshot_dir = temp_dir.path().join("snapshots");
        std::fs::create_dir_all(&snapshot_dir).unwrap();

        let config = StorageConfig::memory(&wal_path);
        let mut config = config;
        config.snapshot_dir = snapshot_dir;

        let backend = StorageBackend::new(config).await.unwrap();

        // Insert 10 documents
        let mut doc_ids = Vec::new();
        for i in 0..10 {
            let doc = VectorDocument::new(DocumentId::new(), vec![i as f32; 128])
                .with_external_id(format!("ext-{}", i));

            doc_ids.push(doc.doc_id.clone());
            backend.insert(doc).await.unwrap();
        }

        // Verify count
        assert_eq!(backend.count(), 10);
        assert_eq!(backend.metrics().inserts, 10);

        // Verify can retrieve all
        for (i, doc_id) in doc_ids.iter().enumerate() {
            let doc = backend.get(doc_id).await.unwrap();
            assert!(doc.is_some());
            let doc = doc.unwrap();
            assert_eq!(doc.external_id, Some(format!("ext-{}", i)));
            assert_eq!(doc.vector[0], i as f32);
        }

        // Verify get nonexistent returns None
        let nonexistent = backend.get(&DocumentId::new()).await.unwrap();
        assert!(nonexistent.is_none());
    }

    #[tokio::test]
    async fn test_delete() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");
        let snapshot_dir = temp_dir.path().join("snapshots");
        std::fs::create_dir_all(&snapshot_dir).unwrap();

        let config = StorageConfig::memory(&wal_path);
        let mut config = config;
        config.snapshot_dir = snapshot_dir;

        let backend = StorageBackend::new(config).await.unwrap();

        // Insert 5 documents
        let mut doc_ids = Vec::new();
        for i in 0..5 {
            let doc = VectorDocument::new(DocumentId::new(), vec![i as f32; 64]);

            doc_ids.push(doc.doc_id.clone());
            backend.insert(doc).await.unwrap();
        }

        assert_eq!(backend.count(), 5);

        // Delete doc 2
        backend.delete(&doc_ids[2]).await.unwrap();

        assert_eq!(backend.count(), 4);
        assert_eq!(backend.metrics().deletes, 1);

        // Verify deleted doc not found
        let deleted = backend.get(&doc_ids[2]).await.unwrap();
        assert!(deleted.is_none());

        // Verify other docs still exist
        let doc0 = backend.get(&doc_ids[0]).await.unwrap();
        assert!(doc0.is_some());
    }

    #[tokio::test]
    async fn test_metrics_tracking() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");
        let snapshot_dir = temp_dir.path().join("snapshots");
        std::fs::create_dir_all(&snapshot_dir).unwrap();

        let config = StorageConfig::memory(&wal_path);
        let mut config = config;
        config.snapshot_dir = snapshot_dir;

        let backend = StorageBackend::new(config).await.unwrap();

        // Insert 100 docs
        for i in 0..100 {
            let doc = VectorDocument::new(DocumentId::new(), vec![i as f32; 128]);

            backend.insert(doc).await.unwrap();
        }

        let metrics = backend.metrics();
        assert_eq!(metrics.inserts, 100);
        assert_eq!(metrics.deletes, 0);
        assert_eq!(metrics.queries, 0);
    }

    #[tokio::test]
    async fn test_recovery_from_wal() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");
        let snapshot_dir = temp_dir.path().join("snapshots");
        std::fs::create_dir_all(&snapshot_dir).unwrap();

        let config = StorageConfig::memory(&wal_path);
        let mut config = config;
        config.snapshot_dir = snapshot_dir.clone();

        // Create backend and insert 5 documents
        {
            let backend = StorageBackend::new(config.clone()).await.unwrap();

            for i in 0..5 {
                let doc = VectorDocument::new(DocumentId::new(), vec![i as f32; 128])
                    .with_external_id(format!("doc-{}", i));
                backend.insert(doc).await.unwrap();
            }

            assert_eq!(backend.count(), 5);
        }

        // Create new backend with same WAL path (simulates restart)
        {
            let backend = StorageBackend::new(config).await.unwrap();

            // Should have recovered all 5 documents
            assert_eq!(backend.count(), 5);
        }
    }

    #[tokio::test]
    async fn test_recovery_with_deletes() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");
        let snapshot_dir = temp_dir.path().join("snapshots");
        std::fs::create_dir_all(&snapshot_dir).unwrap();

        let config = StorageConfig::memory(&wal_path);
        let mut config = config;
        config.snapshot_dir = snapshot_dir.clone();

        let mut doc_ids = Vec::new();

        // Create backend, insert 5, delete 2
        {
            let backend = StorageBackend::new(config.clone()).await.unwrap();

            for i in 0..5 {
                let doc = VectorDocument::new(DocumentId::new(), vec![i as f32; 128]);
                doc_ids.push(doc.doc_id.clone());
                backend.insert(doc).await.unwrap();
            }

            // Delete first 2
            backend.delete(&doc_ids[0]).await.unwrap();
            backend.delete(&doc_ids[1]).await.unwrap();

            assert_eq!(backend.count(), 3);
        }

        // Recover and verify
        {
            let backend = StorageBackend::new(config).await.unwrap();

            // Should have 3 documents (5 - 2 deletes)
            assert_eq!(backend.count(), 3);

            // Verify deleted docs are gone
            assert!(backend.get(&doc_ids[0]).await.unwrap().is_none());
            assert!(backend.get(&doc_ids[1]).await.unwrap().is_none());

            // Verify remaining docs exist
            assert!(backend.get(&doc_ids[2]).await.unwrap().is_some());
            assert!(backend.get(&doc_ids[3]).await.unwrap().is_some());
            assert!(backend.get(&doc_ids[4]).await.unwrap().is_some());
        }
    }

    #[tokio::test]
    async fn test_compaction() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");
        let snapshot_dir = temp_dir.path().join("snapshots");
        std::fs::create_dir_all(&snapshot_dir).unwrap();

        let config = StorageConfig::memory(&wal_path);
        let mut config = config;
        config.snapshot_dir = snapshot_dir;

        let backend = StorageBackend::new(config).await.unwrap();

        // Insert 10 documents
        for i in 0..10 {
            let doc = VectorDocument::new(DocumentId::new(), vec![i as f32; 128]);
            backend.insert(doc).await.unwrap();
        }

        // Perform compaction
        backend.compact().await.unwrap();

        // Verify metrics
        let metrics = backend.metrics();
        assert_eq!(metrics.compactions, 1);
        assert!(metrics.last_snapshot_at.is_some());

        // Verify all documents still accessible
        assert_eq!(backend.count(), 10);
    }

    #[tokio::test]
    async fn test_compaction_threshold() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");
        let snapshot_dir = temp_dir.path().join("snapshots");
        std::fs::create_dir_all(&snapshot_dir).unwrap();

        let config = StorageConfig::memory(&wal_path).with_compaction_thresholds(1_000_000, 100); // Trigger at 100 ops
        let mut config = config;
        config.snapshot_dir = snapshot_dir;

        let backend = StorageBackend::new(config).await.unwrap();

        // Insert 50 documents (below threshold)
        for i in 0..50 {
            let doc = VectorDocument::new(DocumentId::new(), vec![i as f32; 128]);
            backend.insert(doc).await.unwrap();
        }

        // Should not trigger compaction yet
        assert!(!backend.should_compact());

        // Insert 50 more (total 100, reaches threshold)
        for i in 50..100 {
            let doc = VectorDocument::new(DocumentId::new(), vec![i as f32; 128]);
            backend.insert(doc).await.unwrap();
        }

        // Should trigger compaction
        assert!(backend.should_compact());
    }

    #[tokio::test]
    async fn test_recovery_after_compaction() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");
        let snapshot_dir = temp_dir.path().join("snapshots");
        std::fs::create_dir_all(&snapshot_dir).unwrap();

        let config = StorageConfig::memory(&wal_path);
        let mut config = config;
        config.snapshot_dir = snapshot_dir.clone();

        // Phase 1: Insert and compact
        {
            let backend = StorageBackend::new(config.clone()).await.unwrap();

            for i in 0..10 {
                let doc = VectorDocument::new(DocumentId::new(), vec![i as f32; 128]);
                backend.insert(doc).await.unwrap();
            }

            backend.compact().await.unwrap();
            assert_eq!(backend.count(), 10);
        }

        // Phase 2: Recover from WAL (should still work after compaction)
        {
            let backend = StorageBackend::new(config).await.unwrap();
            assert_eq!(backend.count(), 10);
        }
    }

    #[tokio::test]
    async fn test_auto_compact() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");
        let snapshot_dir = temp_dir.path().join("snapshots");
        std::fs::create_dir_all(&snapshot_dir).unwrap();

        let config = StorageConfig::memory(&wal_path).with_compaction_thresholds(1_000_000, 10); // Low threshold for testing
        let mut config = config;
        config.snapshot_dir = snapshot_dir;
        config.enable_background_compaction = true;
        config.compaction_config.threshold_ops = 10;

        let backend = StorageBackend::new(config).await.unwrap();

        // Insert 9 documents (below threshold)
        for i in 0..9 {
            let doc = VectorDocument::new(DocumentId::new(), vec![i as f32; 128]);
            backend.insert(doc).await.unwrap();
        }

        // No compaction yet
        assert_eq!(backend.metrics().compactions, 0);

        // Insert with auto-compact (reaches threshold of 10)
        let doc = VectorDocument::new(DocumentId::new(), vec![10.0; 128]);
        backend.insert_with_auto_compact(doc).await.unwrap();

        // Wait for background compaction to finish
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Should have triggered auto-compaction
        assert!(backend.metrics().compactions >= 1);
        assert!(backend.metrics().last_snapshot_at.is_some());

        backend.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_insert_with_auto_compact_batch() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");
        let snapshot_dir = temp_dir.path().join("snapshots");
        std::fs::create_dir_all(&snapshot_dir).unwrap();

        let config = StorageConfig::memory(&wal_path).with_compaction_thresholds(1_000_000, 50); // Trigger at 50 ops
        let mut config = config;
        config.snapshot_dir = snapshot_dir;
        config.enable_background_compaction = true;
        config.compaction_config.threshold_ops = 50;

        let backend = StorageBackend::new(config).await.unwrap();

        // Insert 100 documents with auto-compact
        for i in 0..100 {
            let doc = VectorDocument::new(DocumentId::new(), vec![i as f32; 128]);
            backend.insert_with_auto_compact(doc).await.unwrap();
        }

        // Wait for background compaction to finish
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Should have auto-compacted at least once (at 50 ops)
        assert!(backend.metrics().compactions >= 1);

        // All documents should still be accessible
        assert_eq!(backend.count(), 100);

        backend.shutdown().await.unwrap();
    }

    // Day 3 Tests: Background Compaction Worker

    #[tokio::test]
    async fn test_background_compaction_non_blocking() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");
        let snapshot_dir = temp_dir.path().join("snapshots");
        std::fs::create_dir_all(&snapshot_dir).unwrap();

        let config = StorageConfig::memory(&wal_path).with_compaction_thresholds(1024, 10); // Low threshold to trigger easily
        let mut config = config;
        config.snapshot_dir = snapshot_dir;
        config.enable_background_compaction = true;
        config.compaction_config.threshold_bytes = 1024;
        config.compaction_config.threshold_ops = 10;

        let backend = StorageBackend::new(config).await.unwrap();

        // Insert enough vectors to trigger compaction
        let start = std::time::Instant::now();
        for i in 0..20 {
            let doc = VectorDocument::new(DocumentId::new(), vec![i as f32; 128]);
            backend.insert_with_auto_compact(doc).await.unwrap();
        }
        let elapsed = start.elapsed();

        // ASSERT: All inserts complete quickly (no blocking)
        assert!(
            elapsed < std::time::Duration::from_millis(500),
            "Inserts should not block, took {:?}",
            elapsed
        );

        // Wait for background compaction to finish
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // ASSERT: Compaction happened
        let metrics = backend.metrics();
        assert!(
            metrics.compactions >= 1,
            "Background compaction should have run"
        );
        assert!(metrics.last_snapshot_at.is_some());

        // Cleanup
        backend.shutdown().await.unwrap();
    }

    #[tokio::test]
    #[ignore] // Slow test - only run manually
    async fn test_periodic_compaction_trigger() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");
        let snapshot_dir = temp_dir.path().join("snapshots");
        std::fs::create_dir_all(&snapshot_dir).unwrap();

        let config = StorageConfig::memory(&wal_path);
        let mut config = config;
        config.snapshot_dir = snapshot_dir;
        config.enable_background_compaction = true;
        config.compaction_config.threshold_bytes = 1_000_000; // High threshold (won't trigger via size)
        config.compaction_config.threshold_ops = 1_000_000; // High threshold (won't trigger via ops)

        let backend = StorageBackend::new(config).await.unwrap();

        // Insert a few vectors (below threshold)
        for i in 0..5 {
            let doc = VectorDocument::new(DocumentId::new(), vec![i as f32; 128]);
            backend.insert(doc).await.unwrap();
        }

        // Initial check - no compaction yet
        assert_eq!(backend.metrics().compactions, 0);

        // Wait for periodic trigger (5 minutes + buffer)
        // Note: This test would take too long in practice, so it's marked #[ignore]
        tokio::time::sleep(std::time::Duration::from_secs(310)).await;

        let metrics = backend.metrics();
        assert!(
            metrics.compactions > 0,
            "Periodic trigger should have fired"
        );

        backend.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_compaction_worker_graceful_shutdown() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");
        let snapshot_dir = temp_dir.path().join("snapshots");
        std::fs::create_dir_all(&snapshot_dir).unwrap();

        let config = StorageConfig::memory(&wal_path).with_compaction_thresholds(1024, 10);
        let mut config = config;
        config.snapshot_dir = snapshot_dir;
        config.enable_background_compaction = true;

        let backend = StorageBackend::new(config).await.unwrap();

        // Trigger compaction
        for i in 0..15 {
            let doc = VectorDocument::new(DocumentId::new(), vec![i as f32; 128]);
            backend.insert_with_auto_compact(doc).await.unwrap();
        }

        // Wait a moment for compaction to start
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Shutdown should complete cleanly
        let shutdown_result = backend.shutdown().await;
        assert!(shutdown_result.is_ok(), "Shutdown should succeed");

        // Verify count is still correct
        assert_eq!(backend.count(), 15);
    }

    // Day 4 Tests: Exponential Backoff Calculation

    #[test]
    fn test_exponential_backoff_calculation() {
        let base = std::time::Duration::from_secs(1);
        let max = std::time::Duration::from_secs(64);

        assert_eq!(
            calculate_backoff(0, base, max),
            std::time::Duration::from_secs(1)
        ); // 1 * 2^0 = 1
        assert_eq!(
            calculate_backoff(1, base, max),
            std::time::Duration::from_secs(2)
        ); // 1 * 2^1 = 2
        assert_eq!(
            calculate_backoff(2, base, max),
            std::time::Duration::from_secs(4)
        ); // 1 * 2^2 = 4
        assert_eq!(
            calculate_backoff(3, base, max),
            std::time::Duration::from_secs(8)
        ); // 1 * 2^3 = 8
        assert_eq!(
            calculate_backoff(6, base, max),
            std::time::Duration::from_secs(64)
        ); // Capped at max
        assert_eq!(
            calculate_backoff(10, base, max),
            std::time::Duration::from_secs(64)
        ); // Capped at max
    }

    #[test]
    fn test_classify_s3_error_transient() {
        assert_eq!(
            classify_s3_error("500 Internal Server Error"),
            ErrorClass::Transient
        );
        assert_eq!(
            classify_s3_error("503 Service Unavailable"),
            ErrorClass::Transient
        );
        assert_eq!(
            classify_s3_error("504 Gateway Timeout"),
            ErrorClass::Transient
        );
        assert_eq!(
            classify_s3_error("429 Too Many Requests"),
            ErrorClass::Transient
        );
        assert_eq!(classify_s3_error("timeout occurred"), ErrorClass::Transient);
        assert_eq!(classify_s3_error("connection reset"), ErrorClass::Transient);
        assert_eq!(classify_s3_error("unknown error"), ErrorClass::Transient); // Default
    }

    #[test]
    fn test_classify_s3_error_permanent() {
        assert_eq!(classify_s3_error("403 Forbidden"), ErrorClass::Permanent);
        assert_eq!(classify_s3_error("404 Not Found"), ErrorClass::Permanent);
        assert_eq!(classify_s3_error("400 Bad Request"), ErrorClass::Permanent);
    }
}
