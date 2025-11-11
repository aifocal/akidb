use akidb_core::{CollectionId, CoreError, CoreResult};
use chrono::{DateTime, SecondsFormat, Utc};
use sqlx::{query, SqlitePool};
use std::str::FromStr;

/// Tier level for a collection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Hot,
    Warm,
    Cold,
}

impl Tier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Tier::Hot => "hot",
            Tier::Warm => "warm",
            Tier::Cold => "cold",
        }
    }
}

impl FromStr for Tier {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "hot" => Ok(Tier::Hot),
            "warm" => Ok(Tier::Warm),
            "cold" => Ok(Tier::Cold),
            _ => Err(CoreError::invalid_state(format!("Invalid tier: {}", s))),
        }
    }
}

impl std::fmt::Display for Tier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Repository for tier state persistence
pub struct TierStateRepository {
    pool: SqlitePool,
}

impl TierStateRepository {
    /// Create new repository
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Initialize tier state for new collection (default: Hot)
    pub async fn init_tier_state(&self, collection_id: CollectionId) -> CoreResult<()> {
        let now = Utc::now();
        let collection_id_bytes = collection_id.to_bytes().to_vec();
        let created_at = now.to_rfc3339_opts(SecondsFormat::Millis, true);

        query(
            r#"
            INSERT INTO collection_tier_state (
                collection_id, tier, last_accessed_at, access_count,
                access_window_start, pinned, created_at, updated_at
            ) VALUES (?1, 'hot', ?2, 0, ?2, 0, ?2, ?2)
            "#,
        )
        .bind(collection_id_bytes)
        .bind(&created_at)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|e| CoreError::internal(e.to_string()))?;

        Ok(())
    }

    /// Get tier state
    pub async fn get_tier_state(&self, collection_id: CollectionId) -> CoreResult<TierState> {
        let collection_id_bytes = collection_id.to_bytes().to_vec();

        let row = query(
            r#"
            SELECT
                collection_id, tier, last_accessed_at, access_count,
                access_window_start, pinned, snapshot_id, warm_file_path,
                created_at, updated_at
            FROM collection_tier_state
            WHERE collection_id = ?1
            "#,
        )
        .bind(collection_id_bytes)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| CoreError::internal(format!("Tier state not found: {}", e)))?;

        use sqlx::Row;

        let tier_str: String = row
            .try_get("tier")
            .map_err(|e| CoreError::internal(e.to_string()))?;
        let tier = Tier::from_str(&tier_str)?;

        let snapshot_id_bytes: Option<Vec<u8>> = row
            .try_get("snapshot_id")
            .map_err(|e| CoreError::internal(e.to_string()))?;
        let snapshot_id = snapshot_id_bytes.and_then(|bytes| {
            if bytes.len() == 16 {
                let mut arr = [0u8; 16];
                arr.copy_from_slice(&bytes);
                Some(uuid::Uuid::from_bytes(arr))
            } else {
                None
            }
        });

        let last_accessed_str: String = row
            .try_get("last_accessed_at")
            .map_err(|e| CoreError::internal(e.to_string()))?;
        let access_window_str: String = row
            .try_get("access_window_start")
            .map_err(|e| CoreError::internal(e.to_string()))?;
        let created_at_str: String = row
            .try_get("created_at")
            .map_err(|e| CoreError::internal(e.to_string()))?;
        let updated_at_str: String = row
            .try_get("updated_at")
            .map_err(|e| CoreError::internal(e.to_string()))?;

        let collection_id_bytes: Vec<u8> = row
            .try_get("collection_id")
            .map_err(|e| CoreError::internal(e.to_string()))?;

        Ok(TierState {
            collection_id: CollectionId::from_bytes(&collection_id_bytes)
                .map_err(|e| CoreError::internal(e.to_string()))?,
            tier,
            last_accessed_at: DateTime::parse_from_rfc3339(&last_accessed_str)
                .map_err(|e| CoreError::internal(e.to_string()))?
                .with_timezone(&Utc),
            access_count: row
                .try_get::<i64, _>("access_count")
                .map_err(|e| CoreError::internal(e.to_string()))? as u32,
            access_window_start: DateTime::parse_from_rfc3339(&access_window_str)
                .map_err(|e| CoreError::internal(e.to_string()))?
                .with_timezone(&Utc),
            pinned: row
                .try_get::<i64, _>("pinned")
                .map_err(|e| CoreError::internal(e.to_string()))?
                != 0,
            snapshot_id,
            warm_file_path: row
                .try_get("warm_file_path")
                .map_err(|e| CoreError::internal(e.to_string()))?,
            created_at: DateTime::parse_from_rfc3339(&created_at_str)
                .map_err(|e| CoreError::internal(e.to_string()))?
                .with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                .map_err(|e| CoreError::internal(e.to_string()))?
                .with_timezone(&Utc),
        })
    }

    /// Update access time and increment counter
    pub async fn update_access_time(
        &self,
        collection_id: CollectionId,
        accessed_at: DateTime<Utc>,
    ) -> CoreResult<()> {
        let collection_id_bytes = collection_id.to_bytes().to_vec();
        let accessed_at_str = accessed_at.to_rfc3339_opts(SecondsFormat::Millis, true);

        query(
            r#"
            UPDATE collection_tier_state
            SET last_accessed_at = ?2,
                access_count = access_count + 1,
                updated_at = ?2
            WHERE collection_id = ?1
            "#,
        )
        .bind(collection_id_bytes)
        .bind(accessed_at_str)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|e| CoreError::internal(e.to_string()))?;

        Ok(())
    }

    /// Update tier state
    pub async fn update_tier_state(
        &self,
        collection_id: CollectionId,
        tier: Tier,
        warm_file_path: Option<String>,
        snapshot_id: Option<uuid::Uuid>,
    ) -> CoreResult<()> {
        let collection_id_bytes = collection_id.to_bytes().to_vec();
        let tier_str = tier.as_str();
        let snapshot_id_bytes = snapshot_id.map(|id| id.as_bytes().to_vec());
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

        query(
            r#"
            UPDATE collection_tier_state
            SET tier = ?2,
                warm_file_path = ?3,
                snapshot_id = ?4,
                updated_at = ?5
            WHERE collection_id = ?1
            "#,
        )
        .bind(collection_id_bytes)
        .bind(tier_str)
        .bind(warm_file_path)
        .bind(snapshot_id_bytes)
        .bind(now)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|e| CoreError::internal(e.to_string()))?;

        Ok(())
    }

    /// Pin collection to hot tier (prevent demotion)
    pub async fn pin_collection(&self, collection_id: CollectionId) -> CoreResult<()> {
        let collection_id_bytes = collection_id.to_bytes().to_vec();

        query(
            r#"
            UPDATE collection_tier_state
            SET pinned = 1
            WHERE collection_id = ?1
            "#,
        )
        .bind(collection_id_bytes)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|e| CoreError::internal(e.to_string()))?;

        Ok(())
    }

    /// Unpin collection (allow demotion)
    pub async fn unpin_collection(&self, collection_id: CollectionId) -> CoreResult<()> {
        let collection_id_bytes = collection_id.to_bytes().to_vec();

        query(
            r#"
            UPDATE collection_tier_state
            SET pinned = 0
            WHERE collection_id = ?1
            "#,
        )
        .bind(collection_id_bytes)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|e| CoreError::internal(e.to_string()))?;

        Ok(())
    }

    /// Find hot collections idle since cutoff
    pub async fn find_hot_collections_idle_since(
        &self,
        cutoff: DateTime<Utc>,
    ) -> CoreResult<Vec<CollectionId>> {
        let cutoff_str = cutoff.to_rfc3339_opts(SecondsFormat::Millis, true);

        let rows = query(
            r#"
            SELECT collection_id
            FROM collection_tier_state
            WHERE tier = 'hot'
              AND last_accessed_at < ?1
              AND pinned = 0
            "#,
        )
        .bind(cutoff_str)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::internal(e.to_string()))?;

        use sqlx::Row;
        rows.into_iter()
            .map(|row| -> CoreResult<CollectionId> {
                let bytes: Vec<u8> = row
                    .try_get("collection_id")
                    .map_err(|e| CoreError::internal(e.to_string()))?;
                CollectionId::from_bytes(&bytes).map_err(|e| CoreError::internal(e.to_string()))
            })
            .collect()
    }

    /// Find warm collections idle since cutoff
    pub async fn find_warm_collections_idle_since(
        &self,
        cutoff: DateTime<Utc>,
    ) -> CoreResult<Vec<CollectionId>> {
        let cutoff_str = cutoff.to_rfc3339_opts(SecondsFormat::Millis, true);

        let rows = query(
            r#"
            SELECT collection_id
            FROM collection_tier_state
            WHERE tier = 'warm'
              AND last_accessed_at < ?1
              AND pinned = 0
            "#,
        )
        .bind(cutoff_str)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::internal(e.to_string()))?;

        use sqlx::Row;
        rows.into_iter()
            .map(|row| -> CoreResult<CollectionId> {
                let bytes: Vec<u8> = row
                    .try_get("collection_id")
                    .map_err(|e| CoreError::internal(e.to_string()))?;
                CollectionId::from_bytes(&bytes).map_err(|e| CoreError::internal(e.to_string()))
            })
            .collect()
    }

    /// Find warm collections with high access frequency
    pub async fn find_warm_collections_with_high_access(
        &self,
        window_start: DateTime<Utc>,
        threshold: u32,
    ) -> CoreResult<Vec<CollectionId>> {
        let window_start_str = window_start.to_rfc3339_opts(SecondsFormat::Millis, true);
        let threshold_i64 = threshold as i64;

        let rows = query(
            r#"
            SELECT collection_id
            FROM collection_tier_state
            WHERE tier = 'warm'
              AND access_window_start >= ?1
              AND access_count >= ?2
            "#,
        )
        .bind(window_start_str)
        .bind(threshold_i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::internal(e.to_string()))?;

        use sqlx::Row;
        rows.into_iter()
            .map(|row| -> CoreResult<CollectionId> {
                let bytes: Vec<u8> = row
                    .try_get("collection_id")
                    .map_err(|e| CoreError::internal(e.to_string()))?;
                CollectionId::from_bytes(&bytes).map_err(|e| CoreError::internal(e.to_string()))
            })
            .collect()
    }
}

/// Complete tier state for a collection
#[derive(Debug, Clone)]
pub struct TierState {
    pub collection_id: CollectionId,
    pub tier: Tier,
    pub last_accessed_at: DateTime<Utc>,
    pub access_count: u32,
    pub access_window_start: DateTime<Utc>,
    pub pinned: bool,
    pub snapshot_id: Option<uuid::Uuid>,
    pub warm_file_path: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TierState {
    /// Create new tier state (default: Hot)
    pub fn new(collection_id: CollectionId) -> Self {
        let now = Utc::now();
        Self {
            collection_id,
            tier: Tier::Hot,
            last_accessed_at: now,
            access_count: 0,
            access_window_start: now,
            pinned: false,
            snapshot_id: None,
            warm_file_path: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Check if collection is hot
    pub fn is_hot(&self) -> bool {
        self.tier == Tier::Hot
    }

    /// Check if collection is warm
    pub fn is_warm(&self) -> bool {
        self.tier == Tier::Warm
    }

    /// Check if collection is cold
    pub fn is_cold(&self) -> bool {
        self.tier == Tier::Cold
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use akidb_core::{CollectionDescriptor, DatabaseDescriptor, TenantCatalog, TenantDescriptor};
    use crate::{SqliteCollectionRepository, SqliteDatabaseRepository, SqliteTenantCatalog};
    use chrono::Duration;

    async fn setup_db() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    /// Create a test collection with proper tenant/database hierarchy
    async fn create_test_collection(pool: &SqlitePool) -> CollectionId {
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
    async fn test_init_tier_state() {
        let pool = setup_db().await;
        let collection_id = create_test_collection(&pool).await;
        let repo = TierStateRepository::new(pool);

        repo.init_tier_state(collection_id).await.unwrap();

        let state = repo.get_tier_state(collection_id).await.unwrap();
        assert_eq!(state.tier, Tier::Hot);
        assert_eq!(state.access_count, 0);
        assert!(!state.pinned);
    }

    #[tokio::test]
    async fn test_update_access_time() {
        let pool = setup_db().await;
        let collection_id = create_test_collection(&pool).await;
        let repo = TierStateRepository::new(pool);

        repo.init_tier_state(collection_id).await.unwrap();

        let before = repo.get_tier_state(collection_id).await.unwrap();
        assert_eq!(before.access_count, 0);

        repo.update_access_time(collection_id, Utc::now())
            .await
            .unwrap();

        let after = repo.get_tier_state(collection_id).await.unwrap();
        assert_eq!(after.access_count, 1);
    }

    #[tokio::test]
    async fn test_update_tier_state() {
        let pool = setup_db().await;
        let collection_id = create_test_collection(&pool).await;
        let repo = TierStateRepository::new(pool);

        repo.init_tier_state(collection_id).await.unwrap();

        repo.update_tier_state(
            collection_id,
            Tier::Warm,
            Some("warm/test.parquet".to_string()),
            None,
        )
        .await
        .unwrap();

        let state = repo.get_tier_state(collection_id).await.unwrap();
        assert_eq!(state.tier, Tier::Warm);
        assert_eq!(state.warm_file_path, Some("warm/test.parquet".to_string()));
    }

    #[tokio::test]
    async fn test_pin_unpin() {
        let pool = setup_db().await;
        let collection_id = create_test_collection(&pool).await;
        let repo = TierStateRepository::new(pool);

        repo.init_tier_state(collection_id).await.unwrap();

        repo.pin_collection(collection_id).await.unwrap();
        let state = repo.get_tier_state(collection_id).await.unwrap();
        assert!(state.pinned);

        repo.unpin_collection(collection_id).await.unwrap();
        let state = repo.get_tier_state(collection_id).await.unwrap();
        assert!(!state.pinned);
    }

    #[tokio::test]
    async fn test_find_hot_idle() {
        let pool = setup_db().await;
        let collection_id = create_test_collection(&pool).await;
        let repo = TierStateRepository::new(pool);

        repo.init_tier_state(collection_id).await.unwrap();

        // Simulate old access time
        let old_time = Utc::now() - Duration::hours(12);
        repo.update_access_time(collection_id, old_time)
            .await
            .unwrap();

        let cutoff = Utc::now() - Duration::hours(6);
        let idle = repo.find_hot_collections_idle_since(cutoff).await.unwrap();

        assert!(idle.contains(&collection_id));
    }

    #[tokio::test]
    async fn test_find_warm_high_access() {
        let pool = setup_db().await;
        let collection_id = create_test_collection(&pool).await;
        let repo = TierStateRepository::new(pool);

        repo.init_tier_state(collection_id).await.unwrap();
        repo.update_tier_state(
            collection_id,
            Tier::Warm,
            Some("warm/test.parquet".to_string()),
            None,
        )
        .await
        .unwrap();

        // Simulate 15 accesses
        for _ in 0..15 {
            repo.update_access_time(collection_id, Utc::now())
                .await
                .unwrap();
        }

        let window_start = Utc::now() - Duration::hours(1);
        let candidates = repo
            .find_warm_collections_with_high_access(window_start, 10)
            .await
            .unwrap();

        assert!(candidates.contains(&collection_id));
    }
}
