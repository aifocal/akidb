//! HTTP handlers for REST API endpoints

pub mod collections;
pub mod search;
pub mod vectors;

// Phase 3 M4: Production Monitoring
pub mod metrics;

pub use akidb_query::{BatchQueryRequest, BatchQueryResponse};
pub use collections::{
    create_collection, delete_collection, get_collection, list_collections, ApiError,
    CollectionResponse, CreateCollectionRequest, ErrorResponse,
};
pub use metrics::metrics_handler;
pub use search::{
    batch_search_vectors, search_vectors, SearchRequest, SearchResponse, SearchResult,
};
pub use vectors::{insert_vectors, InsertVectorsRequest, InsertVectorsResponse, VectorInput};
