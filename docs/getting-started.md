# Getting Started with AkiDB

This guide will walk you through setting up your local development environment and running AkiDB for the first time.

## Prerequisites

Before you begin, ensure you have the following installed:

- **Rust:** Version 1.77 or later. You can install Rust using [rustup](https://rustup.rs/).
- **Docker:** The latest version of [Docker Desktop](https://www.docker.com/products/docker-desktop/) or Docker Engine.
- **Docker Compose:** Included with Docker Desktop.

## 1. Clone the Repository

Start by cloning the AkiDB repository to your local machine:

```bash
git clone https://github.com/aifocal/akidb.git
cd akidb
```

## 2. Start the Development Environment

AkiDB comes with a script that simplifies the setup of a local development environment. This script uses Docker Compose to start a MinIO container, which AkiDB uses for S3-compatible object storage.

Run the following command from the root of the project directory:

```bash
./scripts/dev-init.sh
```

This script will:

1.  Start a MinIO container.
2.  Start the AkiDB API server.

Once the script finishes, the AkiDB API will be running and available at `http://localhost:8080`.

## 3. Your First Interaction with AkiDB

With AkiDB running, you can now interact with it using `curl` or any HTTP client.

### Create a Collection

A collection is used to store vectors of the same dimension. Let's create a collection named `my_first_collection` to store 4-dimensional vectors.

```bash
curl -X POST http://localhost:8080/collections \
  -H "Content-Type: application/json" \
  -d 
'{ "name": "my_first_collection", "vector_dim": 4, "distance": "Cosine" }'
```

You should receive a JSON response confirming the collection was created.

### Insert Vectors

Now, let's insert some vectors into our collection. Each vector has a unique ID, the vector data, and an optional JSON payload.

```bash
curl -X POST http://localhost:8080/collections/my_first_collection/vectors \
  -H "Content-Type: application/json" \
  -d 
'{ "vectors": [ { "id": "vec1", "vector": [0.1, 0.2, 0.3, 0.4], "payload": { "color": "blue" } }, { "id": "vec2", "vector": [0.9, 0.8, 0.7, 0.6], "payload": { "color": "red" } } ] }'
```

### Perform a Vector Search

Finally, let's perform a semantic search to find the most similar vectors to a given query vector.

```bash
curl -X POST http://localhost:8080/collections/my_first_collection/search \
  -H "Content-Type: application/json" \
  -d 
'{ "vector": [0.1, 0.2, 0.3, 0.4], "top_k": 2 }'
```

The response will contain the most similar vectors, with `vec1` having the highest score because it's an exact match.

## Next Steps

Congratulations! You have successfully set up AkiDB and performed your first vector search.

To learn more, explore the following resources:

- **[API Reference](./api-reference.md):** Dive into the details of the REST API.
- **[Architecture](./architecture.md):** Understand how AkiDB works under the hood.
