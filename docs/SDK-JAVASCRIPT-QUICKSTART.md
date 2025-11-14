# AkiDB JavaScript SDK Quickstart

Get started with AkiDB in JavaScript/TypeScript in 5 minutes.

## Installation

### Node.js

```bash
npm install @akidb/client
# or
yarn add @akidb/client
# or
pnpm add @akidb/client
```

### Browser (via CDN)

```html
<script src="https://cdn.jsdelivr.net/npm/@akidb/client@latest/dist/akidb.min.js"></script>
```

## Quick Start

### 1. Initialize Client

```javascript
import { AkiDBClient } from '@akidb/client';

// Connect to AkiDB
const client = new AkiDBClient({
  apiKey: 'your-api-key-here',
  endpoint: 'https://api.akidb.com'
});
```

### 2. Create a Collection

```javascript
// Create a collection for 384-dimensional embeddings
const collection = await client.createCollection({
  name: 'my-embeddings',
  dimension: 384,
  metric: 'cosine',
  embeddingModel: 'sentence-transformers/all-MiniLM-L6-v2'
});

console.log(`Created collection: ${collection.id}`);
```

### 3. Insert Vectors

```javascript
// Insert single vector
await collection.insert({
  id: 'doc-001',
  vector: [0.1, 0.2, 0.3, /* ...384 dims */],
  metadata: {
    text: 'Machine learning on edge devices',
    source: 'blog',
    category: 'ml'
  }
});

// Batch insert
const vectors = [
  {
    id: 'doc-001',
    vector: [0.1, 0.2, 0.3, /* ... */],
    metadata: { text: 'First document' }
  },
  {
    id: 'doc-002',
    vector: [0.5, 0.6, 0.7, /* ... */],
    metadata: { text: 'Second document' }
  }
];

await collection.insertBatch(vectors);
console.log(`Inserted ${vectors.length} vectors`);
```

### 4. Search Vectors

```javascript
// Similarity search
const results = await collection.search({
  vector: [0.1, 0.2, 0.3, /* ... */],
  topK: 10
});

results.forEach(result => {
  console.log(`ID: ${result.id}, Score: ${result.score.toFixed(4)}`);
  console.log(`Metadata:`, result.metadata);
});
```

### 5. Generate Embeddings

```javascript
// Use built-in embedding service
const embeddings = await client.embed([
  'Machine learning on ARM devices',
  'Vector database optimization',
  'Edge computing for AI'
]);

// Insert with auto-generated embeddings
for (let i = 0; i < embeddings.length; i++) {
  await collection.insert({
    id: `doc-${i}`,
    vector: embeddings[i],
    metadata: { text: texts[i] }
  });
}
```

## Complete Example

```javascript
import { AkiDBClient } from '@akidb/client';

async function main() {
  // Initialize client
  const client = new AkiDBClient({
    apiKey: 'your-api-key',
    endpoint: 'https://api.akidb.com'
  });

  // Create collection
  const collection = await client.createCollection({
    name: 'semantic-search',
    dimension: 384,
    metric: 'cosine'
  });

  // Sample documents
  const documents = [
    'AkiDB is a vector database for ARM devices',
    'Machine learning on edge hardware',
    'Semantic search with embeddings',
    'High-performance vector similarity search'
  ];

  // Generate embeddings and insert
  const embeddings = await client.embed(documents);

  for (let i = 0; i < documents.length; i++) {
    await collection.insert({
      id: `doc-${i}`,
      vector: embeddings[i],
      metadata: { text: documents[i] }
    });
  }

  console.log(`Inserted ${documents.length} documents`);

  // Search for similar documents
  const query = 'vector database for edge computing';
  const [queryEmbedding] = await client.embed([query]);

  const results = await collection.search({
    vector: queryEmbedding,
    topK: 3
  });

  console.log(`\nTop 3 results for: '${query}'`);
  results.forEach((result, i) => {
    console.log(`${i + 1}. Score: ${result.score.toFixed(4)}`);
    console.log(`   Text: ${result.metadata.text}`);
  });
}

main().catch(console.error);
```

## TypeScript Support

The SDK is written in TypeScript with full type definitions:

```typescript
import { AkiDBClient, Collection, SearchResult } from '@akidb/client';

// Type-safe client
const client = new AkiDBClient({
  apiKey: process.env.AKIDB_API_KEY!,
  endpoint: 'https://api.akidb.com'
});

// Type-safe collection
const collection: Collection = await client.createCollection({
  name: 'my-collection',
  dimension: 384,
  metric: 'cosine'
});

// Type-safe search results
const results: SearchResult[] = await collection.search({
  vector: queryVector,
  topK: 10
});

// Custom metadata types
interface DocMetadata {
  text: string;
  source: string;
  timestamp: number;
}

const customResults = await collection.search<DocMetadata>({
  vector: queryVector,
  topK: 10
});

// TypeScript infers metadata type
customResults.forEach(result => {
  console.log(result.metadata.text); // ✅ Type-safe
  console.log(result.metadata.unknown); // ❌ TypeScript error
});
```

## Advanced Usage

### Metadata Filtering

```javascript
// Search with metadata filter
const results = await collection.search({
  vector: queryVector,
  topK: 10,
  filter: {
    source: 'blog',
    category: 'ml'
  }
});
```

### Collection Management

```javascript
// List all collections
const collections = await client.listCollections();
collections.forEach(coll => {
  console.log(`Name: ${coll.name}, Vectors: ${coll.documentCount}`);
});

// Get collection info
const info = await client.getCollection('my-embeddings');
console.log(`Dimension: ${info.dimension}, Metric: ${info.metric}`);

// Delete collection
await client.deleteCollection('my-embeddings');
```

### Error Handling

```javascript
import {
  CollectionNotFoundError,
  RateLimitError,
  AkiDBError
} from '@akidb/client';

try {
  const collection = await client.getCollection('nonexistent');
} catch (error) {
  if (error instanceof CollectionNotFoundError) {
    console.log('Collection not found, creating new one...');
    collection = await client.createCollection({
      name: 'nonexistent',
      dimension: 384,
      metric: 'cosine'
    });
  } else if (error instanceof RateLimitError) {
    console.log(`Rate limit exceeded. Retry after ${error.retryAfter}s`);
  } else if (error instanceof AkiDBError) {
    console.error(`AkiDB error: ${error.message}`);
  } else {
    throw error;
  }
}
```

### Promise.all for Concurrent Operations

```javascript
// Concurrent inserts (faster for large batches)
const insertPromises = vectors.map(vector =>
  collection.insert(vector)
);
await Promise.all(insertPromises);

// Concurrent searches
const queries = [vector1, vector2, vector3];
const searchPromises = queries.map(query =>
  collection.search({ vector: query, topK: 10 })
);
const allResults = await Promise.all(searchPromises);
```

### React Integration

```jsx
import { useState, useEffect } from 'react';
import { AkiDBClient } from '@akidb/client';

function VectorSearch() {
  const [client] = useState(() => new AkiDBClient({
    apiKey: process.env.REACT_APP_AKIDB_API_KEY,
    endpoint: 'https://api.akidb.com'
  }));

  const [results, setResults] = useState([]);
  const [loading, setLoading] = useState(false);

  async function handleSearch(query) {
    setLoading(true);
    try {
      const collection = await client.getCollection('my-embeddings');
      const [queryEmbedding] = await client.embed([query]);
      const searchResults = await collection.search({
        vector: queryEmbedding,
        topK: 10
      });
      setResults(searchResults);
    } catch (error) {
      console.error('Search failed:', error);
    } finally {
      setLoading(false);
    }
  }

  return (
    <div>
      <SearchBox onSearch={handleSearch} />
      {loading ? <Spinner /> : <ResultsList results={results} />}
    </div>
  );
}
```

### Next.js API Route

```javascript
// pages/api/search.js
import { AkiDBClient } from '@akidb/client';

const client = new AkiDBClient({
  apiKey: process.env.AKIDB_API_KEY,
  endpoint: 'https://api.akidb.com'
});

export default async function handler(req, res) {
  if (req.method !== 'POST') {
    return res.status(405).json({ error: 'Method not allowed' });
  }

  const { query, topK = 10 } = req.body;

  try {
    const collection = await client.getCollection('my-embeddings');
    const [queryEmbedding] = await client.embed([query]);

    const results = await collection.search({
      vector: queryEmbedding,
      topK
    });

    res.status(200).json({ results });
  } catch (error) {
    console.error('Search error:', error);
    res.status(500).json({ error: error.message });
  }
}
```

## Performance Tips

### Batch Operations

```javascript
// ✅ GOOD: Batch insert (5,000+ ops/sec)
await collection.insertBatch(vectors);

// ❌ BAD: Individual inserts
for (const vector of vectors) {
  await collection.insert(vector);
}
```

### Connection Configuration

```javascript
// Configure client for high throughput
const client = new AkiDBClient({
  apiKey: 'your-api-key',
  endpoint: 'https://api.akidb.com',
  maxConnections: 100,
  timeout: 30000, // 30 seconds
  retries: 3
});
```

### Caching Embeddings

```javascript
// Simple cache implementation
const embeddingCache = new Map();

async function getCachedEmbedding(text) {
  if (embeddingCache.has(text)) {
    return embeddingCache.get(text);
  }

  const [embedding] = await client.embed([text]);
  embeddingCache.set(text, embedding);
  return embedding;
}

// Use cached embeddings
const embedding = await getCachedEmbedding('same text');
```

## Migration from Other Databases

### From Pinecone

```javascript
// Pinecone
import { PineconeClient } from '@pinecone-database/pinecone';
const pinecone = new PineconeClient();
const index = pinecone.Index('my-index');
await index.upsert([{
  id: 'id1',
  values: [0.1, 0.2, ...],
  metadata: { key: 'value' }
}]);

// AkiDB (similar API)
import { AkiDBClient } from '@akidb/client';
const client = new AkiDBClient({ apiKey: '...' });
const collection = await client.createCollection({
  name: 'my-index',
  dimension: 384,
  metric: 'cosine'
});
await collection.insert({
  id: 'id1',
  vector: [0.1, 0.2, ...],
  metadata: { key: 'value' }
});
```

### From Weaviate

```javascript
// Weaviate
import weaviate from 'weaviate-ts-client';
const client = weaviate.client({ scheme: 'http', host: 'localhost:8080' });
await client.data.creator()
  .withClassName('Document')
  .withProperties({ text: 'Hello' })
  .withVector([0.1, 0.2, ...])
  .do();

// AkiDB
import { AkiDBClient } from '@akidb/client';
const client = new AkiDBClient({ apiKey: '...' });
const collection = await client.createCollection({
  name: 'Document',
  dimension: 384,
  metric: 'cosine'
});
await collection.insert({
  id: 'doc-1',
  vector: [0.1, 0.2, ...],
  metadata: { text: 'Hello' }
});
```

## Rate Limits & Quotas

```javascript
// Check current usage
const usage = await client.getUsage();
console.log(`Vectors: ${usage.vectorCount}/${usage.vectorQuota}`);
console.log(`QPS: ${usage.currentQps}/${usage.qpsQuota}`);
console.log(`Tier: ${usage.tier}`); // free, startup, business, enterprise
```

## Troubleshooting

### Enable Debug Logging

```javascript
const client = new AkiDBClient({
  apiKey: 'your-api-key',
  endpoint: 'https://api.akidb.com',
  debug: true // Enables console logging
});
```

### Timeout Configuration

```javascript
// Set custom timeout for slow networks
const client = new AkiDBClient({
  apiKey: 'your-api-key',
  endpoint: 'https://api.akidb.com',
  timeout: 60000 // 60 seconds
});
```

## API Reference

Full API documentation: https://docs.akidb.com/javascript-sdk

## Support

- 📧 Email: support@akidb.com
- 💬 Discord: https://discord.gg/akidb
- 📚 Documentation: https://docs.akidb.com
- 🐛 Issues: https://github.com/akidb/akidb-js/issues

## Next Steps

- Read the [API Tutorial](./API-TUTORIAL.md)
- Review [Performance Benchmarks](./PERFORMANCE-BENCHMARKS.md)
- Deploy with [Deployment Guide](./DEPLOYMENT-GUIDE.md)
- Explore [Migration Guides](./MIGRATION-GUIDES.md)
