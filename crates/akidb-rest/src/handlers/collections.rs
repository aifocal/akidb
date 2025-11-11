use akidb_core::{CollectionId, DocumentId, VectorDocument};
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
pub struct QueryRequest {
    query_vector: Vec<f32>,
    top_k: usize,
}

#[derive(Serialize)]
pub struct QueryResponse {
    matches: Vec<MatchResult>,
    latency_ms: f64,
}

#[derive(Serialize)]
pub struct MatchResult {
    doc_id: String,
    external_id: Option<String>,
    distance: f32,
}

#[tracing::instrument(skip(service, req), fields(collection_id = %collection_id, top_k = req.top_k))]
pub async fn query_vectors(
    Path(collection_id): Path<String>,
    State(service): State<Arc<CollectionService>>,
    Json(req): Json<QueryRequest>,
) -> Result<Json<QueryResponse>, (StatusCode, String)> {
    let start = std::time::Instant::now();

    let collection_id = CollectionId::from_str(&collection_id).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid collection_id: {}", e),
        )
    })?;

    if req.query_vector.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "query_vector cannot be empty".to_string(),
        ));
    }

    let results = service
        .query(collection_id, req.query_vector, req.top_k)
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") {
                (StatusCode::NOT_FOUND, e.to_string())
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            }
        })?;

    let matches = results
        .into_iter()
        .map(|r| MatchResult {
            doc_id: r.doc_id.to_string(),
            external_id: r.external_id,
            distance: r.score,
        })
        .collect();

    Ok(Json(QueryResponse {
        matches,
        latency_ms: start.elapsed().as_secs_f64() * 1000.0,
    }))
}

#[derive(Deserialize)]
pub struct InsertRequest {
    doc_id: String,
    external_id: Option<String>,
    vector: Vec<f32>,
}

#[derive(Serialize)]
pub struct InsertResponse {
    doc_id: String,
    latency_ms: f64,
}

#[tracing::instrument(skip(service, req), fields(collection_id = %collection_id, doc_id = %req.doc_id))]
pub async fn insert_vector(
    Path(collection_id): Path<String>,
    State(service): State<Arc<CollectionService>>,
    Json(req): Json<InsertRequest>,
) -> Result<Json<InsertResponse>, (StatusCode, String)> {
    let start = std::time::Instant::now();

    let collection_id = CollectionId::from_str(&collection_id).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid collection_id: {}", e),
        )
    })?;

    let doc_id = DocumentId::from_str(&req.doc_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid doc_id: {}", e)))?;

    if req.vector.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "vector cannot be empty".to_string(),
        ));
    }

    let mut doc = VectorDocument::new(doc_id, req.vector);
    if let Some(external_id) = req.external_id {
        doc = doc.with_external_id(external_id);
    }

    let inserted_id = service.insert(collection_id, doc).await.map_err(|e| {
        if e.to_string().contains("not found") {
            (StatusCode::NOT_FOUND, e.to_string())
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        }
    })?;

    Ok(Json(InsertResponse {
        doc_id: inserted_id.to_string(),
        latency_ms: start.elapsed().as_secs_f64() * 1000.0,
    }))
}

#[derive(Serialize)]
pub struct GetResponse {
    document: Option<VectorDocumentResponse>,
}

#[derive(Serialize)]
pub struct VectorDocumentResponse {
    doc_id: String,
    external_id: Option<String>,
    vector: Vec<f32>,
    inserted_at: String,
}

pub async fn get_vector(
    Path((collection_id, doc_id)): Path<(String, String)>,
    State(service): State<Arc<CollectionService>>,
) -> Result<Json<GetResponse>, (StatusCode, String)> {
    let collection_id = CollectionId::from_str(&collection_id).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid collection_id: {}", e),
        )
    })?;

    let doc_id = DocumentId::from_str(&doc_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid doc_id: {}", e)))?;

    let doc = service.get(collection_id, doc_id).await.map_err(|e| {
        if e.to_string().contains("not found") {
            (StatusCode::NOT_FOUND, e.to_string())
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        }
    })?;

    let document = doc.map(|d| VectorDocumentResponse {
        doc_id: d.doc_id.to_string(),
        external_id: d.external_id,
        vector: d.vector,
        inserted_at: d.inserted_at.to_rfc3339(),
    });

    Ok(Json(GetResponse { document }))
}

#[derive(Serialize)]
pub struct DeleteResponse {
    latency_ms: f64,
}

pub async fn delete_vector(
    Path((collection_id, doc_id)): Path<(String, String)>,
    State(service): State<Arc<CollectionService>>,
) -> Result<Json<DeleteResponse>, (StatusCode, String)> {
    let start = std::time::Instant::now();

    let collection_id = CollectionId::from_str(&collection_id).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid collection_id: {}", e),
        )
    })?;

    let doc_id = DocumentId::from_str(&doc_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid doc_id: {}", e)))?;

    service.delete(collection_id, doc_id).await.map_err(|e| {
        if e.to_string().contains("not found") {
            (StatusCode::NOT_FOUND, e.to_string())
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        }
    })?;

    Ok(Json(DeleteResponse {
        latency_ms: start.elapsed().as_secs_f64() * 1000.0,
    }))
}

#[derive(Serialize)]
pub struct HealthResponse {
    status: String,
    version: String,
}

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}
