use async_trait::async_trait;

use crate::audit::AuditLogEntry;
use crate::collection::CollectionDescriptor;
use crate::database::DatabaseDescriptor;
use crate::error::CoreResult;
use crate::ids::{CollectionId, DatabaseId, DocumentId, TenantId, UserId};
use crate::tenant::TenantDescriptor;
use crate::user::UserDescriptor;
use crate::vector::{SearchResult, VectorDocument};

/// Catalog interface for managing tenant metadata.
#[async_trait]
pub trait TenantCatalog: Send + Sync {
    /// Returns all tenants in the catalog ordered by creation time.
    async fn list(&self) -> CoreResult<Vec<TenantDescriptor>>;

    /// Fetches a tenant by its identifier.
    async fn get(&self, tenant_id: TenantId) -> CoreResult<Option<TenantDescriptor>>;

    /// Persists a newly created tenant descriptor.
    async fn create(&self, tenant: &TenantDescriptor) -> CoreResult<()>;

    /// Updates an existing tenant descriptor.
    async fn update(&self, tenant: &TenantDescriptor) -> CoreResult<()>;

    /// Permanently deletes a tenant descriptor and cascades related metadata.
    async fn delete(&self, tenant_id: TenantId) -> CoreResult<()>;
}

/// Repository interface for logical database metadata.
#[async_trait]
pub trait DatabaseRepository: Send + Sync {
    /// Creates a new database entry.
    async fn create(&self, database: &DatabaseDescriptor) -> CoreResult<()>;

    /// Retrieves a database by its identifier.
    async fn get(&self, database_id: DatabaseId) -> CoreResult<Option<DatabaseDescriptor>>;

    /// Lists all databases owned by a tenant ordered by creation time.
    async fn list_by_tenant(&self, tenant_id: TenantId) -> CoreResult<Vec<DatabaseDescriptor>>;

    /// Updates an existing database descriptor.
    async fn update(&self, database: &DatabaseDescriptor) -> CoreResult<()>;

    /// Deletes a database descriptor.
    async fn delete(&self, database_id: DatabaseId) -> CoreResult<()>;
}

/// Repository interface for vector collection metadata.
#[async_trait]
pub trait CollectionRepository: Send + Sync {
    /// Creates a new collection entry.
    async fn create(&self, collection: &CollectionDescriptor) -> CoreResult<()>;

    /// Retrieves a collection by its identifier.
    async fn get(&self, collection_id: CollectionId) -> CoreResult<Option<CollectionDescriptor>>;

    /// Lists all collections owned by a database ordered by creation time.
    async fn list_by_database(
        &self,
        database_id: DatabaseId,
    ) -> CoreResult<Vec<CollectionDescriptor>>;

    /// Lists all collections across all databases ordered by creation time.
    /// Used for loading collections on startup.
    async fn list_all(&self) -> CoreResult<Vec<CollectionDescriptor>>;

    /// Updates an existing collection descriptor.
    async fn update(&self, collection: &CollectionDescriptor) -> CoreResult<()>;

    /// Deletes a collection descriptor.
    async fn delete(&self, collection_id: CollectionId) -> CoreResult<()>;
}

/// Repository interface for user management.
#[async_trait]
pub trait UserRepository: Send + Sync {
    /// Creates a new user entry.
    async fn create(&self, user: &UserDescriptor) -> CoreResult<()>;

    /// Retrieves a user by their identifier.
    async fn get(&self, user_id: UserId) -> CoreResult<Option<UserDescriptor>>;

    /// Retrieves a user by email within a tenant.
    async fn get_by_email(
        &self,
        tenant_id: TenantId,
        email: &str,
    ) -> CoreResult<Option<UserDescriptor>>;

    /// Lists all users for a tenant ordered by creation time.
    async fn list_by_tenant(&self, tenant_id: TenantId) -> CoreResult<Vec<UserDescriptor>>;

    /// Updates an existing user descriptor.
    async fn update(&self, user: &UserDescriptor) -> CoreResult<()>;

    /// Deletes a user descriptor.
    async fn delete(&self, user_id: UserId) -> CoreResult<()>;
}

/// Repository interface for audit log entries.
#[async_trait]
pub trait AuditLogRepository: Send + Sync {
    /// Creates a new audit log entry.
    async fn create(&self, entry: &AuditLogEntry) -> CoreResult<()>;

    /// Lists audit logs for a tenant with pagination.
    async fn list_by_tenant(
        &self,
        tenant_id: TenantId,
        limit: usize,
        offset: usize,
    ) -> CoreResult<Vec<AuditLogEntry>>;

    /// Lists audit logs for a specific user with pagination.
    async fn list_by_user(
        &self,
        user_id: UserId,
        limit: usize,
        offset: usize,
    ) -> CoreResult<Vec<AuditLogEntry>>;
}

/// Vector index trait for insert, search, and delete operations.
#[async_trait]
pub trait VectorIndex: Send + Sync {
    /// Inserts a vector document into the index.
    async fn insert(&self, doc: VectorDocument) -> CoreResult<()>;

    /// Inserts multiple documents in a batch.
    ///
    /// Default implementation calls `insert` for each document sequentially.
    /// Implementations may override for optimized bulk loading.
    async fn insert_batch(&self, docs: Vec<VectorDocument>) -> CoreResult<()> {
        for doc in docs {
            self.insert(doc).await?;
        }
        Ok(())
    }

    /// Searches for k nearest neighbors.
    ///
    /// Returns results sorted by score according to the distance metric:
    /// - Cosine/Dot: descending (higher is more similar)
    /// - L2: ascending (lower is more similar)
    ///
    /// # Parameters
    ///
    /// - `query`: Query vector (must match index dimension)
    /// - `k`: Number of nearest neighbors to return
    /// - `ef_search`: HNSW search parameter (optional, ignored by brute-force)
    async fn search(
        &self,
        query: &[f32],
        k: usize,
        ef_search: Option<usize>,
    ) -> CoreResult<Vec<SearchResult>>;

    /// Deletes a document by ID.
    ///
    /// HNSW implementations may use soft deletion with tombstone marking.
    async fn delete(&self, doc_id: DocumentId) -> CoreResult<()>;

    /// Retrieves a document by ID (for verification).
    async fn get(&self, doc_id: DocumentId) -> CoreResult<Option<VectorDocument>>;

    /// Returns the total number of documents in the index.
    async fn count(&self) -> CoreResult<usize>;

    /// Clears the entire index (for testing).
    async fn clear(&self) -> CoreResult<()>;
}
