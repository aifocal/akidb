# ADR-003: gRPC + REST Dual API Strategy

**Status:** Proposed
**Date:** 2025-11-06
**Decision Makers:** API Team Lead, Architecture Lead, Product Lead
**Consulted:** SDK Team, Customer Success, DevOps

---

## Context

AkiDB v1.x provides a REST API (Axum-based) for all operations:
- Tenant management
- Collection CRUD
- Vector ingest
- Query execution
- User/RBAC management

**Current v1.x REST API Example:**
```http
POST /api/v1/tenants/{tenant_id}/collections
Content-Type: application/json

{
  "name": "products",
  "vector_dim": 512,
  "distance": "cosine",
  "hnsw_params": { "M": 16, "ef_construction": 200 }
}
```

**Customer Feedback and Requirements:**
1. **Performance-sensitive customers** (fintech, real-time AI) request lower-latency APIs for high-frequency queries
2. **Streaming use cases** (real-time embeddings, bulk ingest) need bidirectional streaming support
3. **Type safety** concerns from enterprises using statically-typed languages (Go, Java, Rust, C++)
4. **SDK maintenance burden:** REST requires custom SDKs for each language with manual schema synchronization

Meanwhile:
5. **Existing integrations** (MCP server, CLI, web UI) rely on REST and cannot migrate immediately
6. **Developer experience:** REST is easier for prototyping, debugging (curl, browser DevTools)

We need an API strategy that:
- Provides **low-latency, type-safe** APIs for performance-critical use cases (gRPC)
- Maintains **backward compatibility** with existing REST clients
- Reduces **SDK maintenance burden** through code generation
- Supports **streaming** for bulk ingest and real-time embeddings
- Works on **ARM edge devices** (Mac ARM, Jetson, OCI ARM)

---

## Decision

We will implement a **dual API layer** in AkiDB:
- **gRPC (Tonic)** for data plane operations (high-frequency, performance-critical)
- **REST (Axum)** for control plane operations (management, admin, backward compatibility)

**API Segmentation:**

| API Category | Protocol | Use Cases | Latency Target |
|--------------|----------|-----------|----------------|
| **Data Plane** | gRPC | Vector ingest, query, bulk operations, streaming | P95 <10ms overhead |
| **Control Plane** | REST | Tenant/database/collection CRUD, user management, policy admin | P95 <50ms overhead |
| **Legacy Compatibility** | REST | Existing MCP server, CLI, web UI | Maintained indefinitely |

**Implementation Architecture:**

```rust
// Shared service layer (business logic)
// akidb-core/src/service.rs
pub struct CollectionService {
    metadata: Arc<MetadataStore>,
    storage: Arc<StorageEngine>,
}

impl CollectionService {
    pub async fn create_collection(&self, req: CreateCollectionRequest) -> Result<Collection> {
        // Business logic (shared by gRPC and REST)
    }

    pub async fn query_vectors(&self, req: QueryRequest) -> Result<QueryResponse> {
        // Business logic (shared by gRPC and REST)
    }
}

// gRPC API (data plane)
// akidb-control-plane/src/grpc.rs
use tonic::{Request, Response, Status};
use akidb_proto::collection_service_server::CollectionService as GrpcCollectionService;

pub struct GrpcCollectionHandler {
    service: Arc<CollectionService>,
}

#[tonic::async_trait]
impl GrpcCollectionService for GrpcCollectionHandler {
    async fn query_vectors(
        &self,
        request: Request<QueryRequest>,
    ) -> Result<Response<QueryResponse>, Status> {
        let req = request.into_inner();
        let response = self.service.query_vectors(req).await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(response))
    }

    // Server streaming for batch query
    type QueryBatchStream = ReceiverStream<Result<QueryResponse, Status>>;

    async fn query_batch(
        &self,
        request: Request<Streaming<QueryRequest>>,
    ) -> Result<Response<Self::QueryBatchStream>, Status> {
        // Bidirectional streaming implementation
    }
}

// REST API (control plane + legacy)
// akidb-api/src/rest.rs
use axum::{Router, Json, extract::Path};

pub fn collection_routes() -> Router {
    Router::new()
        .route("/api/v1/collections", post(create_collection))
        .route("/api/v1/collections/:id/query", post(query_vectors))
        .route("/api/v2/collections", post(create_collection_v2))  // New v2 endpoint
}

async fn query_vectors(
    Path(id): Path<Uuid>,
    Json(req): Json<QueryRequest>,
    service: Arc<CollectionService>,
) -> Result<Json<QueryResponse>, ApiError> {
    let response = service.query_vectors(req).await?;
    Ok(Json(response))
}
```

**Protobuf Schema (gRPC Contract):**
```protobuf
// akidb-proto/collection.proto
syntax = "proto3";

package akidb.collection.v1;

service CollectionService {
  rpc CreateCollection(CreateCollectionRequest) returns (Collection);
  rpc QueryVectors(QueryRequest) returns (QueryResponse);
  rpc QueryBatch(stream QueryRequest) returns (stream QueryResponse);  // Streaming
  rpc IngestVectors(stream IngestRequest) returns (IngestResponse);     // Streaming
}

message QueryRequest {
  string collection_id = 1;
  repeated float query_vector = 2 [packed=true];
  int32 top_k = 3;
  optional string filter = 4;  // Cedar policy filter
}

message QueryResponse {
  repeated VectorMatch matches = 1;
  double latency_ms = 2;
}

message VectorMatch {
  string doc_id = 1;
  float distance = 2;
  map<string, string> metadata = 3;
}
```

---

## Alternatives Considered

### Alternative 1: gRPC Only (No REST)

**Pros:**
- Single API surface to maintain
- Maximum performance (no HTTP/1.1 overhead)
- Type-safe contracts (protobuf)
- Built-in streaming support

**Cons:**
- ❌ **Breaking Change:** Existing MCP server, CLI, web UI would break
- ❌ **Developer Experience:** Harder to debug (requires gRPC tooling)
- ❌ **Browser Incompatibility:** gRPC-web requires transcoding proxy
- ❌ **Migration Burden:** Customers must update integrations immediately

**Decision:** Rejected due to breaking changes and migration risk.

### Alternative 2: REST Only (Status Quo)

**Pros:**
- Zero migration effort (v1.x compatibility)
- Simple debugging (curl, browser DevTools)
- Universal client support (every language has HTTP client)
- No protobuf toolchain required

**Cons:**
- ❌ **Performance:** HTTP/1.1 overhead (headers, JSON parsing)
- ❌ **No Streaming:** Bulk ingest requires chunking or long polling
- ❌ **SDK Maintenance:** Manual schema synchronization across SDKs
- ❌ **Type Safety:** JSON schema less strict than protobuf

**Decision:** Rejected due to performance and streaming limitations.

### Alternative 3: GraphQL

**Pros:**
- Flexible queries (clients request only needed fields)
- Strong typing (GraphQL schema)
- Single endpoint (reduces API surface)
- Good developer tooling (GraphQL Playground)

**Cons:**
- ❌ **Complexity:** GraphQL adds resolver layer overhead
- ❌ **Performance:** Query parsing and validation adds latency
- ❌ **Caching:** Difficult to cache GraphQL queries (POST requests)
- ❌ **Overkill:** Vector database operations are not graph-shaped

**Decision:** Rejected due to complexity and performance overhead.

### Alternative 4: WebSockets

**Pros:**
- Bidirectional streaming (similar to gRPC)
- Browser-compatible (no transcoding proxy)
- Low latency (persistent connections)

**Cons:**
- ❌ **No Type Safety:** JSON-based, requires manual schema validation
- ❌ **Operational Complexity:** Long-lived connections, load balancing challenges
- ❌ **Limited Ecosystem:** Fewer libraries compared to gRPC
- ❌ **No RPC Semantics:** Must build request/response matching manually

**Decision:** Rejected due to lack of type safety and operational complexity.

---

## Rationale

The dual API strategy is chosen for these reasons:

### 1. **Performance-Critical Data Plane via gRPC**

**Latency Comparison** (estimated):
| Operation | REST (HTTP/1.1 + JSON) | gRPC (HTTP/2 + Protobuf) | Improvement |
|-----------|------------------------|--------------------------|-------------|
| Query 1 vector | 12ms | 8ms | 33% faster |
| Ingest 1000 vectors | 450ms | 280ms | 38% faster |
| Bulk query (100 requests) | 3.2s | 1.1s | 66% faster (streaming) |

**Why gRPC is faster:**
- HTTP/2 multiplexing (multiple requests on single connection)
- Binary protobuf encoding (vs JSON parsing)
- Header compression (HPACK)
- No HTTP/1.1 handshake overhead per request

### 2. **Backward Compatibility via REST**

- **v1 REST API:** Maintained indefinitely for existing clients
- **v2 REST API:** New endpoints mirror gRPC (JSON representation of protobuf)
- **Migration Path:** Clients can migrate at their own pace

**Example Migration:**
```bash
# v1 REST (deprecated but supported)
curl -X POST https://akidb.example.com/api/v1/collections/{id}/query \
  -H "Content-Type: application/json" \
  -d '{"vector": [0.1, 0.2, ...], "top_k": 10}'

# v2 REST (mirrors gRPC)
curl -X POST https://akidb.example.com/api/v2/collections/{id}/query \
  -H "Content-Type: application/json" \
  -d '{"collection_id": "...", "query_vector": [0.1, 0.2, ...], "top_k": 10}'

# gRPC (performance-critical clients)
grpcurl -d '{"collection_id": "...", "query_vector": [0.1, 0.2, ...], "top_k": 10}' \
  akidb.example.com:9000 akidb.collection.v1.CollectionService/QueryVectors
```

### 3. **Streaming for Bulk Operations**

**gRPC Streaming Example (Bulk Ingest):**
```rust
// Client streaming: send many IngestRequest, receive one IngestResponse
#[tonic::async_trait]
impl CollectionService for GrpcCollectionHandler {
    async fn ingest_vectors(
        &self,
        request: Request<Streaming<IngestRequest>>,
    ) -> Result<Response<IngestResponse>, Status> {
        let mut stream = request.into_inner();
        let mut total_ingested = 0;

        while let Some(req) = stream.next().await {
            let req = req?;
            self.service.ingest_batch(req.vectors).await?;
            total_ingested += req.vectors.len();
        }

        Ok(Response::new(IngestResponse {
            total_ingested: total_ingested as u64,
        }))
    }
}
```

REST cannot achieve this without:
- Chunking requests (requires client-side batching logic)
- Long polling (inefficient, no backpressure)
- WebSockets (non-standard for REST)

### 4. **Type Safety and Code Generation**

**gRPC Protobuf:**
```protobuf
message QueryRequest {
  string collection_id = 1;
  repeated float query_vector = 2 [packed=true];
  int32 top_k = 3;
}
```

Generates type-safe clients for 10+ languages:
```bash
# Generate Rust client
cargo build  # Automatically generates from .proto via build.rs

# Generate Go client
protoc --go_out=. --go-grpc_out=. collection.proto

# Generate Python client
python -m grpc_tools.protoc -I. --python_out=. --grpc_python_out=. collection.proto
```

**REST JSON Schema** (requires manual SDKs):
```json
{
  "type": "object",
  "properties": {
    "collection_id": {"type": "string"},
    "query_vector": {"type": "array", "items": {"type": "number"}},
    "top_k": {"type": "integer"}
  }
}
```

Must manually implement SDKs for each language and keep synchronized.

### 5. **Edge Compatibility**

- **gRPC:** Native ARM64 builds (Tonic compiles to native binary)
- **HTTP/2:** Supported on Mac ARM, Jetson, OCI ARM
- **TLS:** mTLS for secure edge-to-cloud communication

---

## Consequences

### Positive

- ✅ **Performance:** 30-60% latency reduction for data plane operations
- ✅ **Streaming:** Bidirectional streaming for bulk ingest, real-time embeddings
- ✅ **Type Safety:** Protobuf contracts prevent integration bugs
- ✅ **SDK Automation:** Code generation reduces maintenance burden
- ✅ **Backward Compatibility:** Existing REST clients continue working
- ✅ **Developer Choice:** Use REST for prototyping, gRPC for production
- ✅ **Future-Proof:** gRPC supports features like deadline propagation, load balancing

### Negative

- ⚠️ **Increased Complexity:** Two API layers to maintain
  - *Mitigation:* Shared service layer (`CollectionService`) contains all business logic
  - *Testing:* Contract tests ensure gRPC and REST parity

- ⚠️ **Operational Overhead:** gRPC requires HTTP/2, TLS setup
  - *Mitigation:* Week 0 DevOps team sets up gRPC in staging (Nov 6-20)
  - *Tooling:* Automated TLS certificate rotation (Let's Encrypt)

- ⚠️ **Learning Curve:** Team needs protobuf and gRPC training
  - *Mitigation:* Week 4 gRPC workshop for API team (Dec 2-6)
  - *Documentation:* Comprehensive gRPC quickstart guide

- ⚠️ **Browser Compatibility:** gRPC requires grpc-web transcoding for web UI
  - *Mitigation:* Continue using REST for web UI (control plane)
  - *Future:* Add grpc-web proxy if browser clients need data plane access

### Trade-offs

| Dimension | gRPC | REST | Dual (Our Choice) |
|-----------|------|------|-------------------|
| Performance | ✅ Fast | ⚠️ Slower | ✅ Fast (data plane) |
| Backward Compat | ❌ Breaking | ✅ Yes | ✅ Yes (control plane) |
| Type Safety | ✅ Strict | ⚠️ Weak | ✅ Strict (gRPC clients) |
| Developer UX | ⚠️ Complex | ✅ Simple | ✅ Choice (both) |
| Streaming | ✅ Native | ❌ Workarounds | ✅ Native (gRPC) |
| Maintenance | ✅ Low (codegen) | ❌ High (manual SDKs) | ⚠️ Medium (both) |

**Verdict:** Dual API wins by providing best-in-class performance without breaking existing integrations.

---

## Implementation Plan

### Phase 4: API Unification (Weeks 13-16)

1. **Week 13:** Define protobuf schema
   ```bash
   mkdir -p akidb-proto/proto/akidb/{collection,tenant,query}.proto
   ```
   - Collection service (CRUD, query, ingest)
   - Tenant service (management, quotas)
   - Embedding service (embed text, batch embedding)

2. **Week 14:** Implement gRPC server (Tonic)
   ```rust
   // akidb-control-plane/src/grpc.rs
   use tonic::transport::Server;

   #[tokio::main]
   async fn main() -> Result<()> {
       let collection_service = GrpcCollectionHandler::new(service_layer);

       Server::builder()
           .add_service(CollectionServiceServer::new(collection_service))
           .serve("0.0.0.0:9000".parse()?)
           .await?;

       Ok(())
   }
   ```

3. **Week 15:** Extract shared service layer
   - Move business logic from `akidb-api` REST handlers to `akidb-core/service.rs`
   - Wire both gRPC and REST to shared service
   - Ensure transaction boundaries are identical

4. **Week 16:** gRPC client SDKs
   - Generate Rust, Go, Python, Java clients from protobuf
   - Publish to package registries (crates.io, npm, PyPI, Maven)
   - Update documentation with gRPC examples

### Testing Strategy

5. **Contract Tests:** Ensure gRPC and REST API parity
   ```rust
   #[tokio::test]
   async fn test_query_parity() {
       let req = QueryRequest { ... };

       // gRPC call
       let grpc_response = grpc_client.query_vectors(req.clone()).await?;

       // REST call
       let rest_response = rest_client.post("/api/v2/collections/{id}/query", req).await?;

       assert_eq!(grpc_response, rest_response);  // Results must match
   }
   ```

6. **Load Tests:** Validate gRPC performance improvement
   - Benchmark REST vs gRPC latency (P50/P95/P99)
   - Measure throughput (QPS) under load
   - Validate streaming performance (bulk ingest)

---

## Success Metrics

- [ ] **gRPC Latency:** P95 <10ms overhead vs direct function call
- [ ] **REST Parity:** 100% of v1 REST endpoints have v2 equivalents
- [ ] **SDK Coverage:** Auto-generated clients for Rust, Go, Python, Java, TypeScript
- [ ] **Migration Rate:** 50% of high-frequency clients migrate to gRPC within 3 months
- [ ] **Streaming Throughput:** Ingest 10k vectors/sec via gRPC streaming
- [ ] **Backward Compatibility:** Zero v1 REST API breakage

---

## API Versioning Strategy

- **v1 REST:** Legacy, maintained indefinitely, no new features
- **v2 REST:** New control plane, mirrors gRPC
- **gRPC v1:** Data plane, use protobuf versioning for evolution

**Breaking Changes:**
- gRPC uses protobuf field numbering (additive changes only)
- REST v2 uses semantic versioning (v2.1, v2.2 for non-breaking changes)
- Deprecation policy: 12-month notice for breaking changes

---

## References

- [gRPC Documentation](https://grpc.io/)
- [Tonic (Rust gRPC Framework)](https://github.com/hyperium/tonic)
- [Protocol Buffers](https://protobuf.dev/)
- [AkiDB v1.x REST API](../../../akidb/crates/akidb-api/)
- [HTTP/2 Specification](https://http2.github.io/)

---

## Notes

- **Security:** gRPC supports mutual TLS (mTLS) for secure edge-to-cloud communication
- **Observability:** Integrate gRPC with OTLP traces (OpenTelemetry) for distributed tracing
- **Rate Limiting:** Apply same Cedar policies to both gRPC and REST
- **Error Handling:** Map gRPC status codes (INVALID_ARGUMENT, PERMISSION_DENIED) to HTTP status codes for REST

---

**Decision Outcome:** ✅ **Approved** with Week 13-16 implementation timeline.

**Next Review:** 2025-12-20 (after Phase 4 completion)

---

**Signatures:**
- API Team Lead: _____________________ Date: _______
- Architecture Lead: __________________ Date: _______
- SDK Team Lead: ____________________ Date: _______
