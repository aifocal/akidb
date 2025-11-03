use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Materialized view for pre-computed query results
///
/// Materialized views store frequently-accessed query patterns in optimized form.
/// They are automatically refreshed when underlying data changes.
///
/// # Use Cases
///
/// - **Top-K Queries**: Pre-compute most similar vectors for popular items
/// - **Filtered Searches**: Cache results for common filter combinations
/// - **Aggregations**: Pre-compute statistics and summaries
///
/// # Refresh Strategies
///
/// - **On-Write**: Refresh immediately when data changes (consistent but slower writes)
/// - **Scheduled**: Refresh at regular intervals (eventual consistency)
/// - **On-Demand**: Refresh explicitly via API (manual control)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterializedView {
    /// Unique view identifier
    pub view_id: String,
    /// View name
    pub name: String,
    /// Tenant ID
    pub tenant_id: String,
    /// Source collection
    pub collection: String,
    /// View type (top_k, filtered, aggregation)
    pub view_type: MaterializedViewType,
    /// View definition (query parameters)
    pub definition: ViewDefinition,
    /// Cached results
    pub results: Vec<MaterializedResult>,
    /// Refresh strategy
    pub refresh_strategy: RefreshStrategy,
    /// Last refresh timestamp
    pub last_refreshed: DateTime<Utc>,
    /// View status
    pub status: ViewStatus,
    /// View statistics
    pub stats: ViewStats,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
}

impl MaterializedView {
    /// Create a new materialized view
    pub fn new(
        name: String,
        tenant_id: String,
        collection: String,
        view_type: MaterializedViewType,
        definition: ViewDefinition,
        refresh_strategy: RefreshStrategy,
    ) -> Self {
        let now = Utc::now();
        Self {
            view_id: uuid::Uuid::new_v4().to_string(),
            name,
            tenant_id,
            collection,
            view_type,
            definition,
            results: Vec::new(),
            refresh_strategy,
            last_refreshed: now,
            status: ViewStatus::Building,
            stats: ViewStats::default(),
            created_at: now,
        }
    }

    /// Check if view needs refresh based on strategy
    pub fn needs_refresh(&self) -> bool {
        match &self.refresh_strategy {
            RefreshStrategy::OnWrite => true, // Always refresh on write
            RefreshStrategy::Scheduled { interval_seconds } => {
                let elapsed = Utc::now().signed_duration_since(self.last_refreshed);
                elapsed.num_seconds() >= *interval_seconds as i64
            }
            RefreshStrategy::OnDemand => false, // Only refresh when explicitly requested
        }
    }

    /// Mark view as refreshed
    pub fn mark_refreshed(&mut self, result_count: usize) {
        self.last_refreshed = Utc::now();
        self.status = ViewStatus::Ready;
        self.stats.refresh_count += 1;
        self.stats.last_result_count = result_count;
    }

    /// Mark view as stale
    pub fn mark_stale(&mut self) {
        self.status = ViewStatus::Stale;
    }
}

/// Materialized view type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MaterializedViewType {
    /// Top-K nearest neighbors for a specific query vector
    TopK,
    /// Results filtered by metadata criteria
    Filtered,
    /// Aggregated statistics
    Aggregation,
}

/// View definition containing query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewDefinition {
    /// Query vector (for TopK views)
    pub query_vector: Option<Vec<f32>>,
    /// Number of results (k)
    pub k: Option<usize>,
    /// Filters (key-value pairs)
    pub filters: HashMap<String, String>,
    /// Aggregation function (for Aggregation views)
    pub aggregation: Option<AggregationType>,
}

impl ViewDefinition {
    pub fn top_k(query_vector: Vec<f32>, k: usize) -> Self {
        Self {
            query_vector: Some(query_vector),
            k: Some(k),
            filters: HashMap::new(),
            aggregation: None,
        }
    }

    pub fn filtered(filters: HashMap<String, String>, k: usize) -> Self {
        Self {
            query_vector: None,
            k: Some(k),
            filters,
            aggregation: None,
        }
    }

    pub fn aggregation(aggregation: AggregationType) -> Self {
        Self {
            query_vector: None,
            k: None,
            filters: HashMap::new(),
            aggregation: Some(aggregation),
        }
    }
}

/// Aggregation type for materialized views
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AggregationType {
    Count,
    Sum { field: String },
    Average { field: String },
    Min { field: String },
    Max { field: String },
}

/// Materialized result entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterializedResult {
    pub id: String,
    pub distance: Option<f32>,
    pub metadata: Option<serde_json::Value>,
    pub score: Option<f64>,
}

/// Refresh strategy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RefreshStrategy {
    /// Refresh immediately on data write (strong consistency)
    OnWrite,
    /// Refresh at regular intervals (eventual consistency)
    Scheduled { interval_seconds: u64 },
    /// Refresh only on explicit request (manual control)
    OnDemand,
}

/// View status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ViewStatus {
    /// View is being built
    Building,
    /// View is ready for queries
    Ready,
    /// View is stale (needs refresh)
    Stale,
    /// View is disabled
    Disabled,
}

/// View statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ViewStats {
    /// Number of times view has been refreshed
    pub refresh_count: u64,
    /// Number of times view has been queried
    pub query_count: u64,
    /// Last refresh duration (milliseconds)
    pub last_refresh_duration_ms: u64,
    /// Last result count
    pub last_result_count: usize,
}

/// Materialized view manager
#[derive(Clone)]
pub struct MaterializedViewManager {
    /// In-memory view registry
    views: Arc<RwLock<HashMap<String, MaterializedView>>>,
}

impl MaterializedViewManager {
    /// Create a new view manager
    pub fn new() -> Self {
        Self {
            views: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new materialized view
    pub async fn create_view(&self, view: MaterializedView) -> Result<String, ViewError> {
        let mut views = self.views.write().await;

        if views.contains_key(&view.view_id) {
            return Err(ViewError::AlreadyExists(view.view_id.clone()));
        }

        let view_id = view.view_id.clone();
        views.insert(view_id.clone(), view);

        Ok(view_id)
    }

    /// Get a view by ID
    pub async fn get_view(&self, view_id: &str) -> Option<MaterializedView> {
        self.views.read().await.get(view_id).cloned()
    }

    /// Update view results
    pub async fn update_results(
        &self,
        view_id: &str,
        results: Vec<MaterializedResult>,
        duration_ms: u64,
    ) -> Result<(), ViewError> {
        let mut views = self.views.write().await;

        let view = views
            .get_mut(view_id)
            .ok_or_else(|| ViewError::NotFound(view_id.to_string()))?;

        view.results = results.clone();
        view.mark_refreshed(results.len());
        view.stats.last_refresh_duration_ms = duration_ms;

        Ok(())
    }

    /// Mark view as stale
    pub async fn mark_stale(&self, collection: &str, tenant_id: &str) {
        let mut views = self.views.write().await;

        for view in views.values_mut() {
            if view.collection == collection && view.tenant_id == tenant_id {
                view.mark_stale();
            }
        }
    }

    /// Get all views for a collection
    pub async fn list_views(&self, collection: &str, tenant_id: &str) -> Vec<MaterializedView> {
        self.views
            .read()
            .await
            .values()
            .filter(|v| v.collection == collection && v.tenant_id == tenant_id)
            .cloned()
            .collect()
    }

    /// Get views that need refresh
    pub async fn get_stale_views(&self) -> Vec<MaterializedView> {
        self.views
            .read()
            .await
            .values()
            .filter(|v| v.needs_refresh() || v.status == ViewStatus::Stale)
            .cloned()
            .collect()
    }

    /// Delete a view
    pub async fn delete_view(&self, view_id: &str) -> Result<(), ViewError> {
        let mut views = self.views.write().await;

        if views.remove(view_id).is_none() {
            return Err(ViewError::NotFound(view_id.to_string()));
        }

        Ok(())
    }

    /// Increment query count for a view
    pub async fn record_query(&self, view_id: &str) -> Result<(), ViewError> {
        let mut views = self.views.write().await;

        let view = views
            .get_mut(view_id)
            .ok_or_else(|| ViewError::NotFound(view_id.to_string()))?;

        view.stats.query_count += 1;

        Ok(())
    }
}

impl Default for MaterializedViewManager {
    fn default() -> Self {
        Self::new()
    }
}

/// View-related errors
#[derive(Debug, Clone, thiserror::Error)]
pub enum ViewError {
    #[error("View not found: {0}")]
    NotFound(String),

    #[error("View already exists: {0}")]
    AlreadyExists(String),

    #[error("Invalid view definition: {0}")]
    InvalidDefinition(String),

    #[error("Refresh failed: {0}")]
    RefreshFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_materialized_view_creation() {
        let view = MaterializedView::new(
            "top_products".to_string(),
            "tenant_1".to_string(),
            "products".to_string(),
            MaterializedViewType::TopK,
            ViewDefinition::top_k(vec![1.0, 2.0, 3.0], 10),
            RefreshStrategy::Scheduled { interval_seconds: 300 },
        );

        assert_eq!(view.name, "top_products");
        assert_eq!(view.collection, "products");
        assert_eq!(view.status, ViewStatus::Building);
    }

    #[tokio::test]
    async fn test_view_manager_create_and_get() {
        let manager = MaterializedViewManager::new();

        let view = MaterializedView::new(
            "test_view".to_string(),
            "tenant_1".to_string(),
            "test_collection".to_string(),
            MaterializedViewType::TopK,
            ViewDefinition::top_k(vec![1.0, 2.0], 5),
            RefreshStrategy::OnDemand,
        );

        let view_id = manager.create_view(view.clone()).await.unwrap();

        let retrieved = manager.get_view(&view_id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test_view");
    }

    #[tokio::test]
    async fn test_view_results_update() {
        let manager = MaterializedViewManager::new();

        let view = MaterializedView::new(
            "test_view".to_string(),
            "tenant_1".to_string(),
            "test_collection".to_string(),
            MaterializedViewType::TopK,
            ViewDefinition::top_k(vec![1.0], 5),
            RefreshStrategy::OnDemand,
        );

        let view_id = manager.create_view(view).await.unwrap();

        let results = vec![MaterializedResult {
            id: "vec_1".to_string(),
            distance: Some(0.5),
            metadata: None,
            score: None,
        }];

        manager.update_results(&view_id, results, 100).await.unwrap();

        let updated = manager.get_view(&view_id).await.unwrap();
        assert_eq!(updated.status, ViewStatus::Ready);
        assert_eq!(updated.results.len(), 1);
        assert_eq!(updated.stats.refresh_count, 1);
    }

    #[tokio::test]
    async fn test_view_needs_refresh() {
        // On-demand view should not need refresh
        let mut view1 = MaterializedView::new(
            "view1".to_string(),
            "tenant_1".to_string(),
            "collection1".to_string(),
            MaterializedViewType::TopK,
            ViewDefinition::top_k(vec![1.0], 5),
            RefreshStrategy::OnDemand,
        );
        assert!(!view1.needs_refresh());

        // On-write view should always need refresh
        view1.refresh_strategy = RefreshStrategy::OnWrite;
        assert!(view1.needs_refresh());
    }

    #[tokio::test]
    async fn test_mark_stale() {
        let manager = MaterializedViewManager::new();

        let view = MaterializedView::new(
            "test_view".to_string(),
            "tenant_1".to_string(),
            "test_collection".to_string(),
            MaterializedViewType::TopK,
            ViewDefinition::top_k(vec![1.0], 5),
            RefreshStrategy::OnDemand,
        );

        let view_id = manager.create_view(view).await.unwrap();

        // Mark collection as stale
        manager.mark_stale("test_collection", "tenant_1").await;

        let updated = manager.get_view(&view_id).await.unwrap();
        assert_eq!(updated.status, ViewStatus::Stale);
    }

    #[tokio::test]
    async fn test_list_views_by_collection() {
        let manager = MaterializedViewManager::new();

        let view1 = MaterializedView::new(
            "view1".to_string(),
            "tenant_1".to_string(),
            "collection_a".to_string(),
            MaterializedViewType::TopK,
            ViewDefinition::top_k(vec![1.0], 5),
            RefreshStrategy::OnDemand,
        );

        let view2 = MaterializedView::new(
            "view2".to_string(),
            "tenant_1".to_string(),
            "collection_a".to_string(),
            MaterializedViewType::Filtered,
            ViewDefinition::filtered(HashMap::new(), 10),
            RefreshStrategy::OnDemand,
        );

        let view3 = MaterializedView::new(
            "view3".to_string(),
            "tenant_1".to_string(),
            "collection_b".to_string(),
            MaterializedViewType::TopK,
            ViewDefinition::top_k(vec![2.0], 5),
            RefreshStrategy::OnDemand,
        );

        manager.create_view(view1).await.unwrap();
        manager.create_view(view2).await.unwrap();
        manager.create_view(view3).await.unwrap();

        let views = manager.list_views("collection_a", "tenant_1").await;
        assert_eq!(views.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_view() {
        let manager = MaterializedViewManager::new();

        let view = MaterializedView::new(
            "test_view".to_string(),
            "tenant_1".to_string(),
            "test_collection".to_string(),
            MaterializedViewType::TopK,
            ViewDefinition::top_k(vec![1.0], 5),
            RefreshStrategy::OnDemand,
        );

        let view_id = manager.create_view(view).await.unwrap();
        assert!(manager.get_view(&view_id).await.is_some());

        manager.delete_view(&view_id).await.unwrap();
        assert!(manager.get_view(&view_id).await.is_none());
    }

    #[tokio::test]
    async fn test_record_query() {
        let manager = MaterializedViewManager::new();

        let view = MaterializedView::new(
            "test_view".to_string(),
            "tenant_1".to_string(),
            "test_collection".to_string(),
            MaterializedViewType::TopK,
            ViewDefinition::top_k(vec![1.0], 5),
            RefreshStrategy::OnDemand,
        );

        let view_id = manager.create_view(view).await.unwrap();

        manager.record_query(&view_id).await.unwrap();
        manager.record_query(&view_id).await.unwrap();

        let updated = manager.get_view(&view_id).await.unwrap();
        assert_eq!(updated.stats.query_count, 2);
    }
}
