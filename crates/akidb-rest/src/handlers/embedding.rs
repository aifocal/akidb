//! Embedding generation handlers for REST API

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use akidb_service::EmbeddingManager;

/// Application state containing embedding manager
pub struct AppState {
    pub embedding_manager: Arc<EmbeddingManager>,
}

/// Request payload for embedding generation
#[derive(Debug, Deserialize)]
pub struct EmbedRequest {
    /// List of texts to embed
    pub texts: Vec<String>,

    /// Optional model name (default: "qwen3-0.6b-4bit")
    #[serde(default = "default_model")]
    pub model: String,

    /// Optional pooling strategy (default: "mean")
    #[serde(default = "default_pooling")]
    pub pooling: String,

    /// Optional L2 normalization (default: true)
    #[serde(default = "default_normalize")]
    pub normalize: bool,
}

fn default_model() -> String {
    "qwen3-0.6b-4bit".to_string()
}

fn default_pooling() -> String {
    "mean".to_string()
}

fn default_normalize() -> bool {
    true
}

/// Response payload for embedding generation
#[derive(Debug, Serialize)]
pub struct EmbedResponse {
    /// Generated embeddings (one per input text)
    pub embeddings: Vec<Vec<f32>>,

    /// Model name used
    pub model: String,

    /// Embedding dimension
    pub dimension: u32,

    /// Usage information
    pub usage: UsageInfo,
}

/// Usage statistics
#[derive(Debug, Serialize)]
pub struct UsageInfo {
    /// Estimated total tokens processed
    pub total_tokens: usize,

    /// Duration in milliseconds
    pub duration_ms: u64,
}

/// POST /embed - Generate embeddings for texts
///
/// # Request
///
/// ```json
/// {
///   "texts": ["Hello world", "Machine learning"],
///   "model": "qwen3-0.6b-4bit",  // optional
///   "pooling": "mean",            // optional
///   "normalize": true             // optional
/// }
/// ```
///
/// # Response
///
/// ```json
/// {
///   "embeddings": [[0.001, ...], [0.002, ...]],
///   "model": "qwen3-0.6b-4bit",
///   "dimension": 1024,
///   "usage": {
///     "total_tokens": 42,
///     "duration_ms": 87
///   }
/// }
/// ```
pub async fn embed_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<EmbedRequest>,
) -> Result<Json<EmbedResponse>, (StatusCode, String)> {
    // Validate input
    if request.texts.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "texts cannot be empty".to_string()));
    }

    if request.texts.len() > 32 {
        return Err((
            StatusCode::BAD_REQUEST,
            "Maximum 32 texts per request".to_string(),
        ));
    }

    // Record start time
    let start = std::time::Instant::now();

    tracing::info!(
        "Embedding request: {} texts, model: {}, pooling: {}, normalize: {}",
        request.texts.len(),
        request.model,
        request.pooling,
        request.normalize
    );

    // Generate embeddings
    let embeddings = state
        .embedding_manager
        .embed(request.texts.clone())
        .await
        .map_err(|e| {
            tracing::error!("Embedding generation failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e)
        })?;

    // Calculate duration
    let duration_ms = start.elapsed().as_millis() as u64;

    // Get model info for dimension
    let model_info = state.embedding_manager.model_info().await.map_err(|e| {
        tracing::error!("Failed to get model info: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e)
    })?;

    // Estimate token count (rough: 1 token ~= 4 characters)
    let total_tokens: usize = request
        .texts
        .iter()
        .map(|s| s.len() / 4)
        .sum::<usize>()
        .max(request.texts.len());

    tracing::info!(
        "Embedding completed: {} embeddings generated in {}ms (dimension: {})",
        embeddings.len(),
        duration_ms,
        model_info.dimension
    );

    Ok(Json(EmbedResponse {
        embeddings,
        model: request.model,
        dimension: model_info.dimension,
        usage: UsageInfo {
            total_tokens,
            duration_ms,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embed_request_defaults() {
        let json = r#"{"texts": ["Hello world"]}"#;
        let request: EmbedRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.texts.len(), 1);
        assert_eq!(request.model, "qwen3-0.6b-4bit");
        assert_eq!(request.pooling, "mean");
        assert_eq!(request.normalize, true);
    }

    #[test]
    fn test_embed_request_custom() {
        let json = r#"{
            "texts": ["Test"],
            "model": "custom-model",
            "pooling": "cls",
            "normalize": false
        }"#;
        let request: EmbedRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.model, "custom-model");
        assert_eq!(request.pooling, "cls");
        assert_eq!(request.normalize, false);
    }
}
