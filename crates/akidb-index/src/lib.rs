pub mod hnsw;
pub mod native;
pub mod provider;
pub mod simd;
pub mod types;

// Phase 7 M10-M12: DiskANN
pub mod diskann;

pub use hnsw::{HnswConfig, HnswIndexProvider};
pub use native::NativeIndexProvider;
pub use provider::IndexProvider;
pub use simd::compute_distance_simd;
pub use types::{
    BuildRequest, IndexBatch, IndexHandle, IndexKind, QueryVector, SearchOptions, SearchResult,
};
pub use diskann::{DiskANNConfig, DiskANNError, DiskANNIndex, IndexStats};
