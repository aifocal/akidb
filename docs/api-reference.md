# AkiDB REST API Reference

**Version:** 0.4.0
**Base URL:** `http://localhost:8080`
**Last Updated:** 2025-11-03

---

## Table of Contents

1. [Authentication](#authentication)
2. [Collections API](#collections-api)
3. [Vectors API](#vectors-api)
4. [Search API](#search-api)
5. [Health & Monitoring](#health--monitoring)
6. [Error Codes](#error-codes)

---

## Authentication

Currently, AkiDB supports **API key authentication** (optional).

**Header:**
```
Authorization: Bearer <api-key>
```

Set API key via environment variable:
```bash
AKIDB_API_KEY=your-secret-key-here
```

If no API key is set, authentication is disabled (development mode).

---

## Collections API

### List Collections

**GET `/collections`**

List all collections in the database.

**Response:**
```json
{
  "collections": [
    {
      "name": "products",
      "vector_dim": 768,
      "distance": "Cosine",
      "total_vectors": 10000,
      "created_at": "2025-01-15T10:30:00Z"
    }
  ]
}
```

**Status Codes:**
- `200 OK` - Success

---

### Create Collection

**POST `/collections`**

Create a new collection with specified vector dimension and distance metric.

**Request:**
```json
{
  "name": "products",
  "vector_dim": 768,
  "distance": "Cosine"
}
```

**Parameters:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Collection name (alphanumeric, max 255 chars) |
| `vector_dim` | integer | Yes | Vector dimension (1-4096) |
| `distance` | string | Yes | Distance metric: `Cosine`, `Euclidean`, `DotProduct` |

**Response:**
```json
{
  "name": "products",
  "vector_dim": 768,
  "distance": "Cosine",
  "created_at": "2025-01-15T10:30:00Z"
}
```

**Status Codes:**
- `201 Created` - Collection created successfully
- `400 Bad Request` - Invalid parameters
- `409 Conflict` - Collection already exists

**Example:**
```bash
curl -X POST http://localhost:8080/collections \
  -H "Content-Type: application/json" \
  -d '{
    "name": "products",
    "vector_dim": 768,
    "distance": "Cosine"
  }'
```

---

### Get Collection

**GET `/collections/:name`**

Get details about a specific collection.

**Response:**
```json
{
  "name": "products",
  "vector_dim": 768,
  "distance": "Cosine",
  "total_vectors": 10000,
  "segments": 5,
  "created_at": "2025-01-15T10:30:00Z",
  "updated_at": "2025-01-15T12:00:00Z"
}
```

**Status Codes:**
- `200 OK` - Success
- `404 Not Found` - Collection does not exist

---

### Delete Collection

**DELETE `/collections/:name`**

Delete a collection and all its vectors.

**Response:**
```json
{
  "message": "Collection 'products' deleted successfully"
}
```

**Status Codes:**
- `200 OK` - Collection deleted
- `404 Not Found` - Collection does not exist

**Example:**
```bash
curl -X DELETE http://localhost:8080/collections/products
```

---

## Vectors API

### Insert Vectors

**POST `/collections/:name/vectors`**

Insert one or more vectors into a collection.

**Request:**
```json
{
  "vectors": [
    {
      "id": "product_1",
      "vector": [0.1, 0.2, 0.3, ...],  // 768 dimensions
      "payload": {
        "name": "Laptop",
        "price": 999.99,
        "category": "Electronics"
      }
    },
    {
      "id": "product_2",
      "vector": [0.4, 0.5, 0.6, ...],
      "payload": {
        "name": "Mouse",
        "price": 29.99,
        "category": "Accessories"
      }
    }
  ]
}
```

**Parameters:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `vectors` | array | Yes | Array of vectors to insert (max 1000 per request) |
| `vectors[].id` | string | Yes | Unique vector ID |
| `vectors[].vector` | array[float] | Yes | Vector embeddings (must match collection dimension) |
| `vectors[].payload` | object | No | Arbitrary JSON metadata |

**Response:**
```json
{
  "inserted": 2,
  "ids": ["product_1", "product_2"]
}
```

**Status Codes:**
- `201 Created` - Vectors inserted
- `400 Bad Request` - Invalid vector dimension or payload
- `404 Not Found` - Collection does not exist

**Example:**
```bash
curl -X POST http://localhost:8080/collections/products/vectors \
  -H "Content-Type: application/json" \
  -d '{
    "vectors": [
      {
        "id": "product_1",
        "vector": [0.1, 0.2, 0.3, ...],
        "payload": {"name": "Laptop", "price": 999.99}
      }
    ]
  }'
```

---

## Search API

### Search Vectors

**POST `/collections/:name/search`**

Perform vector similarity search.

**Request:**
```json
{
  "vector": [0.1, 0.2, 0.3, ...],  // Query vector (768-dim)
  "top_k": 10,
  "filter": {
    "must": [
      {"key": "category", "value": "Electronics"}
    ],
    "must_not": [
      {"key": "price", "value": {"gt": 1000}}
    ]
  }
}
```

**Parameters:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `vector` | array[float] | Yes | Query vector (must match collection dimension) |
| `top_k` | integer | No | Number of results to return (default: 10, max: 1000) |
| `filter` | object | No | Payload filter (see [Filter Syntax](#filter-syntax)) |

**Response:**
```json
{
  "results": [
    {
      "id": "product_1",
      "score": 0.95,
      "payload": {
        "name": "Laptop",
        "price": 999.99,
        "category": "Electronics"
      }
    },
    {
      "id": "product_3",
      "score": 0.89,
      "payload": {
        "name": "Tablet",
        "price": 599.99,
        "category": "Electronics"
      }
    }
  ],
  "took_ms": 12
}
```

**Status Codes:**
- `200 OK` - Search successful
- `400 Bad Request` - Invalid query vector or filter
- `404 Not Found` - Collection does not exist

**Example:**
```bash
curl -X POST http://localhost:8080/collections/products/search \
  -H "Content-Type: application/json" \
  -d '{
    "vector": [0.1, 0.2, 0.3, ...],
    "top_k": 5,
    "filter": {
      "must": [
        {"key": "category", "value": "Electronics"}
      ]
    }
  }'
```

---

### Batch Search

**POST `/collections/:name/batch-search`**

Perform multiple searches in a single request (optimized with parallel execution).

**Request:**
```json
{
  "queries": [
    {
      "vector": [0.1, 0.2, ...],
      "top_k": 10
    },
    {
      "vector": [0.4, 0.5, ...],
      "top_k": 5,
      "filter": {
        "must": [{"key": "category", "value": "Electronics"}]
      }
    }
  ]
}
```

**Response:**
```json
{
  "results": [
    {
      "results": [
        {"id": "product_1", "score": 0.95, "payload": {...}},
        {"id": "product_2", "score": 0.89, "payload": {...}}
      ],
      "took_ms": 10
    },
    {
      "results": [
        {"id": "product_3", "score": 0.92, "payload": {...}}
      ],
      "took_ms": 8
    }
  ]
}
```

**Status Codes:**
- `200 OK` - Batch search successful
- `400 Bad Request` - Invalid queries

---

### Filter Syntax

**Supported Operators:**

| Operator | Description | Example |
|----------|-------------|---------|
| `eq` | Equals | `{"key": "category", "value": "Electronics"}` |
| `ne` | Not equals | `{"key": "status", "value": {"ne": "deleted"}}` |
| `gt` | Greater than | `{"key": "price", "value": {"gt": 100}}` |
| `gte` | Greater than or equal | `{"key": "rating", "value": {"gte": 4.5}}` |
| `lt` | Less than | `{"key": "stock", "value": {"lt": 10}}` |
| `lte` | Less than or equal | `{"key": "discount", "value": {"lte": 50}}` |
| `in` | In array | `{"key": "tags", "value": {"in": ["new", "featured"]}}` |
| `contains` | String contains | `{"key": "description", "value": {"contains": "laptop"}}` |

**Boolean Operators:**

```json
{
  "must": [
    {"key": "category", "value": "Electronics"},
    {"key": "price", "value": {"gte": 500, "lte": 1500}}
  ],
  "must_not": [
    {"key": "status", "value": "out_of_stock"}
  ],
  "should": [
    {"key": "brand", "value": {"in": ["Apple", "Samsung"]}}
  ]
}
```

---

## Health & Monitoring

### Liveness Probe

**GET `/health/live`**

Check if the service is alive (for Kubernetes liveness probe).

**Response:**
```
HTTP/1.1 200 OK
```

**Status Codes:**
- `200 OK` - Service is running

---

### Readiness Probe

**GET `/health/ready`**

Check if the service is ready to handle traffic (for Kubernetes readiness probe).

**Response:**
```
HTTP/1.1 200 OK
```

or

```
HTTP/1.1 503 Service Unavailable
Storage backend not ready
```

**Status Codes:**
- `200 OK` - Service is ready
- `503 Service Unavailable` - Dependencies (S3, WAL) are not healthy

---

### Detailed Health

**GET `/health`**

Get detailed health status with component-level information.

**Response:**
```json
{
  "status": "healthy",
  "version": "0.4.0",
  "uptime_seconds": 3600,
  "components": {
    "storage": {
      "status": "healthy",
      "message": "Storage backend operational"
    },
    "wal": {
      "status": "healthy",
      "message": "WAL operational"
    },
    "index": {
      "status": "healthy",
      "message": "Index provider operational"
    }
  }
}
```

**Status Values:**
- `healthy` - All systems operational
- `degraded` - Some non-critical issues (e.g., circuit breaker open)
- `unhealthy` - Critical failures (service should not receive traffic)

---

### Prometheus Metrics

**GET `/metrics`**

Export Prometheus metrics in text format.

**Response:**
```
# HELP akidb_api_requests_total Total number of API requests
# TYPE akidb_api_requests_total counter
akidb_api_requests_total{method="POST",endpoint="/collections/products/search",status="200"} 1542

# HELP akidb_api_request_duration_seconds API request duration in seconds
# TYPE akidb_api_request_duration_seconds histogram
akidb_api_request_duration_seconds_bucket{method="POST",endpoint="/search",le="0.005"} 1234
akidb_api_request_duration_seconds_bucket{method="POST",endpoint="/search",le="0.01"} 1450
...
```

See [Deployment Guide - Monitoring](deployment-production.md#monitoring--observability) for metric details.

---

## Error Codes

### Standard Error Response

```json
{
  "error": {
    "code": "COLLECTION_NOT_FOUND",
    "message": "Collection 'products' does not exist",
    "details": {}
  }
}
```

### Error Codes

| HTTP Status | Error Code | Description |
|-------------|------------|-------------|
| 400 | `INVALID_REQUEST` | Malformed JSON or missing required fields |
| 400 | `INVALID_VECTOR_DIMENSION` | Vector dimension mismatch |
| 400 | `INVALID_FILTER` | Invalid filter syntax |
| 401 | `UNAUTHORIZED` | Missing or invalid API key |
| 404 | `COLLECTION_NOT_FOUND` | Collection does not exist |
| 404 | `VECTOR_NOT_FOUND` | Vector ID does not exist |
| 409 | `COLLECTION_ALREADY_EXISTS` | Collection name already in use |
| 429 | `RATE_LIMIT_EXCEEDED` | Too many requests |
| 500 | `INTERNAL_ERROR` | Unexpected server error |
| 503 | `SERVICE_UNAVAILABLE` | Storage backend or dependencies unavailable |

---

## Rate Limiting

**Default Limits:**
- 1000 requests/min per IP (global)
- 100 requests/min per collection (write operations)

**Headers:**
```
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 542
X-RateLimit-Reset: 1704124800
```

Rate limit exceeded response:
```json
{
  "error": {
    "code": "RATE_LIMIT_EXCEEDED",
    "message": "Rate limit exceeded. Please retry after 60 seconds.",
    "retry_after_seconds": 60
  }
}
```

---

## Client SDKs

**Official SDKs (Planned for Phase 7):**
- Python: `pip install akidb`
- TypeScript/Node.js: `npm install @aifocal/akidb`
- Go: `go get github.com/aifocal/akidb-go`

**Community SDKs:**
- Rust: Use `reqwest` directly
- Swift: Use `URLSession` directly

---

## Examples

### End-to-End Workflow

```bash
# 1. Create collection
curl -X POST http://localhost:8080/collections \
  -H "Content-Type: application/json" \
  -d '{"name": "products", "vector_dim": 768, "distance": "Cosine"}'

# 2. Insert vectors (batch)
curl -X POST http://localhost:8080/collections/products/vectors \
  -H "Content-Type: application/json" \
  -d '{
    "vectors": [
      {
        "id": "laptop_1",
        "vector": [0.1, 0.2, ...],
        "payload": {"name": "Dell XPS 15", "price": 1299}
      },
      {
        "id": "laptop_2",
        "vector": [0.3, 0.4, ...],
        "payload": {"name": "MacBook Pro", "price": 2499}
      }
    ]
  }'

# 3. Search
curl -X POST http://localhost:8080/collections/products/search \
  -H "Content-Type: application/json" \
  -d '{
    "vector": [0.15, 0.25, ...],
    "top_k": 10,
    "filter": {
      "must": [{"key": "price", "value": {"lte": 2000}}]
    }
  }'

# 4. Check health
curl http://localhost:8080/health

# 5. View metrics
curl http://localhost:8080/metrics
```

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 0.4.0 | 2025-11-03 | Added OpenTelemetry tracing, improved health checks |
| 0.3.0 | 2025-10-15 | HNSW index, WAL implementation |
| 0.2.0 | 2025-09-20 | Batch search API, filter support |
| 0.1.0 | 2025-08-10 | Initial release |

---

**Support:** https://github.com/aifocal/akidb/issues
**Changelog:** https://github.com/aifocal/akidb/blob/main/CHANGELOG.md
