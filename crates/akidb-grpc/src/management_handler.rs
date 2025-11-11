use akidb_core::{CollectionId, DistanceMetric};
use akidb_proto::{
    collection_management_service_server::CollectionManagementService as GrpcCollectionManagementService,
    CollectionInfo, CreateCollectionRequest, CreateCollectionResponse, DeleteCollectionRequest,
    DeleteCollectionResponse, GetCollectionRequest, GetCollectionResponse, ListCollectionsRequest,
    ListCollectionsResponse,
};
use akidb_service::CollectionService;
use std::str::FromStr;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct CollectionManagementHandler {
    service: Arc<CollectionService>,
}

impl CollectionManagementHandler {
    pub fn new(service: Arc<CollectionService>) -> Self {
        Self { service }
    }
}

#[tonic::async_trait]
impl GrpcCollectionManagementService for CollectionManagementHandler {
    async fn create_collection(
        &self,
        request: Request<CreateCollectionRequest>,
    ) -> Result<Response<CreateCollectionResponse>, Status> {
        let req = request.into_inner();

        // Validate name
        if req.name.is_empty() {
            return Err(Status::invalid_argument("name cannot be empty"));
        }

        // Parse metric
        let metric = match req.metric.to_lowercase().as_str() {
            "cosine" => DistanceMetric::Cosine,
            "l2" => DistanceMetric::L2,
            "dot" => DistanceMetric::Dot,
            _ => {
                return Err(Status::invalid_argument(format!(
                    "invalid metric: '{}', must be one of: cosine, l2, dot",
                    req.metric
                )))
            }
        };

        // Create collection
        let collection_id = self
            .service
            .create_collection(req.name.clone(), req.dimension, metric, req.embedding_model)
            .await
            .map_err(|e| {
                if e.to_string().contains("dimension") {
                    Status::invalid_argument(e.to_string())
                } else {
                    Status::internal(e.to_string())
                }
            })?;

        Ok(Response::new(CreateCollectionResponse {
            collection_id: collection_id.to_string(),
            name: req.name,
            dimension: req.dimension,
            metric: req.metric,
        }))
    }

    async fn list_collections(
        &self,
        _request: Request<ListCollectionsRequest>,
    ) -> Result<Response<ListCollectionsResponse>, Status> {
        let collections = self
            .service
            .list_collections()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

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

        Ok(Response::new(ListCollectionsResponse {
            collections: collection_infos,
        }))
    }

    async fn get_collection(
        &self,
        request: Request<GetCollectionRequest>,
    ) -> Result<Response<GetCollectionResponse>, Status> {
        let req = request.into_inner();

        let collection_id = CollectionId::from_str(&req.collection_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid collection_id: {}", e)))?;

        let collection = self
            .service
            .get_collection(collection_id)
            .await
            .map_err(|e| {
                if e.to_string().contains("not found") {
                    Status::not_found(e.to_string())
                } else {
                    Status::internal(e.to_string())
                }
            })?;

        // Get document count
        let document_count = self.service.get_count(collection_id).await.unwrap_or(0) as u64;

        Ok(Response::new(GetCollectionResponse {
            collection: Some(CollectionInfo {
                collection_id: collection.collection_id.to_string(),
                name: collection.name,
                dimension: collection.dimension,
                metric: collection.metric.as_str().to_string(),
                document_count,
                created_at: collection.created_at.to_rfc3339(),
            }),
        }))
    }

    async fn delete_collection(
        &self,
        request: Request<DeleteCollectionRequest>,
    ) -> Result<Response<DeleteCollectionResponse>, Status> {
        let req = request.into_inner();

        let collection_id = CollectionId::from_str(&req.collection_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid collection_id: {}", e)))?;

        self.service
            .delete_collection(collection_id)
            .await
            .map_err(|e| {
                if e.to_string().contains("not found") {
                    Status::not_found(e.to_string())
                } else {
                    Status::internal(e.to_string())
                }
            })?;

        Ok(Response::new(DeleteCollectionResponse { success: true }))
    }
}
