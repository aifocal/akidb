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
}
