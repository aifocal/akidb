//! Collection management handlers

use crate::{state::AppState, validation};
use akidb_core::{
    collection::{CollectionDescriptor, DistanceMetric, PayloadSchema},
    manifest::CollectionManifest,
    Error,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, info};

/// Request to create a new collection
#[derive(Debug, Deserialize)]
pub struct CreateCollectionRequest {
    pub name: String,
    pub vector_dim: u16,
    #[serde(default = "default_distance")]
    pub distance: DistanceMetric,
    #[serde(default = "default_replication")]
    pub replication: u8,
    #[serde(default = "default_shard_count")]
    pub shard_count: u16,
    #[serde(default)]
    pub payload_schema: PayloadSchema,
}

fn default_distance() -> DistanceMetric {
    DistanceMetric::Cosine
}

fn default_replication() -> u8 {
    1
}

fn default_shard_count() -> u16 {
    1
}

/// Response for collection operations
#[derive(Debug, Serialize)]
pub struct CollectionResponse {
    pub name: String,
    pub vector_dim: u16,
    pub distance: DistanceMetric,
    pub replication: u8,
    pub shard_count: u16,
    pub segment_count: usize,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Create a new collection
pub async fn create_collection(
    State(state): State<AppState>,
    Json(req): Json<CreateCollectionRequest>,
) -> std::result::Result<Json<CollectionResponse>, ApiError> {
    info!("Creating collection: {}", req.name);

    // Validate request
    validation::validate_collection_name(&req.name)?;
    validation::validate_vector_dim(req.vector_dim)?;

    // Check if collection already exists
    if state.collection_exists(&req.name).await {
        return Err(ApiError::Conflict(format!("Collection '{}' already exists", req.name)));
    }

    // Create collection descriptor
    let descriptor = Arc::new(CollectionDescriptor {
        name: req.name.clone(),
        vector_dim: req.vector_dim,
        distance: req.distance,
        replication: req.replication,
        shard_count: req.shard_count,
        payload_schema: req.payload_schema,
    });

    // Create empty manifest
    let now = chrono::Utc::now();
    let manifest = CollectionManifest {
        collection: req.name.clone(),
        latest_version: 0,
        updated_at: now,
        dimension: req.vector_dim as u32,
        metric: req.distance,
        total_vectors: 0,
        epoch: 0,
        created_at: Some(now),
        snapshot: None,
        segments: Vec::new(),
    };

    // Create collection in storage
    state
        .storage
        .create_collection(&descriptor)
        .await
        .map_err(ApiError::Internal)?;

    // Register in app state
    state
        .register_collection(req.name.clone(), descriptor.clone(), manifest.clone())
        .await
        .map_err(ApiError::Internal)?;

    info!("Collection '{}' created successfully", req.name);

    Ok(Json(CollectionResponse {
        name: descriptor.name.clone(),
        vector_dim: descriptor.vector_dim,
        distance: descriptor.distance,
        replication: descriptor.replication,
        shard_count: descriptor.shard_count,
        segment_count: manifest.segments.len(),
    }))
}

/// Get collection information
pub async fn get_collection(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> std::result::Result<Json<CollectionResponse>, ApiError> {
    info!("Getting collection: {}", name);

    let metadata = state
        .get_collection(&name)
        .await
        .map_err(|e| match e {
            Error::NotFound(_) => ApiError::NotFound(format!("Collection '{}' not found", name)),
            _ => ApiError::Internal(e),
        })?;

    Ok(Json(CollectionResponse {
        name: metadata.descriptor.name.clone(),
        vector_dim: metadata.descriptor.vector_dim,
        distance: metadata.descriptor.distance,
        replication: metadata.descriptor.replication,
        shard_count: metadata.descriptor.shard_count,
        segment_count: metadata.manifest.segments.len(),
    }))
}

/// Delete a collection
pub async fn delete_collection(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> std::result::Result<StatusCode, ApiError> {
    info!("Deleting collection: {}", name);

    state
        .delete_collection(&name)
        .await
        .map_err(|e| match e {
            Error::NotFound(_) => ApiError::NotFound(format!("Collection '{}' not found", name)),
            _ => ApiError::Internal(e),
        })?;

    info!("Collection '{}' deleted successfully", name);
    Ok(StatusCode::NO_CONTENT)
}

/// List all collections
pub async fn list_collections(
    State(state): State<AppState>,
) -> Json<Vec<String>> {
    info!("Listing all collections");

    let collections = state.list_collections().await;

    debug!("Found {} collections", collections.len());
    Json(collections)
}

/// API error types
#[derive(Debug)]
pub enum ApiError {
    Validation(String),
    Conflict(String),
    NotFound(String),
    Internal(Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::Validation(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::Conflict(msg) => (StatusCode::CONFLICT, msg),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::Internal(err) => {
                error!("Internal error: {:?}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Internal error: {}", err))
            }
        };

        (status, Json(ErrorResponse { error: message })).into_response()
    }
}
