use akidb_core::{CollectionId, DistanceMetric};
use akidb_service::CollectionService;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct CreateCollectionRequest {
    name: String,
    dimension: u32,
    metric: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    embedding_model: Option<String>,
}

#[derive(Serialize)]
pub struct CreateCollectionResponse {
    collection_id: String,
    name: String,
    dimension: u32,
    metric: String,
}

#[tracing::instrument(skip(service, req), fields(name = %req.name, dimension = req.dimension, metric = %req.metric))]
pub async fn create_collection(
    State(service): State<Arc<CollectionService>>,
    Json(req): Json<CreateCollectionRequest>,
) -> Result<(StatusCode, Json<CreateCollectionResponse>), (StatusCode, String)> {
    // Validate name
    if req.name.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "name cannot be empty".to_string()));
    }

    // Parse metric
    let metric = match req.metric.to_lowercase().as_str() {
        "cosine" => DistanceMetric::Cosine,
        "l2" => DistanceMetric::L2,
        "dot" => DistanceMetric::Dot,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "invalid metric: '{}', must be one of: cosine, l2, dot",
                    req.metric
                ),
            ))
        }
    };

    // Create collection
    let collection_id = service
        .create_collection(req.name.clone(), req.dimension, metric, req.embedding_model)
        .await
        .map_err(|e| {
            if e.to_string().contains("dimension") {
                (StatusCode::BAD_REQUEST, e.to_string())
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            }
        })?;

    Ok((
        StatusCode::CREATED,
        Json(CreateCollectionResponse {
            collection_id: collection_id.to_string(),
            name: req.name,
            dimension: req.dimension,
            metric: req.metric,
        }),
    ))
}

#[derive(Serialize)]
pub struct ListCollectionsResponse {
    collections: Vec<CollectionInfo>,
}

#[derive(Serialize)]
pub struct CollectionInfo {
    collection_id: String,
    name: String,
    dimension: u32,
    metric: String,
    document_count: u64,
    created_at: String,
}

pub async fn list_collections(
    State(service): State<Arc<CollectionService>>,
) -> Result<Json<ListCollectionsResponse>, (StatusCode, String)> {
    let collections = service
        .list_collections()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let collection_infos = collections
        .into_iter()
        .map(|c| CollectionInfo {
            collection_id: c.collection_id.to_string(),
            name: c.name,
            dimension: c.dimension,
            metric: c.metric.as_str().to_string(),
            document_count: 0, // TODO: Get actual count from service
            created_at: c.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(ListCollectionsResponse {
        collections: collection_infos,
    }))
}

#[derive(Serialize)]
pub struct GetCollectionResponse {
    collection: CollectionInfo,
}

pub async fn get_collection(
    Path(collection_id): Path<String>,
    State(service): State<Arc<CollectionService>>,
) -> Result<Json<GetCollectionResponse>, (StatusCode, String)> {
    let collection_id = CollectionId::from_str(&collection_id).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid collection_id: {}", e),
        )
    })?;

    let collection = service.get_collection(collection_id).await.map_err(|e| {
        if e.to_string().contains("not found") {
            (StatusCode::NOT_FOUND, e.to_string())
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        }
    })?;

    // Get document count
    let document_count = service.get_count(collection_id).await.unwrap_or(0) as u64;

    Ok(Json(GetCollectionResponse {
        collection: CollectionInfo {
            collection_id: collection.collection_id.to_string(),
            name: collection.name,
            dimension: collection.dimension,
            metric: collection.metric.as_str().to_string(),
            document_count,
            created_at: collection.created_at.to_rfc3339(),
        },
    }))
}

pub async fn delete_collection(
    Path(collection_id): Path<String>,
    State(service): State<Arc<CollectionService>>,
) -> Result<StatusCode, (StatusCode, String)> {
    let collection_id = CollectionId::from_str(&collection_id).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid collection_id: {}", e),
        )
    })?;

    service
        .delete_collection(collection_id)
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") {
                (StatusCode::NOT_FOUND, e.to_string())
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            }
        })?;

    Ok(StatusCode::NO_CONTENT)
}

/// GET /metrics - Prometheus metrics endpoint
///
/// Returns metrics in Prometheus text format for scraping.
/// Includes both service-level metrics (collections, vectors, searches)
/// and storage-level metrics (S3, DLQ, circuit breaker).
///
/// If metrics are not enabled (service created without full persistence),
/// returns a message indicating metrics are unavailable.
pub async fn metrics(
    State(service): State<Arc<CollectionService>>,
) -> Result<(StatusCode, String), (StatusCode, String)> {
    let mut output = String::new();

    // Header
    output.push_str("# AkiDB 2.0 Metrics\n");
    output.push_str("# Prometheus Text Format v0.0.4\n\n");

    // Service metrics (collections, vectors, searches)
    match service.metrics() {
        Some(metrics) => {
            output.push_str(&metrics.export_prometheus().await);
            output.push('\n');
        }
        None => {
            output.push_str("# Service metrics not available\n");
            output.push_str("# Service was not created with full persistence\n\n");
        }
    }

    // Storage metrics (S3, DLQ, circuit breaker, WAL)
    match service.storage_metrics().await {
        Some(storage_metrics) => {
            output.push_str(&storage_metrics.export_prometheus());
            output.push('\n');
        }
        None => {
            output.push_str("# Storage metrics not available\n");
            output.push_str("# No storage backends configured\n\n");
        }
    }

    // Build info
    output.push_str("# HELP akidb_build_info Build information\n");
    output.push_str("# TYPE akidb_build_info gauge\n");
    output.push_str(&format!(
        "akidb_build_info{{version=\"{}\"}} 1\n",
        env!("CARGO_PKG_VERSION")
    ));

    Ok((StatusCode::OK, output))
}
