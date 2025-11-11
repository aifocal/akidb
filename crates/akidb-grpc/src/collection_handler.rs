use akidb_core::{CollectionId, DocumentId, VectorDocument};
use akidb_proto::{
    collection_service_server::CollectionService as GrpcCollectionService, DeleteRequest,
    DeleteResponse, DescribeRequest, DescribeResponse, GetRequest, GetResponse, InsertRequest,
    InsertResponse, QueryRequest, QueryResponse, VectorDocument as ProtoVectorDocument,
    VectorMatch,
};
use akidb_service::CollectionService;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;
use tonic::{Request, Response, Status};

pub struct CollectionHandler {
    service: Arc<CollectionService>,
}

impl CollectionHandler {
    pub fn new(service: Arc<CollectionService>) -> Self {
        Self { service }
    }
}

#[tonic::async_trait]
impl GrpcCollectionService for CollectionHandler {
    async fn query(
        &self,
        request: Request<QueryRequest>,
    ) -> Result<Response<QueryResponse>, Status> {
        let start = Instant::now();
        let req = request.into_inner();

        // Parse collection ID
        let collection_id = CollectionId::from_str(&req.collection_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid collection_id: {}", e)))?;

        // Validate query vector
        if req.query_vector.is_empty() {
            return Err(Status::invalid_argument("query_vector cannot be empty"));
        }

        // Perform search
        let results = self
            .service
            .query(collection_id, req.query_vector, req.top_k as usize)
            .await
            .map_err(|e| {
                if e.to_string().contains("not found") {
                    Status::not_found(e.to_string())
                } else {
                    Status::internal(e.to_string())
                }
            })?;

        // Convert to protobuf
        let matches = results
            .into_iter()
            .map(|r| VectorMatch {
                doc_id: r.doc_id.to_string(),
                external_id: r.external_id,
                distance: r.score,
            })
            .collect();

        Ok(Response::new(QueryResponse {
            matches,
            latency_ms: start.elapsed().as_secs_f64() * 1000.0,
        }))
    }

    async fn insert(
        &self,
        request: Request<InsertRequest>,
    ) -> Result<Response<InsertResponse>, Status> {
        let start = Instant::now();
        let req = request.into_inner();

        let collection_id = CollectionId::from_str(&req.collection_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid collection_id: {}", e)))?;

        let doc_id = DocumentId::from_str(&req.doc_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid doc_id: {}", e)))?;

        if req.vector.is_empty() {
            return Err(Status::invalid_argument("vector cannot be empty"));
        }

        let mut doc = VectorDocument::new(doc_id, req.vector);
        if let Some(external_id) = req.external_id {
            doc = doc.with_external_id(external_id);
        }

        let inserted_id = self.service.insert(collection_id, doc).await.map_err(|e| {
            if e.to_string().contains("not found") {
                Status::not_found(e.to_string())
            } else {
                Status::internal(e.to_string())
            }
        })?;

        Ok(Response::new(InsertResponse {
            doc_id: inserted_id.to_string(),
            latency_ms: start.elapsed().as_secs_f64() * 1000.0,
        }))
    }

    async fn get(&self, request: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        let req = request.into_inner();

        let collection_id = CollectionId::from_str(&req.collection_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid collection_id: {}", e)))?;

        let doc_id = DocumentId::from_str(&req.doc_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid doc_id: {}", e)))?;

        let doc = self.service.get(collection_id, doc_id).await.map_err(|e| {
            if e.to_string().contains("not found") {
                Status::not_found(e.to_string())
            } else {
                Status::internal(e.to_string())
            }
        })?;

        let document = doc.map(|d| ProtoVectorDocument {
            doc_id: d.doc_id.to_string(),
            external_id: d.external_id,
            vector: d.vector,
            inserted_at: d.inserted_at.to_rfc3339(),
        });

        Ok(Response::new(GetResponse { document }))
    }

    async fn delete(
        &self,
        request: Request<DeleteRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let start = Instant::now();
        let req = request.into_inner();

        let collection_id = CollectionId::from_str(&req.collection_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid collection_id: {}", e)))?;

        let doc_id = DocumentId::from_str(&req.doc_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid doc_id: {}", e)))?;

        self.service
            .delete(collection_id, doc_id)
            .await
            .map_err(|e| {
                if e.to_string().contains("not found") {
                    Status::not_found(e.to_string())
                } else {
                    Status::internal(e.to_string())
                }
            })?;

        Ok(Response::new(DeleteResponse {
            latency_ms: start.elapsed().as_secs_f64() * 1000.0,
        }))
    }

    async fn describe(
        &self,
        request: Request<DescribeRequest>,
    ) -> Result<Response<DescribeResponse>, Status> {
        let req = request.into_inner();

        let collection_id = CollectionId::from_str(&req.collection_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid collection_id: {}", e)))?;

        // Get document count from service
        let document_count = self.service.get_count(collection_id).await.map_err(|e| {
            if e.to_string().contains("not found") {
                Status::not_found(e.to_string())
            } else {
                Status::internal(e.to_string())
            }
        })?;

        // For now, return basic info (TODO: integrate with metadata layer)
        Ok(Response::new(DescribeResponse {
            collection_id: collection_id.to_string(),
            name: "unknown".to_string(),  // TODO: Get from metadata
            dimension: 0,                 // TODO: Get from metadata
            metric: "cosine".to_string(), // TODO: Get from metadata
            document_count: document_count as u64,
        }))
    }
}
