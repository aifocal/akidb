use super::{AccessTracker, Tier, TieringPolicyConfig};
use akidb_core::{CollectionId, CoreError, CoreResult};
use akidb_metadata::{TierState, TierStateRepository};
use chrono::{Duration, Utc};
use std::sync::Arc;
use tokio::task::JoinHandle;

/// Tiering manager for hot/warm/cold tier transitions
///
/// Automatically moves collections between tiers based on access patterns:
/// - Hot → Warm: No access for `hot_tier_ttl_hours` (default: 6h)
/// - Warm → Cold: No access for `warm_tier_ttl_days` (default: 7d)
/// - Warm → Hot: `hot_promotion_threshold` accesses in `access_window_hours` (default: 10 in 1h)
/// - Cold → Warm: On first access (automatic)
///
/// # Example
///
/// ```no_run
/// use akidb_storage::tiering_manager::{TieringManager, TieringPolicyConfig};
/// use akidb_metadata::TierStateRepository;
/// use std::sync::Arc;
///
/// # async fn example() -> akidb_core::CoreResult<()> {
/// let pool = sqlx::SqlitePool::connect("sqlite::memory:").await?;
/// let repo = Arc::new(TierStateRepository::new(pool));
/// let policy = TieringPolicyConfig::default();
/// let mut manager = TieringManager::new(policy, repo)?;
///
/// // Start background worker
/// manager.start_worker();
///
/// // Record access
/// let collection_id = akidb_core::CollectionId::new();
/// manager.record_access(collection_id).await?;
/// # Ok(())
/// # }
/// ```
pub struct TieringManager {
    access_tracker: Arc<AccessTracker>,
    policy: TieringPolicyConfig,
    metadata: Arc<TierStateRepository>,
    worker: Option<JoinHandle<()>>,
}

impl TieringManager {
    /// Create new tiering manager
    ///
    /// # Errors
    ///
    /// Returns error if policy validation fails
    pub fn new(
        policy: TieringPolicyConfig,
        metadata: Arc<TierStateRepository>,
    ) -> CoreResult<Self> {
        policy.validate().map_err(CoreError::invalid_state)?;

        Ok(Self {
            access_tracker: Arc::new(AccessTracker::new()),
            policy,
            metadata,
            worker: None,
        })
    }

    /// Record collection access
    ///
    /// This should be called on every search/insert operation.
    /// Updates both in-memory access tracker and persistent tier state.
    pub async fn record_access(&self, collection_id: CollectionId) -> CoreResult<()> {
        self.access_tracker.record(collection_id).await?;
        self.metadata
            .update_access_time(collection_id, Utc::now())
            .await
    }

    /// Get current tier state
    pub async fn get_tier_state(&self, collection_id: CollectionId) -> CoreResult<TierState> {
        self.metadata.get_tier_state(collection_id).await
    }

    /// Promote collection from cold to warm
    ///
    /// This is typically called automatically on first access to a cold collection.
    pub async fn promote_from_cold(&self, collection_id: CollectionId) -> CoreResult<()> {
        let state = self.metadata.get_tier_state(collection_id).await?;
        if state.tier != Tier::Cold {
            return Ok(()); // Already promoted
        }

        let snapshot_id = state
            .snapshot_id
            .ok_or_else(|| CoreError::invalid_state("Cold collection missing snapshot ID"))?;

        tracing::info!(
            collection_id = %collection_id,
            snapshot_id = %snapshot_id,
            "Promoting from cold to warm"
        );

        // TODO: Download from S3 and save to warm tier
        // This will be implemented when we integrate with StorageBackend

        let warm_path = format!("warm/{}.parquet", collection_id);
        self.metadata
            .update_tier_state(collection_id, Tier::Warm, Some(warm_path), None)
            .await
    }

    /// Promote collection from warm to hot
    ///
    /// This is typically called automatically when a warm collection exceeds
    /// the access threshold.
    pub async fn promote_from_warm(&self, collection_id: CollectionId) -> CoreResult<()> {
        let state = self.metadata.get_tier_state(collection_id).await?;
        if state.tier != Tier::Warm {
            return Ok(());
        }

        tracing::info!(
            collection_id = %collection_id,
            "Promoting from warm to hot"
        );

        // TODO: Load from warm tier into RAM
        // This will be implemented when we integrate with StorageBackend

        self.metadata
            .update_tier_state(collection_id, Tier::Hot, None, None)
            .await
    }

    /// Demote collection from hot to warm
    async fn demote_to_warm(&self, collection_id: CollectionId) -> CoreResult<()> {
        let state = self.metadata.get_tier_state(collection_id).await?;
        if state.tier != Tier::Hot {
            return Ok(());
        }

        if state.pinned {
            tracing::debug!(
                collection_id = %collection_id,
                "Skipping demotion: collection is pinned"
            );
            return Ok(());
        }

        tracing::info!(
            collection_id = %collection_id,
            "Demoting from hot to warm"
        );

        // TODO: Serialize collection to Parquet and save to warm tier
        // This requires integration with StorageBackend

        let warm_path = format!("warm/{}.parquet", collection_id);
        self.metadata
            .update_tier_state(collection_id, Tier::Warm, Some(warm_path), None)
            .await
    }

    /// Demote collection from warm to cold
    async fn demote_to_cold(&self, collection_id: CollectionId) -> CoreResult<()> {
        let state = self.metadata.get_tier_state(collection_id).await?;
        if state.tier != Tier::Warm {
            return Ok(());
        }

        if state.pinned {
            tracing::debug!(
                collection_id = %collection_id,
                "Skipping demotion: collection is pinned"
            );
            return Ok(());
        }

        let warm_path = state
            .warm_file_path
            .ok_or_else(|| CoreError::invalid_state("Warm collection missing file path"))?;

        tracing::info!(
            collection_id = %collection_id,
            warm_path = %warm_path,
            "Demoting from warm to cold"
        );

        // TODO: Create snapshot and upload to S3
        // This requires integration with ParquetSnapshotter from Week 1

        let snapshot_id = uuid::Uuid::new_v4(); // Placeholder
        self.metadata
            .update_tier_state(collection_id, Tier::Cold, None, Some(snapshot_id))
            .await
    }

    /// Pin collection to hot tier (prevent demotion)
    pub async fn pin_collection(&self, collection_id: CollectionId) -> CoreResult<()> {
        self.metadata.pin_collection(collection_id).await
    }

    /// Unpin collection (allow demotion)
    pub async fn unpin_collection(&self, collection_id: CollectionId) -> CoreResult<()> {
        self.metadata.unpin_collection(collection_id).await
    }

    /// Force promote to hot (manual control)
    pub async fn force_promote_to_hot(&self, collection_id: CollectionId) -> CoreResult<()> {
        let state = self.metadata.get_tier_state(collection_id).await?;

        match state.tier {
            Tier::Hot => Ok(()),
            Tier::Warm => self.promote_from_warm(collection_id).await,
            Tier::Cold => {
                // Promote cold → warm first
                self.promote_from_cold(collection_id).await?;
                // Then warm → hot
                self.promote_from_warm(collection_id).await
            }
        }
    }

    /// Force demote to cold (manual control)
    pub async fn force_demote_to_cold(&self, collection_id: CollectionId) -> CoreResult<()> {
        let state = self.metadata.get_tier_state(collection_id).await?;

        match state.tier {
            Tier::Cold => Ok(()),
            Tier::Warm => self.demote_to_cold(collection_id).await,
            Tier::Hot => {
                // Demote hot → warm first
                self.demote_to_warm(collection_id).await?;
                // Then warm → cold
                self.demote_to_cold(collection_id).await
            }
        }
    }

    /// Start background worker
    ///
    /// The worker runs periodically (default: every 5 minutes) to:
    /// - Demote hot → warm (idle collections)
    /// - Demote warm → cold (idle collections)
    /// - Promote warm → hot (frequently accessed collections)
    pub fn start_worker(&mut self) {
        if self.worker.is_some() {
            tracing::warn!("Background worker already running");
            return;
        }

        let manager = self.clone_for_worker();
        let interval = self.policy.worker_interval();

        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);

            loop {
                ticker.tick().await;

                if let Err(e) = manager.run_tiering_cycle().await {
                    tracing::error!(error = %e, "Tiering cycle failed");
                }
            }
        });

        self.worker = Some(handle);
        tracing::info!("Background worker started (interval: {:?})", interval);
    }

    /// Run one tiering cycle (demotions and promotions)
    ///
    /// This method is called by the background worker but can also be
    /// invoked manually for testing.
    pub async fn run_tiering_cycle(&self) -> CoreResult<()> {
        tracing::info!("Starting tiering cycle");
        let start = std::time::Instant::now();

        // Demote hot → warm (no access for hot_tier_ttl_hours)
        let hot_cutoff = Utc::now() - Duration::hours(self.policy.hot_tier_ttl_hours);
        let hot_candidates = self
            .metadata
            .find_hot_collections_idle_since(hot_cutoff)
            .await?;

        for collection_id in hot_candidates {
            tracing::info!(collection_id = %collection_id, "Demoting hot → warm");
            if let Err(e) = self.demote_to_warm(collection_id).await {
                tracing::error!(
                    collection_id = %collection_id,
                    error = %e,
                    "Failed to demote hot → warm"
                );
            }
        }

        // Demote warm → cold (no access for warm_tier_ttl_days)
        let warm_cutoff = Utc::now() - Duration::days(self.policy.warm_tier_ttl_days);
        let warm_candidates = self
            .metadata
            .find_warm_collections_idle_since(warm_cutoff)
            .await?;

        for collection_id in warm_candidates {
            tracing::info!(collection_id = %collection_id, "Demoting warm → cold");
            if let Err(e) = self.demote_to_cold(collection_id).await {
                tracing::error!(
                    collection_id = %collection_id,
                    error = %e,
                    "Failed to demote warm → cold"
                );
            }
        }

        // Promote warm → hot (high access frequency)
        let access_window_start = Utc::now() - Duration::hours(self.policy.access_window_hours);
        let warm_hot_candidates = self
            .metadata
            .find_warm_collections_with_high_access(
                access_window_start,
                self.policy.hot_promotion_threshold,
            )
            .await?;

        for collection_id in warm_hot_candidates {
            tracing::info!(collection_id = %collection_id, "Promoting warm → hot");
            if let Err(e) = self.promote_from_warm(collection_id).await {
                tracing::error!(
                    collection_id = %collection_id,
                    error = %e,
                    "Failed to promote warm → hot"
                );
            }
            // Reset access window after promotion
            self.access_tracker.reset_window(collection_id).await?;
        }

        let duration = start.elapsed();
        tracing::info!(duration_ms = duration.as_millis(), "Tiering cycle complete");

        Ok(())
    }

    /// Shutdown background worker
    pub async fn shutdown(&mut self) -> CoreResult<()> {
        if let Some(handle) = self.worker.take() {
            handle.abort();
            tracing::info!("Background worker shut down");
        }
        Ok(())
    }

    /// Clone for worker (without JoinHandle)
    fn clone_for_worker(&self) -> Self {
        Self {
            access_tracker: Arc::clone(&self.access_tracker),
            policy: self.policy.clone(),
            metadata: Arc::clone(&self.metadata),
            worker: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use akidb_core::{CollectionDescriptor, DatabaseDescriptor, DatabaseId, DistanceMetric, TenantCatalog, TenantDescriptor, TenantId, TenantQuota};
    use akidb_metadata::{SqliteCollectionRepository, SqliteDatabaseRepository, SqliteTenantCatalog};

    async fn setup() -> (TieringManager, sqlx::SqlitePool) {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("../akidb-metadata/migrations")
            .run(&pool)
            .await
            .unwrap();

        let repo = Arc::new(TierStateRepository::new(pool.clone()));
        let policy = TieringPolicyConfig::default();
        let manager = TieringManager::new(policy, repo).unwrap();

        (manager, pool)
    }

    /// Create a test collection with proper tenant/database hierarchy
    async fn create_test_collection(pool: &sqlx::SqlitePool) -> CollectionId {
        // 1. Create tenant
        let tenant = TenantDescriptor::new("test-tenant", "test-tenant-slug");
        let tenant_id = tenant.tenant_id;
        let tenant_catalog = SqliteTenantCatalog::new(pool.clone());
        tenant_catalog.create(&tenant).await.unwrap();

        // 2. Create database
        let database = DatabaseDescriptor::new(
            tenant_id,
            "test-database",
            None,
        );
        let database_id = database.database_id;
        SqliteDatabaseRepository::create_with_executor(pool, &database).await.unwrap();

        // 3. Create collection
        let collection = CollectionDescriptor::new(
            database_id,
            "test-collection",
            512, // dimension
            "test-model",
        );
        let collection_id = collection.collection_id;
        SqliteCollectionRepository::create_with_executor(pool, &collection).await.unwrap();

        collection_id
    }

    #[tokio::test]
    async fn test_record_access() {
        let (manager, pool) = setup().await;
        let collection_id = create_test_collection(&pool).await;

        manager
            .metadata
            .init_tier_state(collection_id)
            .await
            .unwrap();
        manager.record_access(collection_id).await.unwrap();

        let state = manager.get_tier_state(collection_id).await.unwrap();
        assert_eq!(state.access_count, 1);
    }

    #[tokio::test]
    async fn test_get_tier_state() {
        let (manager, pool) = setup().await;
        let collection_id = create_test_collection(&pool).await;

        manager
            .metadata
            .init_tier_state(collection_id)
            .await
            .unwrap();

        let state = manager.get_tier_state(collection_id).await.unwrap();
        assert_eq!(state.tier, Tier::Hot);
    }

    #[tokio::test]
    async fn test_promote_from_warm() {
        let (manager, pool) = setup().await;
        let collection_id = create_test_collection(&pool).await;

        // Initialize as warm tier
        manager
            .metadata
            .init_tier_state(collection_id)
            .await
            .unwrap();
        let warm_path = format!("warm/{}.parquet", collection_id);
        manager
            .metadata
            .update_tier_state(collection_id, Tier::Warm, Some(warm_path.clone()), None)
            .await
            .unwrap();

        // Promote to hot
        manager.promote_from_warm(collection_id).await.unwrap();

        let state = manager.get_tier_state(collection_id).await.unwrap();
        assert_eq!(state.tier, Tier::Hot);
        assert!(state.warm_file_path.is_none());
    }

    #[tokio::test]
    async fn test_promote_from_cold() {
        let (manager, pool) = setup().await;
        let collection_id = create_test_collection(&pool).await;
        let snapshot_id = uuid::Uuid::new_v4();

        // Initialize as cold tier
        manager
            .metadata
            .init_tier_state(collection_id)
            .await
            .unwrap();
        manager
            .metadata
            .update_tier_state(collection_id, Tier::Cold, None, Some(snapshot_id))
            .await
            .unwrap();

        // Promote to warm
        manager.promote_from_cold(collection_id).await.unwrap();

        let state = manager.get_tier_state(collection_id).await.unwrap();
        assert_eq!(state.tier, Tier::Warm);
        assert!(state.warm_file_path.is_some());
    }
}
