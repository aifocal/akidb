# AkiDB Python SDK Quickstart

Get started with AkiDB in Python in 5 minutes.

## Installation

```bash
pip install akidb
```

## Quick Start

### 1. Initialize Client

```python
import akidb

# Connect to AkiDB
client = akidb.Client(
    api_key="your-api-key-here",
    endpoint="https://api.akidb.com"
)
```

### 2. Create a Collection

```python
# Create a collection for 384-dimensional embeddings
collection = client.create_collection(
    name="my-embeddings",
    dimension=384,
    metric="cosine",
    embedding_model="sentence-transformers/all-MiniLM-L6-v2"
)

print(f"Created collection: {collection.id}")
```

### 3. Insert Vectors

```python
# Insert single vector
collection.insert(
    id="doc-001",
    vector=[0.1, 0.2, 0.3, ...],  # 384-dimensional vector
    metadata={
        "text": "Machine learning on edge devices",
        "source": "blog",
        "category": "ml"
    }
)

# Batch insert
vectors = [
    {
        "id": "doc-001",
        "vector": [0.1, 0.2, 0.3, ...],
        "metadata": {"text": "First document"}
    },
    {
        "id": "doc-002",
        "vector": [0.5, 0.6, 0.7, ...],
        "metadata": {"text": "Second document"}
    }
]

collection.insert_batch(vectors)
print(f"Inserted {len(vectors)} vectors")
```

### 4. Search Vectors

```python
# Similarity search
results = collection.search(
    vector=[0.1, 0.2, 0.3, ...],
    top_k=10
)

for result in results:
    print(f"ID: {result.id}, Score: {result.score:.4f}")
    print(f"Metadata: {result.metadata}")
```

### 5. Generate Embeddings

```python
# Use built-in embedding service
embeddings = client.embed([
    "Machine learning on ARM devices",
    "Vector database optimization",
    "Edge computing for AI"
])

# Insert with auto-generated embeddings
for i, embedding in enumerate(embeddings):
    collection.insert(
        id=f"doc-{i}",
        vector=embedding,
        metadata={"text": texts[i]}
    )
```

## Complete Example

```python
import akidb

def main():
    # Initialize client
    client = akidb.Client(
        api_key="your-api-key",
        endpoint="https://api.akidb.com"
    )

    # Create collection
    collection = client.create_collection(
        name="semantic-search",
        dimension=384,
        metric="cosine"
    )

    # Sample documents
    documents = [
        "AkiDB is a vector database for ARM devices",
        "Machine learning on edge hardware",
        "Semantic search with embeddings",
        "High-performance vector similarity search"
    ]

    # Generate embeddings and insert
    embeddings = client.embed(documents)

    for i, (doc, embedding) in enumerate(zip(documents, embeddings)):
        collection.insert(
            id=f"doc-{i}",
            vector=embedding,
            metadata={"text": doc}
        )

    print(f"Inserted {len(documents)} documents")

    # Search for similar documents
    query = "vector database for edge computing"
    query_embedding = client.embed([query])[0]

    results = collection.search(
        vector=query_embedding,
        top_k=3
    )

    print(f"\nTop 3 results for: '{query}'")
    for i, result in enumerate(results, 1):
        print(f"{i}. Score: {result.score:.4f}")
        print(f"   Text: {result.metadata['text']}")

if __name__ == "__main__":
    main()
```

## Advanced Usage

### Metadata Filtering

```python
# Search with metadata filter
results = collection.search(
    vector=query_vector,
    top_k=10,
    filter={"source": "blog", "category": "ml"}
)
```

### Collection Management

```python
# List all collections
collections = client.list_collections()
for coll in collections:
    print(f"Name: {coll.name}, Vectors: {coll.document_count}")

# Get collection info
info = client.get_collection("my-embeddings")
print(f"Dimension: {info.dimension}, Metric: {info.metric}")

# Delete collection
client.delete_collection("my-embeddings")
```

### Error Handling

```python
from akidb.exceptions import CollectionNotFoundError, RateLimitError

try:
    collection = client.get_collection("nonexistent")
except CollectionNotFoundError:
    print("Collection not found, creating new one...")
    collection = client.create_collection(
        name="nonexistent",
        dimension=384,
        metric="cosine"
    )
except RateLimitError as e:
    print(f"Rate limit exceeded. Retry after {e.retry_after} seconds")
```

### Async Support

```python
import asyncio
import akidb

async def main():
    # Use async client
    client = akidb.AsyncClient(
        api_key="your-api-key",
        endpoint="https://api.akidb.com"
    )

    # All operations are async
    collection = await client.create_collection(
        name="async-collection",
        dimension=384,
        metric="cosine"
    )

    # Concurrent inserts
    tasks = [
        collection.insert(id=f"doc-{i}", vector=vector)
        for i, vector in enumerate(vectors)
    ]
    await asyncio.gather(*tasks)

    # Async search
    results = await collection.search(
        vector=query_vector,
        top_k=10
    )

asyncio.run(main())
```

## Performance Tips

### Batch Operations

```python
# ✅ GOOD: Batch insert (5,000+ ops/sec)
collection.insert_batch(vectors)

# ❌ BAD: Individual inserts
for vector in vectors:
    collection.insert(**vector)
```

### Connection Pooling

```python
# Configure connection pool
client = akidb.Client(
    api_key="your-api-key",
    endpoint="https://api.akidb.com",
    max_connections=100,
    timeout=30
)
```

### Caching Embeddings

```python
from functools import lru_cache

@lru_cache(maxsize=10000)
def get_embedding(text: str):
    return client.embed([text])[0]

# Embeddings are cached automatically
embedding1 = get_embedding("same text")
embedding2 = get_embedding("same text")  # Cached, no API call
```

## Migration from Other Databases

### From Pinecone

```python
# Pinecone
import pinecone
index = pinecone.Index("my-index")
index.upsert(vectors=[
    ("id1", [0.1, 0.2, ...], {"key": "value"})
])
results = index.query(vector=[0.1, 0.2, ...], top_k=10)

# AkiDB (similar API)
import akidb
client = akidb.Client(api_key="...")
collection = client.create_collection(name="my-index", dimension=384, metric="cosine")
collection.insert(id="id1", vector=[0.1, 0.2, ...], metadata={"key": "value"})
results = collection.search(vector=[0.1, 0.2, ...], top_k=10)
```

### From Weaviate

```python
# Weaviate
import weaviate
client = weaviate.Client("http://localhost:8080")
client.data_object.create(
    data_object={"text": "Hello"},
    class_name="Document",
    vector=[0.1, 0.2, ...]
)

# AkiDB
import akidb
client = akidb.Client(api_key="...")
collection = client.create_collection(name="Document", dimension=384, metric="cosine")
collection.insert(id="doc-1", vector=[0.1, 0.2, ...], metadata={"text": "Hello"})
```

## Rate Limits & Quotas

```python
from akidb import Client

client = Client(api_key="...")

# Check current usage
usage = client.get_usage()
print(f"Vectors: {usage.vector_count}/{usage.vector_quota}")
print(f"QPS: {usage.current_qps}/{usage.qps_quota}")
print(f"Tier: {usage.tier}")  # free, startup, business, enterprise
```

## Troubleshooting

### Connection Errors

```python
# Set timeout and retries
client = akidb.Client(
    api_key="your-api-key",
    endpoint="https://api.akidb.com",
    timeout=60,
    max_retries=3
)
```

### Debug Mode

```python
# Enable debug logging
import logging
logging.basicConfig(level=logging.DEBUG)

client = akidb.Client(
    api_key="your-api-key",
    endpoint="https://api.akidb.com",
    debug=True
)
```

## API Reference

Full API documentation: https://docs.akidb.com/python-sdk

## Support

- 📧 Email: support@akidb.com
- 💬 Discord: https://discord.gg/akidb
- 📚 Documentation: https://docs.akidb.com
- 🐛 Issues: https://github.com/akidb/akidb-python/issues

## Next Steps

- Read the [API Tutorial](./API-TUTORIAL.md)
- Review [Performance Benchmarks](./PERFORMANCE-BENCHMARKS.md)
- Deploy with [Deployment Guide](./DEPLOYMENT-GUIDE.md)
- Explore [Migration Guides](./MIGRATION-GUIDES.md)
