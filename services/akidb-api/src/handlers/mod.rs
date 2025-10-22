//! HTTP handlers for REST API endpoints

pub mod collections;
pub mod search;
pub mod vectors;

pub use collections::{
    create_collection, delete_collection, get_collection, list_collections, ApiError,
    CollectionResponse, CreateCollectionRequest, ErrorResponse,
};
pub use search::{search_vectors, SearchRequest, SearchResponse, SearchResult};
pub use vectors::{insert_vectors, InsertVectorsRequest, InsertVectorsResponse, VectorInput};
