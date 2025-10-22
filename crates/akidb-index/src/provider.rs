use async_trait::async_trait;

use crate::types::{
    BuildRequest, IndexBatch, IndexHandle, IndexKind, QueryVector, SearchOptions, SearchResult,
};
use akidb_core::Result;

/// Trait implemented by concrete ANN index providers (FAISS, HNSW, etc.).
#[async_trait]
pub trait IndexProvider: Send + Sync {
    fn kind(&self) -> IndexKind;
    async fn build(&self, request: BuildRequest) -> Result<IndexHandle>;
    async fn add_batch(&self, handle: &IndexHandle, batch: IndexBatch) -> Result<()>;
    async fn remove(&self, handle: &IndexHandle, keys: &[String]) -> Result<()>;
    async fn search(
        &self,
        handle: &IndexHandle,
        query: QueryVector,
        options: SearchOptions,
    ) -> Result<SearchResult>;
    fn serialize(&self, handle: &IndexHandle) -> Result<Vec<u8>>;
    fn deserialize(&self, bytes: &[u8]) -> Result<IndexHandle>;

    /// Extract vectors and payloads for persistence
    ///
    /// This method retrieves all vectors and their associated payloads from the index
    /// so they can be persisted to storage. Returns empty vectors if the index
    /// does not support this operation.
    fn extract_for_persistence(
        &self,
        handle: &IndexHandle,
    ) -> Result<(Vec<Vec<f32>>, Vec<serde_json::Value>)> {
        // Default implementation returns empty - providers can override
        let _ = handle;
        Ok((Vec::new(), Vec::new()))
    }
}
