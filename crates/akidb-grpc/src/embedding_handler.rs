use akidb_proto::embedding::{
    embedding_service_server::EmbeddingService as GrpcEmbeddingService, EmbedRequest,
    EmbedResponse, Embedding, GetModelInfoRequest, GetModelInfoResponse, UsageInfo,
};
use akidb_service::EmbeddingManager;
use std::sync::Arc;
use std::time::Instant;
use tonic::{Request, Response, Status};

pub struct EmbeddingHandler {
    embedding_manager: Arc<EmbeddingManager>,
}

impl EmbeddingHandler {
    pub fn new(embedding_manager: Arc<EmbeddingManager>) -> Self {
        Self { embedding_manager }
    }
}

#[tonic::async_trait]
impl GrpcEmbeddingService for EmbeddingHandler {
    async fn embed(
        &self,
        request: Request<EmbedRequest>,
    ) -> Result<Response<EmbedResponse>, Status> {
        let start = Instant::now();
        let req = request.into_inner();

        // Validate input
        if req.texts.is_empty() {
            return Err(Status::invalid_argument("texts cannot be empty"));
        }

        if req.texts.len() > 32 {
            return Err(Status::invalid_argument("Maximum 32 texts per request"));
        }

        tracing::info!(
            "gRPC Embedding request: {} texts, model: {:?}",
            req.texts.len(),
            req.model
        );

        // Generate embeddings
        let embedding_vectors = self
            .embedding_manager
            .embed(req.texts.clone())
            .await
            .map_err(|e| {
                tracing::error!("Embedding generation failed: {}", e);
                Status::internal(format!("Embedding generation failed: {}", e))
            })?;

        // Get model info for dimension
        let model_info = self.embedding_manager.model_info().await.map_err(|e| {
            tracing::error!("Failed to get model info: {}", e);
            Status::internal(format!("Failed to get model info: {}", e))
        })?;

        // Convert to protobuf Embedding format
        let embeddings: Vec<Embedding> = embedding_vectors
            .into_iter()
            .map(|vec| Embedding { values: vec })
            .collect();

        // Calculate duration
        let duration_ms = start.elapsed().as_millis() as u64;

        // Estimate token count (rough: 1 token ~= 4 characters)
        let total_tokens: u64 = req
            .texts
            .iter()
            .map(|s| (s.len() / 4) as u64)
            .sum::<u64>()
            .max(req.texts.len() as u64);

        tracing::info!(
            "gRPC Embedding completed: {} embeddings generated in {}ms (dimension: {})",
            embeddings.len(),
            duration_ms,
            model_info.dimension
        );

        Ok(Response::new(EmbedResponse {
            embeddings,
            model: req.model.unwrap_or_else(|| "qwen3-0.6b-4bit".to_string()),
            dimension: model_info.dimension,
            usage: Some(UsageInfo {
                total_tokens,
                duration_ms,
            }),
        }))
    }

    async fn get_model_info(
        &self,
        _request: Request<GetModelInfoRequest>,
    ) -> Result<Response<GetModelInfoResponse>, Status> {
        let model_info = self.embedding_manager.model_info().await.map_err(|e| {
            tracing::error!("Failed to get model info: {}", e);
            Status::internal(format!("Failed to get model info: {}", e))
        })?;

        Ok(Response::new(GetModelInfoResponse {
            model: model_info.model,
            dimension: model_info.dimension,
            max_tokens: model_info.max_tokens as u64,
        }))
    }
}
