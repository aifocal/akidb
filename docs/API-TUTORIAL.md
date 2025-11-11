# AkiDB 2.0 REST API Tutorial

A comprehensive guide to using the AkiDB REST API for vector database operations.

## Table of Contents

- [Getting Started](#getting-started)
- [Authentication](#authentication)
- [Core Concepts](#core-concepts)
- [Quick Start Examples](#quick-start-examples)
- [Collection Management](#collection-management)
- [Vector Operations](#vector-operations)
- [Common Workflows](#common-workflows)
- [Error Handling](#error-handling)
- [Performance Tips](#performance-tips)
- [Client Libraries](#client-libraries)

---

## Getting Started

### Prerequisites

- AkiDB server running on `http://localhost:8080`
- Basic understanding of vector embeddings and similarity search
- HTTP client (curl, Python requests, JavaScript axios, etc.)

### Base URL

```
http://localhost:8080
```

API endpoints are prefixed with `/api/v1`.

### Starting the Server

```bash
# From the project root
cargo run -p akidb-rest --release

# Or using Docker
docker-compose up akidb-rest
```

### Health Check

Verify the server is running:

```bash
curl http://localhost:8080/health
```

**Response:**
```json
{
  "status": "healthy",
  "version": "0.1.0"
}
```

---

## Authentication

**RC1 Note:** The current release (RC1) operates in single-tenant mode without authentication. Multi-tenant RBAC will be available in future releases.

---

## Core Concepts

### Collections

A **collection** is a named container for vectors with:
- **Dimension**: Fixed vector dimensionality (16-4096)
- **Metric**: Distance metric for similarity search
  - `cosine`: Cosine similarity (0.0-1.0, higher is more similar)
  - `l2`: Euclidean distance (0.0+, lower is more similar)
  - `dot`: Dot product (higher is more similar)
- **HNSW Index**: Automatic indexing for fast approximate nearest neighbor search

### Documents

A **vector document** contains:
- **doc_id**: UUID v7 (time-ordered, client-generated)
- **vector**: Dense embedding array (f32)
- **external_id**: Optional user-defined identifier (for mapping to external systems)
- **inserted_at**: Automatic timestamp

### UUIDs

AkiDB uses **UUID v7** (time-ordered) for all IDs. Generate them client-side:

**Python:**
```python
import uuid
doc_id = str(uuid.uuid7())  # Python 3.11+
```

**JavaScript:**
```javascript
import { v7 as uuidv7 } from 'uuid';
const docId = uuidv7();
```

**Command line:**
```bash
# macOS/Linux
uuidgen | tr '[:upper:]' '[:lower:]'
```

---

## Quick Start Examples

### 1. Create a Collection

```bash
curl -X POST http://localhost:8080/api/v1/collections \
  -H "Content-Type: application/json" \
  -d '{
    "name": "text_embeddings",
    "dimension": 384,
    "metric": "cosine",
    "embedding_model": "all-MiniLM-L6-v2"
  }'
```

**Response:**
```json
{
  "collection_id": "018f1234-5678-7abc-def0-123456789abc",
  "name": "text_embeddings",
  "dimension": 384,
  "metric": "cosine"
}
```

### 2. Insert Vectors

```bash
curl -X POST http://localhost:8080/api/v1/collections/018f1234-5678-7abc-def0-123456789abc/insert \
  -H "Content-Type: application/json" \
  -d '{
    "doc_id": "018f5678-1234-7abc-def0-111111111111",
    "external_id": "doc-001",
    "vector": [0.1, 0.2, 0.3, 0.4, 0.5]
  }'
```

**Response:**
```json
{
  "doc_id": "018f5678-1234-7abc-def0-111111111111",
  "latency_ms": 4.2
}
```

### 3. Search for Similar Vectors

```bash
curl -X POST http://localhost:8080/api/v1/collections/018f1234-5678-7abc-def0-123456789abc/query \
  -H "Content-Type: application/json" \
  -d '{
    "query_vector": [0.15, 0.25, 0.35, 0.45, 0.55],
    "top_k": 10
  }'
```

**Response:**
```json
{
  "matches": [
    {
      "doc_id": "018f5678-1234-7abc-def0-111111111111",
      "external_id": "doc-001",
      "distance": 0.98
    }
  ],
  "latency_ms": 12.5
}
```

---

## Collection Management

### Create Collection

Creates a new vector collection with specified dimension and metric.

**Endpoint:** `POST /api/v1/collections`

**curl:**
```bash
curl -X POST http://localhost:8080/api/v1/collections \
  -H "Content-Type: application/json" \
  -d '{
    "name": "image_embeddings",
    "dimension": 512,
    "metric": "l2"
  }'
```

**Python:**
```python
import requests

response = requests.post(
    "http://localhost:8080/api/v1/collections",
    json={
        "name": "image_embeddings",
        "dimension": 512,
        "metric": "l2"
    }
)

collection = response.json()
collection_id = collection["collection_id"]
print(f"Created collection: {collection_id}")
```

**JavaScript:**
```javascript
const axios = require('axios');

async function createCollection() {
  const response = await axios.post('http://localhost:8080/api/v1/collections', {
    name: 'image_embeddings',
    dimension: 512,
    metric: 'l2'
  });

  const { collection_id } = response.data;
  console.log(`Created collection: ${collection_id}`);
  return collection_id;
}
```

**Validation Rules:**
- `name`: Non-empty string, unique per database
- `dimension`: Integer between 16 and 4096
- `metric`: One of `cosine`, `l2`, `dot`

### List Collections

Retrieves all collections with metadata.

**Endpoint:** `GET /api/v1/collections`

**curl:**
```bash
curl http://localhost:8080/api/v1/collections
```

**Python:**
```python
response = requests.get("http://localhost:8080/api/v1/collections")
collections = response.json()["collections"]

for coll in collections:
    print(f"{coll['name']}: {coll['document_count']} documents")
```

**JavaScript:**
```javascript
async function listCollections() {
  const response = await axios.get('http://localhost:8080/api/v1/collections');
  const collections = response.data.collections;

  collections.forEach(coll => {
    console.log(`${coll.name}: ${coll.document_count} documents`);
  });
}
```

### Get Collection Details

Retrieves detailed information about a specific collection.

**Endpoint:** `GET /api/v1/collections/{collection_id}`

**curl:**
```bash
curl http://localhost:8080/api/v1/collections/018f1234-5678-7abc-def0-123456789abc
```

**Python:**
```python
collection_id = "018f1234-5678-7abc-def0-123456789abc"
response = requests.get(f"http://localhost:8080/api/v1/collections/{collection_id}")
collection = response.json()["collection"]

print(f"Name: {collection['name']}")
print(f"Dimension: {collection['dimension']}")
print(f"Metric: {collection['metric']}")
print(f"Documents: {collection['document_count']}")
```

**JavaScript:**
```javascript
async function getCollection(collectionId) {
  const response = await axios.get(
    `http://localhost:8080/api/v1/collections/${collectionId}`
  );
  const collection = response.data.collection;

  console.log(`Name: ${collection.name}`);
  console.log(`Dimension: ${collection.dimension}`);
  console.log(`Metric: ${collection.metric}`);
  console.log(`Documents: ${collection.document_count}`);
}
```

### Delete Collection

Permanently deletes a collection and all its vectors.

**Endpoint:** `DELETE /api/v1/collections/{collection_id}`

**curl:**
```bash
curl -X DELETE http://localhost:8080/api/v1/collections/018f1234-5678-7abc-def0-123456789abc
```

**Python:**
```python
collection_id = "018f1234-5678-7abc-def0-123456789abc"
response = requests.delete(f"http://localhost:8080/api/v1/collections/{collection_id}")

if response.status_code == 204:
    print("Collection deleted successfully")
```

**JavaScript:**
```javascript
async function deleteCollection(collectionId) {
  await axios.delete(`http://localhost:8080/api/v1/collections/${collectionId}`);
  console.log('Collection deleted successfully');
}
```

---

## Vector Operations

### Insert Vector

Inserts a single vector document into a collection.

**Endpoint:** `POST /api/v1/collections/{collection_id}/insert`

**curl:**
```bash
curl -X POST http://localhost:8080/api/v1/collections/018f1234-5678-7abc-def0-123456789abc/insert \
  -H "Content-Type: application/json" \
  -d '{
    "doc_id": "018f5678-1234-7abc-def0-111111111111",
    "external_id": "article-42",
    "vector": [0.1, 0.2, 0.3, 0.4, 0.5]
  }'
```

**Python:**
```python
import uuid

doc_id = str(uuid.uuid7())
vector = [0.1, 0.2, 0.3, 0.4, 0.5]  # Your embedding here

response = requests.post(
    f"http://localhost:8080/api/v1/collections/{collection_id}/insert",
    json={
        "doc_id": doc_id,
        "external_id": "article-42",
        "vector": vector
    }
)

result = response.json()
print(f"Inserted: {result['doc_id']} in {result['latency_ms']:.2f}ms")
```

**JavaScript:**
```javascript
import { v7 as uuidv7 } from 'uuid';

async function insertVector(collectionId, vector, externalId) {
  const docId = uuidv7();

  const response = await axios.post(
    `http://localhost:8080/api/v1/collections/${collectionId}/insert`,
    {
      doc_id: docId,
      external_id: externalId,
      vector: vector
    }
  );

  console.log(`Inserted: ${response.data.doc_id} in ${response.data.latency_ms}ms`);
  return docId;
}
```

**Batch Insert (Python):**
```python
import uuid
import concurrent.futures

def insert_document(collection_id, vector, external_id=None):
    doc_id = str(uuid.uuid7())
    response = requests.post(
        f"http://localhost:8080/api/v1/collections/{collection_id}/insert",
        json={
            "doc_id": doc_id,
            "external_id": external_id,
            "vector": vector
        }
    )
    return response.json()

# Batch insert with threading
vectors = [generate_embedding(text) for text in documents]
external_ids = [f"doc-{i}" for i in range(len(documents))]

with concurrent.futures.ThreadPoolExecutor(max_workers=10) as executor:
    futures = [
        executor.submit(insert_document, collection_id, vec, ext_id)
        for vec, ext_id in zip(vectors, external_ids)
    ]
    results = [f.result() for f in futures]

print(f"Inserted {len(results)} documents")
```

### Query Vectors (Similarity Search)

Performs vector similarity search using HNSW index.

**Endpoint:** `POST /api/v1/collections/{collection_id}/query`

**curl:**
```bash
curl -X POST http://localhost:8080/api/v1/collections/018f1234-5678-7abc-def0-123456789abc/query \
  -H "Content-Type: application/json" \
  -d '{
    "query_vector": [0.15, 0.25, 0.35, 0.45, 0.55],
    "top_k": 10
  }'
```

**Python:**
```python
query_vector = [0.15, 0.25, 0.35, 0.45, 0.55]

response = requests.post(
    f"http://localhost:8080/api/v1/collections/{collection_id}/query",
    json={
        "query_vector": query_vector,
        "top_k": 10
    }
)

results = response.json()
print(f"Query completed in {results['latency_ms']:.2f}ms")

for match in results["matches"]:
    print(f"  {match['external_id']}: {match['distance']:.4f}")
```

**JavaScript:**
```javascript
async function queryVectors(collectionId, queryVector, topK = 10) {
  const response = await axios.post(
    `http://localhost:8080/api/v1/collections/${collectionId}/query`,
    {
      query_vector: queryVector,
      top_k: topK
    }
  );

  const { matches, latency_ms } = response.data;
  console.log(`Query completed in ${latency_ms.toFixed(2)}ms`);

  matches.forEach(match => {
    console.log(`  ${match.external_id}: ${match.distance.toFixed(4)}`);
  });

  return matches;
}
```

### Get Vector Document

Retrieves a specific document by doc_id.

**Endpoint:** `GET /api/v1/collections/{collection_id}/docs/{doc_id}`

**curl:**
```bash
curl http://localhost:8080/api/v1/collections/018f1234-5678-7abc-def0-123456789abc/docs/018f5678-1234-7abc-def0-111111111111
```

**Python:**
```python
response = requests.get(
    f"http://localhost:8080/api/v1/collections/{collection_id}/docs/{doc_id}"
)

document = response.json()["document"]
if document:
    print(f"Doc ID: {document['doc_id']}")
    print(f"External ID: {document['external_id']}")
    print(f"Vector dimension: {len(document['vector'])}")
    print(f"Inserted at: {document['inserted_at']}")
else:
    print("Document not found")
```

**JavaScript:**
```javascript
async function getVector(collectionId, docId) {
  const response = await axios.get(
    `http://localhost:8080/api/v1/collections/${collectionId}/docs/${docId}`
  );

  const document = response.data.document;
  if (document) {
    console.log(`Doc ID: ${document.doc_id}`);
    console.log(`External ID: ${document.external_id}`);
    console.log(`Vector dimension: ${document.vector.length}`);
    console.log(`Inserted at: ${document.inserted_at}`);
  } else {
    console.log('Document not found');
  }

  return document;
}
```

### Delete Vector Document

Permanently deletes a document from the collection.

**Endpoint:** `DELETE /api/v1/collections/{collection_id}/docs/{doc_id}`

**curl:**
```bash
curl -X DELETE http://localhost:8080/api/v1/collections/018f1234-5678-7abc-def0-123456789abc/docs/018f5678-1234-7abc-def0-111111111111
```

**Python:**
```python
response = requests.delete(
    f"http://localhost:8080/api/v1/collections/{collection_id}/docs/{doc_id}"
)

result = response.json()
print(f"Deleted in {result['latency_ms']:.2f}ms")
```

**JavaScript:**
```javascript
async function deleteVector(collectionId, docId) {
  const response = await axios.delete(
    `http://localhost:8080/api/v1/collections/${collectionId}/docs/${docId}`
  );

  console.log(`Deleted in ${response.data.latency_ms.toFixed(2)}ms`);
}
```

---

## Common Workflows

### Semantic Search Pipeline

Complete workflow for semantic text search:

**Python:**
```python
import requests
import uuid
from sentence_transformers import SentenceTransformer

# 1. Initialize embedding model
model = SentenceTransformer('all-MiniLM-L6-v2')

# 2. Create collection
response = requests.post(
    "http://localhost:8080/api/v1/collections",
    json={
        "name": "articles",
        "dimension": 384,  # all-MiniLM-L6-v2 dimension
        "metric": "cosine"
    }
)
collection_id = response.json()["collection_id"]

# 3. Index documents
documents = [
    "AkiDB is a vector database for ARM devices",
    "Vector search enables semantic similarity",
    "HNSW provides fast approximate nearest neighbor search"
]

for i, text in enumerate(documents):
    vector = model.encode(text).tolist()
    doc_id = str(uuid.uuid7())

    requests.post(
        f"http://localhost:8080/api/v1/collections/{collection_id}/insert",
        json={
            "doc_id": doc_id,
            "external_id": f"doc-{i}",
            "vector": vector
        }
    )

print(f"Indexed {len(documents)} documents")

# 4. Search
query = "fast vector search"
query_vector = model.encode(query).tolist()

response = requests.post(
    f"http://localhost:8080/api/v1/collections/{collection_id}/query",
    json={
        "query_vector": query_vector,
        "top_k": 3
    }
)

results = response.json()
print(f"\nSearch results for: '{query}'")
for match in results["matches"]:
    doc_idx = int(match["external_id"].split("-")[1])
    print(f"  {match['distance']:.4f}: {documents[doc_idx]}")
```

### Image Similarity Search

**Python with OpenAI CLIP:**
```python
import requests
import uuid
import torch
from transformers import CLIPProcessor, CLIPModel

# 1. Load CLIP model
model = CLIPModel.from_pretrained("openai/clip-vit-base-patch32")
processor = CLIPProcessor.from_pretrained("openai/clip-vit-base-patch32")

# 2. Create collection
response = requests.post(
    "http://localhost:8080/api/v1/collections",
    json={
        "name": "images",
        "dimension": 512,  # CLIP image embedding dimension
        "metric": "cosine"
    }
)
collection_id = response.json()["collection_id"]

# 3. Index images
from PIL import Image
import os

image_paths = ["image1.jpg", "image2.jpg", "image3.jpg"]

for img_path in image_paths:
    image = Image.open(img_path)
    inputs = processor(images=image, return_tensors="pt")

    with torch.no_grad():
        image_features = model.get_image_features(**inputs)
        vector = image_features[0].numpy().tolist()

    doc_id = str(uuid.uuid7())
    requests.post(
        f"http://localhost:8080/api/v1/collections/{collection_id}/insert",
        json={
            "doc_id": doc_id,
            "external_id": os.path.basename(img_path),
            "vector": vector
        }
    )

# 4. Search with text query
text_query = "a cat sitting on a couch"
inputs = processor(text=[text_query], return_tensors="pt")

with torch.no_grad():
    text_features = model.get_text_features(**inputs)
    query_vector = text_features[0].numpy().tolist()

response = requests.post(
    f"http://localhost:8080/api/v1/collections/{collection_id}/query",
    json={
        "query_vector": query_vector,
        "top_k": 5
    }
)

print(f"Similar images for: '{text_query}'")
for match in response.json()["matches"]:
    print(f"  {match['external_id']}: {match['distance']:.4f}")
```

### Recommendation System

**Python:**
```python
import requests
import uuid

# 1. Create user embedding collection
response = requests.post(
    "http://localhost:8080/api/v1/collections",
    json={
        "name": "user_profiles",
        "dimension": 128,
        "metric": "dot"  # Dot product for recommendation
    }
)
collection_id = response.json()["collection_id"]

# 2. Index user profiles
user_profiles = {
    "user-001": [0.8, 0.2, 0.1, ...],  # 128-dim profile
    "user-002": [0.1, 0.9, 0.3, ...],
    "user-003": [0.7, 0.1, 0.8, ...]
}

for user_id, profile_vector in user_profiles.items():
    doc_id = str(uuid.uuid7())
    requests.post(
        f"http://localhost:8080/api/v1/collections/{collection_id}/insert",
        json={
            "doc_id": doc_id,
            "external_id": user_id,
            "vector": profile_vector
        }
    )

# 3. Find similar users
target_user = "user-001"
target_vector = user_profiles[target_user]

response = requests.post(
    f"http://localhost:8080/api/v1/collections/{collection_id}/query",
    json={
        "query_vector": target_vector,
        "top_k": 5  # Top 5 similar users (includes self)
    }
)

similar_users = [
    match for match in response.json()["matches"]
    if match["external_id"] != target_user  # Exclude self
]

print(f"Users similar to {target_user}:")
for match in similar_users:
    print(f"  {match['external_id']}: {match['distance']:.4f}")
```

---

## Error Handling

### Common HTTP Status Codes

- **200 OK**: Request successful
- **201 Created**: Collection created successfully
- **204 No Content**: Collection/document deleted successfully
- **400 Bad Request**: Invalid parameters (dimension, metric, UUID format, empty vector)
- **404 Not Found**: Collection or document not found
- **500 Internal Server Error**: Server-side error

### Error Response Format

All errors return JSON with an `error` field:

```json
{
  "error": "Collection not found"
}
```

### Handling Errors in Python

```python
import requests

try:
    response = requests.post(
        f"http://localhost:8080/api/v1/collections/{collection_id}/insert",
        json={
            "doc_id": doc_id,
            "vector": vector
        }
    )
    response.raise_for_status()  # Raises HTTPError for 4xx/5xx

    result = response.json()
    print(f"Inserted: {result['doc_id']}")

except requests.exceptions.HTTPError as e:
    error_msg = e.response.json().get("error", "Unknown error")
    if e.response.status_code == 404:
        print(f"Collection not found: {collection_id}")
    elif e.response.status_code == 400:
        print(f"Invalid request: {error_msg}")
    else:
        print(f"Error: {error_msg}")

except requests.exceptions.ConnectionError:
    print("Cannot connect to AkiDB server. Is it running?")

except Exception as e:
    print(f"Unexpected error: {e}")
```

### Handling Errors in JavaScript

```javascript
async function insertVectorSafe(collectionId, docId, vector) {
  try {
    const response = await axios.post(
      `http://localhost:8080/api/v1/collections/${collectionId}/insert`,
      { doc_id: docId, vector: vector }
    );

    console.log(`Inserted: ${response.data.doc_id}`);
    return response.data;

  } catch (error) {
    if (error.response) {
      // Server responded with error status
      const status = error.response.status;
      const message = error.response.data.error || 'Unknown error';

      if (status === 404) {
        console.error(`Collection not found: ${collectionId}`);
      } else if (status === 400) {
        console.error(`Invalid request: ${message}`);
      } else {
        console.error(`Error: ${message}`);
      }
    } else if (error.request) {
      // Request made but no response
      console.error('Cannot connect to AkiDB server. Is it running?');
    } else {
      // Other errors
      console.error(`Unexpected error: ${error.message}`);
    }
    throw error;
  }
}
```

### Validation Errors

**Invalid dimension:**
```json
{
  "error": "dimension must be between 16 and 4096"
}
```

**Invalid metric:**
```json
{
  "error": "invalid metric: 'euclidean', must be one of: cosine, l2, dot"
}
```

**Invalid UUID:**
```json
{
  "error": "Invalid collection_id: invalid character: expected an optional prefix of `urn:uuid:` followed by [0-9a-fA-F-], found `x` at 0"
}
```

**Empty vector:**
```json
{
  "error": "query_vector cannot be empty"
}
```

---

## Performance Tips

### 1. Batch Operations

Use concurrent requests for bulk inserts:

**Python with ThreadPoolExecutor:**
```python
from concurrent.futures import ThreadPoolExecutor
import requests
import uuid

def insert_vector(collection_id, vector, external_id):
    doc_id = str(uuid.uuid7())
    response = requests.post(
        f"http://localhost:8080/api/v1/collections/{collection_id}/insert",
        json={"doc_id": doc_id, "external_id": external_id, "vector": vector}
    )
    return response.json()

vectors = [...] # Your vectors
external_ids = [...]

with ThreadPoolExecutor(max_workers=20) as executor:
    futures = [
        executor.submit(insert_vector, collection_id, vec, ext_id)
        for vec, ext_id in zip(vectors, external_ids)
    ]
    results = [f.result() for f in futures]

print(f"Inserted {len(results)} vectors")
```

### 2. Choose the Right Metric

- **Cosine**: Normalized vectors, scale-invariant (most common for text embeddings)
- **L2**: Euclidean distance, sensitive to magnitude
- **Dot product**: Faster than cosine, but not normalized (good for recommendation systems)

### 3. Tune top_k

- Smaller `top_k` values are faster
- For production, use `top_k` between 10-100
- Avoid very large `top_k` (>1000) unless necessary

### 4. Pre-normalize Vectors

For cosine similarity, pre-normalize vectors on the client side to improve performance:

**Python:**
```python
import numpy as np

def normalize_vector(vec):
    norm = np.linalg.norm(vec)
    return (vec / norm).tolist() if norm > 0 else vec

vector = normalize_vector(raw_vector)
```

### 5. Monitor Latency

Track query latency from the response:

**Python:**
```python
response = requests.post(
    f"http://localhost:8080/api/v1/collections/{collection_id}/query",
    json={"query_vector": query_vector, "top_k": 10}
)

latency = response.json()["latency_ms"]
if latency > 50:  # Alert if P95 > 50ms
    print(f"WARNING: High latency: {latency:.2f}ms")
```

### 6. Use Prometheus Metrics

Monitor server health with Prometheus:

```bash
curl http://localhost:8080/metrics
```

Integrate with Prometheus + Grafana for production monitoring.

---

## Client Libraries

### Python Client Example

**akidb_client.py:**
```python
import requests
import uuid
from typing import List, Optional, Dict, Any

class AkiDBClient:
    def __init__(self, base_url: str = "http://localhost:8080"):
        self.base_url = base_url
        self.api_base = f"{base_url}/api/v1"

    def health(self) -> Dict[str, str]:
        """Check server health."""
        response = requests.get(f"{self.base_url}/health")
        response.raise_for_status()
        return response.json()

    def create_collection(
        self,
        name: str,
        dimension: int,
        metric: str = "cosine",
        embedding_model: Optional[str] = None
    ) -> str:
        """Create a collection and return its ID."""
        payload = {
            "name": name,
            "dimension": dimension,
            "metric": metric
        }
        if embedding_model:
            payload["embedding_model"] = embedding_model

        response = requests.post(f"{self.api_base}/collections", json=payload)
        response.raise_for_status()
        return response.json()["collection_id"]

    def list_collections(self) -> List[Dict[str, Any]]:
        """List all collections."""
        response = requests.get(f"{self.api_base}/collections")
        response.raise_for_status()
        return response.json()["collections"]

    def get_collection(self, collection_id: str) -> Dict[str, Any]:
        """Get collection details."""
        response = requests.get(f"{self.api_base}/collections/{collection_id}")
        response.raise_for_status()
        return response.json()["collection"]

    def delete_collection(self, collection_id: str) -> None:
        """Delete a collection."""
        response = requests.delete(f"{self.api_base}/collections/{collection_id}")
        response.raise_for_status()

    def insert(
        self,
        collection_id: str,
        vector: List[float],
        external_id: Optional[str] = None,
        doc_id: Optional[str] = None
    ) -> str:
        """Insert a vector and return its doc_id."""
        if doc_id is None:
            doc_id = str(uuid.uuid7())

        payload = {
            "doc_id": doc_id,
            "vector": vector
        }
        if external_id:
            payload["external_id"] = external_id

        response = requests.post(
            f"{self.api_base}/collections/{collection_id}/insert",
            json=payload
        )
        response.raise_for_status()
        return response.json()["doc_id"]

    def query(
        self,
        collection_id: str,
        query_vector: List[float],
        top_k: int = 10
    ) -> List[Dict[str, Any]]:
        """Search for similar vectors."""
        response = requests.post(
            f"{self.api_base}/collections/{collection_id}/query",
            json={
                "query_vector": query_vector,
                "top_k": top_k
            }
        )
        response.raise_for_status()
        return response.json()["matches"]

    def get(self, collection_id: str, doc_id: str) -> Optional[Dict[str, Any]]:
        """Get a vector document."""
        response = requests.get(
            f"{self.api_base}/collections/{collection_id}/docs/{doc_id}"
        )
        response.raise_for_status()
        return response.json()["document"]

    def delete(self, collection_id: str, doc_id: str) -> None:
        """Delete a vector document."""
        response = requests.delete(
            f"{self.api_base}/collections/{collection_id}/docs/{doc_id}"
        )
        response.raise_for_status()

# Usage
client = AkiDBClient()

# Create collection
collection_id = client.create_collection("test", dimension=128, metric="cosine")

# Insert vectors
doc_id = client.insert(collection_id, [0.1] * 128, external_id="doc-1")

# Query
results = client.query(collection_id, [0.15] * 128, top_k=5)
for result in results:
    print(f"{result['external_id']}: {result['distance']:.4f}")

# Cleanup
client.delete_collection(collection_id)
```

### JavaScript Client Example

**akidb-client.js:**
```javascript
const axios = require('axios');
const { v7: uuidv7 } = require('uuid');

class AkiDBClient {
  constructor(baseUrl = 'http://localhost:8080') {
    this.baseUrl = baseUrl;
    this.apiBase = `${baseUrl}/api/v1`;
  }

  async health() {
    const response = await axios.get(`${this.baseUrl}/health`);
    return response.data;
  }

  async createCollection(name, dimension, metric = 'cosine', embeddingModel = null) {
    const payload = { name, dimension, metric };
    if (embeddingModel) {
      payload.embedding_model = embeddingModel;
    }

    const response = await axios.post(`${this.apiBase}/collections`, payload);
    return response.data.collection_id;
  }

  async listCollections() {
    const response = await axios.get(`${this.apiBase}/collections`);
    return response.data.collections;
  }

  async getCollection(collectionId) {
    const response = await axios.get(`${this.apiBase}/collections/${collectionId}`);
    return response.data.collection;
  }

  async deleteCollection(collectionId) {
    await axios.delete(`${this.apiBase}/collections/${collectionId}`);
  }

  async insert(collectionId, vector, externalId = null, docId = null) {
    if (!docId) {
      docId = uuidv7();
    }

    const payload = { doc_id: docId, vector };
    if (externalId) {
      payload.external_id = externalId;
    }

    const response = await axios.post(
      `${this.apiBase}/collections/${collectionId}/insert`,
      payload
    );
    return response.data.doc_id;
  }

  async query(collectionId, queryVector, topK = 10) {
    const response = await axios.post(
      `${this.apiBase}/collections/${collectionId}/query`,
      { query_vector: queryVector, top_k: topK }
    );
    return response.data.matches;
  }

  async get(collectionId, docId) {
    const response = await axios.get(
      `${this.apiBase}/collections/${collectionId}/docs/${docId}`
    );
    return response.data.document;
  }

  async delete(collectionId, docId) {
    await axios.delete(
      `${this.apiBase}/collections/${collectionId}/docs/${docId}`
    );
  }
}

// Usage
(async () => {
  const client = new AkiDBClient();

  // Create collection
  const collectionId = await client.createCollection('test', 128, 'cosine');

  // Insert vectors
  const docId = await client.insert(collectionId, Array(128).fill(0.1), 'doc-1');

  // Query
  const results = await client.query(collectionId, Array(128).fill(0.15), 5);
  results.forEach(result => {
    console.log(`${result.external_id}: ${result.distance.toFixed(4)}`);
  });

  // Cleanup
  await client.deleteCollection(collectionId);
})();
```

---

## Working with S3 Storage

AkiDB 2.0 supports tiered storage with S3/MinIO for production deployments. This section demonstrates how to configure and monitor S3 backend integration.

### Configure S3 Backend

**1. Set Environment Variables:**
```bash
# AWS credentials
export AWS_ACCESS_KEY_ID=your_key
export AWS_SECRET_ACCESS_KEY=your_secret
export AWS_DEFAULT_REGION=us-west-2
```

**2. Update config.toml:**
```toml
[storage]
tiering_policy = "MemoryS3"
wal_path = "/var/lib/akidb/wal"
snapshot_dir = "/var/lib/akidb/snapshots"
s3_bucket = "s3://my-akidb-bucket"
s3_region = "us-west-2"
enable_background_compaction = true

[storage.compaction]
threshold_bytes = 104857600  # 100MB
threshold_ops = 10000

[storage.retry]
max_retries = 5
base_backoff_secs = 1
max_backoff_secs = 64
```

**3. Start Server:**
```bash
# REST server with S3 backend
akidb-rest --config config.toml

# gRPC server with S3 backend
akidb-grpc --config config.toml
```

### Monitor S3 Operations

**Get Storage Metrics:**
```bash
# Query storage metrics for a collection
curl http://localhost:8080/collections/{collection_id}/metrics

# Example response:
{
  "inserts": 1000,
  "s3_uploads": 1000,
  "s3_retries": 5,
  "s3_permanent_failures": 0,
  "dlq_size": 0,
  "cache_hits": 850,
  "cache_misses": 150,
  "cache_hit_rate": 0.85,
  "compactions": 2,
  "last_snapshot_at": "2025-11-08T10:30:00Z"
}
```

**Prometheus Metrics:**
```bash
# View all storage metrics
curl http://localhost:8080/metrics | grep storage_

# Example metrics:
storage_inserts{collection_id="..."} 1000
storage_s3_uploads{collection_id="..."} 1000
storage_s3_retries{collection_id="..."} 5
storage_s3_permanent_failures{collection_id="..."} 0
storage_dlq_size{collection_id="..."} 0
storage_cache_hits{collection_id="..."} 850
storage_cache_misses{collection_id="..."} 150
storage_compactions{collection_id="..."} 2
```

### Inspect Dead Letter Queue

**View DLQ Entries:**
```bash
# Get DLQ entries (vectors that failed permanent S3 upload)
curl http://localhost:8080/admin/collections/{collection_id}/dlq

# Example response:
[
  {
    "collection_id": "01234567-89ab-cdef-0123-456789abcdef",
    "document_id": "fedcba98-7654-3210-fedc-ba9876543210",
    "failed_at": "2025-11-08T10:30:00Z",
    "error": "403 Forbidden: Invalid credentials",
    "retry_count": 5
  }
]
```

**Clear DLQ (After Manual Resolution):**
```bash
# Delete all DLQ entries for a collection
curl -X DELETE http://localhost:8080/admin/collections/{collection_id}/dlq

# Response:
{
  "cleared": 1,
  "message": "Dead letter queue cleared"
}
```

**Retry DLQ Entries:**
```bash
# Retry all DLQ entries (after fixing root cause)
curl -X POST http://localhost:8080/admin/collections/{collection_id}/dlq/retry

# Response:
{
  "retried": 1,
  "successful": 1,
  "failed": 0
}
```

### Trigger Manual Compaction

**Compact Collection WAL:**
```bash
# Manually trigger background compaction
curl -X POST http://localhost:8080/admin/collections/{collection_id}/compact

# Response:
{
  "compaction_started": true,
  "wal_size_before": 104857600,
  "message": "Compaction triggered in background"
}
```

### Monitor Cache Performance (S3Only Policy)

**Get Cache Stats:**
```bash
# Query cache statistics
curl http://localhost:8080/collections/{collection_id}/cache/stats

# Example response:
{
  "size": 8500,
  "capacity": 10000,
  "hit_rate": 0.85,
  "hits": 850,
  "misses": 150
}
```

### Example: Complete S3 Workflow

```bash
#!/bin/bash

# 1. Configure S3 backend
export AWS_ACCESS_KEY_ID=your_key
export AWS_SECRET_ACCESS_KEY=your_secret

# 2. Start server with MemoryS3 policy
akidb-rest --config config.toml &

# 3. Create collection
COLLECTION_ID=$(curl -X POST http://localhost:8080/collections \
  -H "Content-Type: application/json" \
  -d '{
    "name": "s3-backed-collection",
    "dimension": 512,
    "metric": "cosine",
    "embedding_model": "sentence-transformers/all-MiniLM-L6-v2"
  }' | jq -r '.collection_id')

# 4. Insert vectors (will be backed up to S3 asynchronously)
for i in {1..1000}; do
  curl -X POST http://localhost:8080/collections/$COLLECTION_ID/vectors \
    -H "Content-Type: application/json" \
    -d "{
      \"text\": \"Document $i\",
      \"metadata\": {\"source\": \"s3-test\", \"index\": $i}
    }"
done

# 5. Wait for S3 uploads
sleep 5

# 6. Check S3 upload metrics
curl http://localhost:8080/collections/$COLLECTION_ID/metrics | jq '.s3_uploads'

# 7. Query vectors (served from memory, backed by S3)
curl -X POST http://localhost:8080/collections/$COLLECTION_ID/query \
  -H "Content-Type: application/json" \
  -d '{
    "text": "Document 500",
    "k": 10
  }' | jq '.results | length'

# 8. Verify S3 backup exists (AWS CLI)
aws s3 ls s3://my-akidb-bucket/vectors/$COLLECTION_ID/ --recursive

# 9. Monitor DLQ (should be empty)
curl http://localhost:8080/admin/collections/$COLLECTION_ID/dlq | jq 'length'

echo "S3 workflow complete!"
```

---

## Next Steps

1. **Explore the OpenAPI Spec**: See `/docs/openapi.yaml` for complete API reference
2. **Monitor with Prometheus**: Use `/metrics` endpoint for production monitoring
3. **Optimize Embeddings**: Experiment with different embedding models and dimensions
4. **Benchmark Performance**: Test with your specific workload and dataset size
5. **Plan for Multi-tenancy**: RC2+ will include full RBAC and tenant isolation

---

## Additional Resources

- **GitHub Repository**: https://github.com/yourusername/akidb2
- **OpenAPI Specification**: `/docs/openapi.yaml`
- **Architecture Documentation**: `/automatosx/PRD/akidb-2.0-technical-architecture.md`
- **Migration Guide**: `/docs/MIGRATION-V1-TO-V2.md`

---

## Support

For issues, questions, or feature requests, please open an issue on GitHub or contact the maintainers.

**Version**: 0.1.0 (RC1)
**Last Updated**: 2024-11-07
