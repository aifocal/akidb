# Phase 7: Enterprise Scale

**Status**: Planning
**Timeline**: 16-20 weeks
**Effort**: 320-400 hours
**Goal**: Production-grade features for large-scale deployments

---

## Executive Summary

Phase 7 transforms AkiDB from a capable offline vector database into an **enterprise-grade platform** supporting:
- **Multi-tenancy**: Namespace isolation for hundreds of organizations
- **RBAC**: Role-based access control with MinIO policy integration
- **Billion-scale indices**: DiskANN for memory-efficient vector search
- **Distributed queries**: Sharding and query coordination across nodes
- **Client SDKs**: Python, TypeScript, and Go SDKs for developer productivity
- **Advanced caching**: Materialized views and intelligent cache invalidation

---

## Architecture Overview

### Current (Phase 6)
```
┌─────────────┐
│  akidb-api  │ ← Single-tenant, in-memory cache
└──────┬──────┘
       │
┌──────▼──────┐
│    MinIO    │ ← Simple bucket structure
└─────────────┘
```

### Target (Phase 7)
```
┌───────────────────────────────────────────────────┐
│            Load Balancer / API Gateway            │
└───────────┬───────────────────────────────────────┘
            │
    ┌───────┴────────┬─────────────┬─────────────┐
    │                │             │             │
┌───▼────┐      ┌───▼────┐    ┌───▼────┐    ┌───▼────┐
│ Node 1 │      │ Node 2 │    │ Node 3 │    │ Node N │
│  API   │      │  API   │    │  API   │    │  API   │
│ +RBAC  │      │ +RBAC  │    │ +RBAC  │    │ +RBAC  │
│ +Cache │      │ +Cache │    │ +Cache │    │ +Cache │
└───┬────┘      └───┬────┘    └───┬────┘    └───┬────┘
    │                │             │             │
    └────────────────┴─────────┬───┴─────────────┘
                                │
                    ┌───────────▼───────────┐
                    │  Distributed MinIO    │
                    │  Multi-tenant buckets │
                    │  tenant1/, tenant2/   │
                    └───────────────────────┘
```

---

## Milestones (18 Total)

### M1-M3: Multi-Tenancy (Weeks 1-3)
**Goal**: Namespace isolation for multiple organizations

#### M1: Tenant Management API
- Tenant CRUD operations (create, read, update, delete)
- Tenant metadata storage in MinIO (`tenants/{tenant_id}/manifest.json`)
- Unique tenant IDs (UUIDv4)
- Tenant quotas (storage, API rate limits)

**Deliverables**:
- `POST /tenants` - Create tenant
- `GET /tenants/{id}` - Get tenant info
- `PUT /tenants/{id}` - Update tenant
- `DELETE /tenants/{id}` - Delete tenant (soft delete)
- `TenantDescriptor` struct with metadata

#### M2: Namespace Isolation
- Collection scoping by tenant (`{tenant_id}/{collection_name}`)
- MinIO bucket structure: `akidb/{tenant_id}/collections/{collection}/...`
- Path rewriting middleware for tenant context
- Tenant context propagation through request lifecycle

**Deliverables**:
- Tenant-aware storage backend
- Path rewriting: `GET /collections/foo` → `s3://akidb/{tenant_id}/collections/foo/`
- Request middleware: `TenantContextMiddleware`

#### M3: Tenant Resource Limits
- Storage quotas per tenant
- API rate limiting per tenant (requests/second)
- Collection limits per tenant
- Vector count limits per tenant

**Deliverables**:
- `TenantQuota` struct
- Quota enforcement middleware
- Metrics: `akidb_tenant_storage_bytes`, `akidb_tenant_api_requests_total`

---

### M4-M6: RBAC (Weeks 4-6)
**Goal**: Role-based access control with MinIO policies

#### M4: User & Role Management
- User authentication (API keys, JWT)
- Role definitions: `admin`, `editor`, `viewer`
- Custom roles with permissions matrix
- User-role assignment

**Deliverables**:
- `POST /users` - Create user
- `POST /roles` - Create role
- `POST /users/{id}/roles` - Assign role
- `User`, `Role`, `Permission` structs

#### M5: MinIO Policy Integration
- Generate MinIO policies from AkiDB roles
- Sync roles to MinIO policies via Admin API
- Policy enforcement at storage layer
- Audit logging for policy changes

**Deliverables**:
- `MinIOPolicyGenerator` trait
- Policy sync: `akidb_sync_policies` command
- MinIO Admin API client

#### M6: Authentication & Authorization Middleware
- JWT token validation
- API key authentication
- Role-based endpoint protection
- Per-collection access control

**Deliverables**:
- `AuthMiddleware` (JWT + API key)
- `RbacMiddleware` (role checking)
- `PermissionCheck` trait for endpoints

---

### M7-M9: Advanced Query Caching (Weeks 7-9)
**Goal**: Materialized views and intelligent cache invalidation

#### M7: Materialized Views
- Pre-computed query results
- Automatic view refresh on data changes
- View definitions in collection manifest
- TTL-based expiration

**Deliverables**:
- `MaterializedView` struct
- `POST /collections/{name}/views` - Create view
- Background refresh job
- View storage in MinIO

#### M8: Multi-Level Cache
- L1: In-memory LRU (moka)
- L2: Redis cluster (distributed)
- L3: MinIO (persistent)
- Cache coherency protocol

**Deliverables**:
- `CacheBackend` trait
- `RedisCache` implementation
- Cache key strategy
- Metrics: cache hit rates per level

#### M9: Intelligent Cache Invalidation
- Fine-grained invalidation (by collection, segment, query)
- Write-through invalidation
- Bloom filters for query cache lookup
- Cache warming on startup

**Deliverables**:
- `CacheInvalidator` trait
- Write hooks for invalidation
- `BloomFilterIndex` for cache keys

---

### M10-M12: DiskANN (Weeks 10-12)
**Goal**: Billion-scale vector search with memory efficiency

#### M10: DiskANN Index Provider
- Vamana graph construction
- SSD-backed index storage
- Memory-mapped file access
- Beam search for queries

**Deliverables**:
- `DiskAnnProvider` implementing `IndexProvider`
- Vamana graph builder
- Beam search algorithm
- Contract tests

#### M11: Index Compression & Quantization
- Product quantization (PQ)
- Scalar quantization (SQ)
- Compressed vectors in memory
- Approximate distance computation

**Deliverables**:
- `ProductQuantizer` struct
- `ScalarQuantizer` struct
- Compressed index format
- Benchmarks: memory reduction vs accuracy

#### M12: Streaming Index Build
- Out-of-core index construction
- Incremental graph updates
- Parallel build using rayon
- Progress tracking

**Deliverables**:
- Streaming Vamana builder
- Incremental insert API
- Build progress metrics
- Memory-bounded construction

---

### M13-M15: Distributed Query Coordination (Weeks 13-15)
**Goal**: Sharding and distributed query execution

#### M13: Data Sharding
- Consistent hashing for shard assignment
- Replication factor configuration
- Shard rebalancing on node join/leave
- Virtual nodes for load balancing

**Deliverables**:
- `ShardRouter` struct
- Consistent hash ring
- Shard assignment algorithm
- Rebalancing logic

#### M14: Distributed Query Planner
- Query decomposition into shard queries
- Parallel shard execution
- Result merging (top-k aggregation)
- Fault tolerance (retry on shard failure)

**Deliverables**:
- `DistributedQueryPlanner` struct
- Shard query execution
- Top-k merge algorithm
- Timeout & retry logic

#### M15: Node Discovery & Health
- Service registry (etcd or Consul)
- Health checks (liveness/readiness)
- Node metadata (shard assignments)
- Cluster state synchronization

**Deliverables**:
- `NodeRegistry` trait
- `EtcdRegistry` implementation
- Health check endpoint: `/cluster/health`
- Cluster info endpoint: `/cluster/nodes`

---

### M16-M18: Client SDKs (Weeks 16-18)
**Goal**: Developer-friendly SDKs for Python, TypeScript, and Go

#### M16: Python SDK
- pip installable: `akidb-python`
- Async client using `httpx`
- Type hints and docstrings
- Collection/vector CRUD
- Search with filters

**Deliverables**:
- `akidb` Python package
- `AkiDBClient` class
- PyPI package
- Examples and Jupyter notebooks

#### M17: TypeScript SDK
- npm installable: `@akidb/client`
- Promise-based API
- TypeScript type definitions
- Browser and Node.js support
- React hooks (optional)

**Deliverables**:
- `@akidb/client` npm package
- `AkiDBClient` class
- Type definitions
- Examples and CodeSandbox demos

#### M18: Go SDK
- go-gettable: `github.com/aifocal/akidb-go`
- Idiomatic Go API
- Context support
- Connection pooling
- gRPC option (future)

**Deliverables**:
- `akidb-go` module
- `Client` struct
- Examples and godoc
- Integration tests

---

## Technical Specifications

### Multi-Tenancy Design

#### Tenant Manifest
```json
{
  "tenant_id": "tenant_abc123",
  "name": "Acme Corp",
  "created_at": "2025-01-15T10:00:00Z",
  "quotas": {
    "max_storage_bytes": 1099511627776,
    "max_collections": 100,
    "max_vectors_per_collection": 10000000,
    "api_rate_limit_per_second": 1000
  },
  "metadata": {
    "contact_email": "admin@acme.com",
    "billing_plan": "enterprise"
  }
}
```

#### Storage Path Structure
```
s3://akidb/
├── tenants/
│   ├── tenant_abc123/
│   │   ├── manifest.json
│   │   └── collections/
│   │       ├── products/
│   │       │   ├── manifest.json
│   │       │   └── segments/
│   │       │       ├── seg_001.v1
│   │       │       └── seg_002.v1
│   │       └── articles/
│   │           └── ...
│   └── tenant_xyz789/
│       └── ...
└── system/
    ├── users.json
    ├── roles.json
    └── policies/
```

---

### RBAC Design

#### Permission Matrix
| Permission | Admin | Editor | Viewer |
|------------|-------|--------|--------|
| collections.create | ✅ | ✅ | ❌ |
| collections.read | ✅ | ✅ | ✅ |
| collections.update | ✅ | ✅ | ❌ |
| collections.delete | ✅ | ❌ | ❌ |
| vectors.insert | ✅ | ✅ | ❌ |
| vectors.search | ✅ | ✅ | ✅ |
| vectors.delete | ✅ | ✅ | ❌ |
| tenants.manage | ✅ | ❌ | ❌ |
| users.manage | ✅ | ❌ | ❌ |

#### MinIO Policy Generation
```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "s3:GetObject",
        "s3:PutObject"
      ],
      "Resource": [
        "arn:aws:s3:::akidb/tenants/tenant_abc123/*"
      ]
    }
  ]
}
```

---

### DiskANN Design

#### Vamana Graph Structure
```rust
pub struct VamanaGraph {
    /// Adjacency list: node_id → neighbor_ids
    pub edges: Vec<Vec<u32>>,
    /// Max degree (R parameter)
    pub max_degree: usize,
    /// Alpha parameter for pruning
    pub alpha: f32,
    /// Node positions (for distance computation)
    pub nodes: Vec<CompressedVector>,
}

pub struct CompressedVector {
    /// Product quantization codes
    pub pq_codes: Vec<u8>,
    /// Original vector ID
    pub id: String,
}
```

#### Beam Search Algorithm
```rust
pub fn beam_search(
    &self,
    query: &[f32],
    k: usize,
    beam_width: usize,
) -> Vec<SearchResult> {
    let mut candidates = BinaryHeap::new();
    let mut visited = HashSet::new();

    // Start from entry point
    candidates.push(entry_node);

    while !candidates.is_empty() && candidates.len() < beam_width {
        let node = candidates.pop();
        if visited.contains(&node) {
            continue;
        }
        visited.insert(node);

        // Expand neighbors
        for neighbor in self.edges[node].iter() {
            if !visited.contains(neighbor) {
                candidates.push(*neighbor);
            }
        }
    }

    // Return top-k from candidates
    candidates.into_sorted_vec().truncate(k)
}
```

---

### Distributed Query Coordination

#### Shard Assignment
```rust
pub struct ShardRouter {
    /// Consistent hash ring
    ring: ConsistentHashRing,
    /// Replication factor
    replication_factor: usize,
    /// Node list
    nodes: Vec<NodeInfo>,
}

impl ShardRouter {
    pub fn get_shards(&self, collection: &str) -> Vec<ShardId> {
        let hash = self.hash(collection);
        self.ring.get_nodes(hash, self.replication_factor)
    }
}
```

#### Distributed Search
```
1. Client → API Gateway: search(query, k=10)
2. Gateway → Query Planner: decompose
3. Planner → Shard1, Shard2, Shard3: parallel queries
4. Shards → Planner: partial results (k=10 each)
5. Planner: merge top-k from 3 shards
6. Planner → Gateway: final top-10
7. Gateway → Client: results
```

---

## API Examples

### Multi-Tenancy

#### Create Tenant
```bash
curl -X POST http://localhost:8080/tenants \
  -H "Authorization: Bearer admin_token" \
  -d '{
    "name": "Acme Corp",
    "quotas": {
      "max_storage_bytes": 1099511627776,
      "max_collections": 100
    }
  }'

# Response:
{
  "tenant_id": "tenant_abc123",
  "name": "Acme Corp",
  "api_key": "ak_abc123...xyz"
}
```

#### Create Collection (Tenant-Scoped)
```bash
curl -X POST http://localhost:8080/collections \
  -H "X-Tenant-ID: tenant_abc123" \
  -H "Authorization: Bearer tenant_api_key" \
  -d '{
    "name": "products",
    "vector_dim": 768
  }'
```

### RBAC

#### Create User
```bash
curl -X POST http://localhost:8080/users \
  -H "Authorization: Bearer admin_token" \
  -d '{
    "email": "alice@acme.com",
    "tenant_id": "tenant_abc123",
    "role": "editor"
  }'

# Response:
{
  "user_id": "user_123",
  "api_key": "ak_user_abc...xyz"
}
```

#### Search with User Token
```bash
curl -X POST http://localhost:8080/collections/products/search \
  -H "Authorization: Bearer user_api_key" \
  -d '{
    "vector": [0.1, 0.2, ...],
    "k": 10
  }'
```

### Materialized Views

#### Create View
```bash
curl -X POST http://localhost:8080/collections/products/views \
  -H "Authorization: Bearer api_key" \
  -d '{
    "name": "popular_products",
    "query": {
      "filter": {"category": "electronics"},
      "k": 100
    },
    "refresh_interval_secs": 300
  }'
```

#### Query View
```bash
curl -X GET http://localhost:8080/collections/products/views/popular_products \
  -H "Authorization: Bearer api_key"

# Returns pre-computed top-100 electronics
```

### DiskANN

#### Create DiskANN Index
```bash
curl -X POST http://localhost:8080/collections/large_dataset/index \
  -H "Authorization: Bearer api_key" \
  -d '{
    "type": "diskann",
    "parameters": {
      "max_degree": 64,
      "alpha": 1.2,
      "build_memory_gb": 8,
      "quantization": "pq",
      "pq_subvectors": 64
    }
  }'
```

---

## Client SDK Examples

### Python SDK
```python
from akidb import AkiDBClient

# Initialize client
client = AkiDBClient(
    endpoint="http://localhost:8080",
    api_key="ak_abc123...xyz",
    tenant_id="tenant_abc123"
)

# Create collection
collection = client.create_collection(
    name="products",
    vector_dim=768,
    distance="cosine"
)

# Insert vectors
client.insert_vectors(
    collection="products",
    vectors=[
        {"id": "prod_1", "vector": [0.1, 0.2, ...], "payload": {"name": "iPhone"}},
        {"id": "prod_2", "vector": [0.3, 0.4, ...], "payload": {"name": "MacBook"}}
    ]
)

# Search
results = client.search(
    collection="products",
    vector=[0.15, 0.25, ...],
    k=10,
    filter={"category": "electronics"}
)

for result in results:
    print(f"{result.id}: {result.score} - {result.payload}")
```

### TypeScript SDK
```typescript
import { AkiDBClient } from '@akidb/client';

// Initialize client
const client = new AkiDBClient({
  endpoint: 'http://localhost:8080',
  apiKey: 'ak_abc123...xyz',
  tenantId: 'tenant_abc123'
});

// Create collection
await client.createCollection({
  name: 'products',
  vectorDim: 768,
  distance: 'cosine'
});

// Insert vectors
await client.insertVectors('products', [
  { id: 'prod_1', vector: [0.1, 0.2, ...], payload: { name: 'iPhone' } },
  { id: 'prod_2', vector: [0.3, 0.4, ...], payload: { name: 'MacBook' } }
]);

// Search
const results = await client.search('products', {
  vector: [0.15, 0.25, ...],
  k: 10,
  filter: { category: 'electronics' }
});

results.forEach(result => {
  console.log(`${result.id}: ${result.score} - ${result.payload.name}`);
});
```

### Go SDK
```go
package main

import (
    "context"
    "fmt"
    "github.com/aifocal/akidb-go"
)

func main() {
    // Initialize client
    client := akidb.NewClient(&akidb.Config{
        Endpoint: "http://localhost:8080",
        APIKey:   "ak_abc123...xyz",
        TenantID: "tenant_abc123",
    })

    // Create collection
    _, err := client.CreateCollection(context.Background(), &akidb.Collection{
        Name:      "products",
        VectorDim: 768,
        Distance:  akidb.Cosine,
    })
    if err != nil {
        panic(err)
    }

    // Insert vectors
    err = client.InsertVectors(context.Background(), "products", []*akidb.Vector{
        {ID: "prod_1", Vector: []float32{0.1, 0.2, ...}, Payload: map[string]any{"name": "iPhone"}},
        {ID: "prod_2", Vector: []float32{0.3, 0.4, ...}, Payload: map[string]any{"name": "MacBook"}},
    })
    if err != nil {
        panic(err)
    }

    // Search
    results, err := client.Search(context.Background(), "products", &akidb.SearchRequest{
        Vector: []float32{0.15, 0.25, ...},
        K:      10,
        Filter: map[string]any{"category": "electronics"},
    })
    if err != nil {
        panic(err)
    }

    for _, result := range results {
        fmt.Printf("%s: %.3f - %v\n", result.ID, result.Score, result.Payload)
    }
}
```

---

## Performance Targets

### Multi-Tenancy
- **Tenant isolation overhead**: < 5% latency increase
- **Tenants per node**: 1,000+
- **Collections per tenant**: 100+
- **Quota enforcement**: < 1ms per request

### RBAC
- **Auth check latency**: < 1ms (cached)
- **JWT validation**: < 2ms
- **Policy sync time**: < 10s for 1,000 policies

### Query Caching
- **L1 cache hit rate**: > 80%
- **L2 cache hit rate**: > 60%
- **Materialized view latency**: < 10ms (vs 100ms+ for live query)
- **Cache invalidation latency**: < 50ms

### DiskANN
- **Index memory**: 10-20 bytes per vector (vs 4KB for HNSW)
- **Search latency (1B vectors)**: < 10ms @ 95% recall
- **Build time**: < 2 hours for 1B vectors
- **Throughput**: 10,000 QPS per node

### Distributed Queries
- **Shard routing latency**: < 1ms
- **Query fan-out**: 3-5 shards per query
- **Result merging**: < 5ms for top-100
- **Cluster rebalancing**: < 5 minutes

---

## Testing Strategy

### Unit Tests
- Tenant CRUD operations
- RBAC permission checking
- Cache invalidation logic
- DiskANN graph construction
- Shard routing

### Integration Tests
- Multi-tenant isolation (no cross-tenant access)
- End-to-end RBAC flow
- Cache coherency across nodes
- DiskANN index build & search
- Distributed query execution

### Performance Tests
- 1,000 tenant load test
- 1M QPS with RBAC enabled
- DiskANN 1B vector benchmark
- Distributed query latency (10 nodes)
- Cache hit rate measurement

### Contract Tests
- `DiskAnnProvider` implements `IndexProvider`
- Client SDKs match REST API spec
- MinIO policy generation correctness

---

## Dependencies

### New Rust Crates
- `diskann` - DiskANN implementation (or custom)
- `etcd-client` - Service registry
- `jsonwebtoken` - JWT authentication
- `redis` - Distributed cache
- `tower-governor` - Rate limiting

### Client SDK Dependencies
- **Python**: `httpx`, `pydantic`, `typing_extensions`
- **TypeScript**: `axios`, `zod`
- **Go**: `net/http`, `encoding/json`

---

## Migration Path

### From Phase 6 to Phase 7

#### 1. Enable Multi-Tenancy (Opt-in)
```bash
# Set environment variable
export AKIDB_MULTI_TENANCY=true
export AKIDB_DEFAULT_TENANT=default

# All existing collections go under "default" tenant
# New tenants can be created via API
```

#### 2. Enable RBAC (Opt-in)
```bash
export AKIDB_RBAC_ENABLED=true
export AKIDB_ADMIN_API_KEY=admin_key_here

# Existing API calls require X-API-Key header
```

#### 3. Enable DiskANN (Per-collection)
```bash
# Existing collections continue using HNSW
# New collections can opt-in to DiskANN
curl -X POST /collections -d '{"name": "large", "index_type": "diskann"}'
```

---

## Risks & Mitigations

### Risk 1: DiskANN Complexity
- **Risk**: DiskANN implementation is complex and error-prone
- **Mitigation**: Use existing library (diskann-rs) or hire expert consultant
- **Fallback**: Skip DiskANN, use quantized HNSW instead

### Risk 2: Distributed Coordination Overhead
- **Risk**: Etcd/Consul adds operational complexity
- **Mitigation**: Make distributed mode optional (default: single-node)
- **Fallback**: Use static node configuration

### Risk 3: Client SDK Maintenance Burden
- **Risk**: 3 SDKs (Python/TS/Go) require ongoing maintenance
- **Mitigation**: Auto-generate SDKs from OpenAPI spec
- **Fallback**: Community-maintained SDKs

### Risk 4: Multi-Tenancy Performance Impact
- **Risk**: Tenant isolation adds latency overhead
- **Mitigation**: Benchmark early, optimize hot paths
- **Fallback**: Document performance trade-offs

---

## Success Criteria

### Phase 7 Complete When:
- ✅ Multi-tenancy supports 1,000+ tenants per node
- ✅ RBAC integrated with MinIO policies
- ✅ Materialized views reduce query latency by 10x
- ✅ DiskANN handles 1B vectors with < 10ms latency
- ✅ Distributed queries work across 10+ nodes
- ✅ Python, TypeScript, Go SDKs published
- ✅ Documentation covers all enterprise features
- ✅ 90%+ test coverage maintained

---

## Timeline

| Milestone | Weeks | Effort | Dependencies |
|-----------|-------|--------|--------------|
| M1-M3: Multi-Tenancy | 3 | 60h | None |
| M4-M6: RBAC | 3 | 60h | M1-M3 |
| M7-M9: Caching | 3 | 60h | M1-M3 |
| M10-M12: DiskANN | 3 | 80h | None (parallel) |
| M13-M15: Distributed | 3 | 80h | M1-M3 |
| M16-M18: SDKs | 2 | 40h | M4-M6 |
| **Total** | **17 weeks** | **380 hours** | |

---

**Document Version**: 1.0
**Last Updated**: 2025-11-03
**Owner**: AkiDB Team
