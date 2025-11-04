# AkiDB API Reference

This document provides a detailed reference for the AkiDB REST API. All API endpoints are available under the base URL of your AkiDB instance.

## Collections

Collections are used to store and manage vectors.

### Create a Collection

Creates a new collection for storing vectors.

-   **Endpoint:** `POST /collections`
-   **Success Response:** `201 Created`

#### Request Body

| Field          | Type                               | Description                                                                                      |
| -------------- | ---------------------------------- | ------------------------------------------------------------------------------------------------ |
| `name`         | `string`                           | The unique name of the collection.                                                               |
| `vector_dim`   | `integer`                          | The dimension of the vectors that will be stored in this collection.                             |
| `distance`     | `string` (optional)                | The distance metric to use for similarity search. Can be `Cosine`, `Euclidean`, or `Dot`. Defaults to `Cosine`. |
| `replication`  | `integer` (optional)               | The number of replicas for each shard. Defaults to `1`.                                          |
| `shard_count`  | `integer` (optional)               | The number of shards for the collection. Defaults to `1`.                                        |
| `payload_schema` | `object` (optional)              | A JSON schema for the vector payloads.                                                           |

#### Example

```bash
curl -X POST http://localhost:8080/collections \
  -H "Content-Type: application/json" \
  -d 
{
    "name": "product_embeddings",
    "vector_dim": 768,
    "distance": "Cosine"
}
```

### Get a Collection

Retrieves information about a specific collection.

-   **Endpoint:** `GET /collections/{name}`
-   **Success Response:** `200 OK`

#### Example

```bash
curl http://localhost:8080/collections/product_embeddings
```

### List Collections

Returns a list of all collection names.

-   **Endpoint:** `GET /collections`
-   **Success Response:** `200 OK`

#### Example

```bash
curl http://localhost:8080/collections
```

### Delete a Collection

Deletes a collection and all of its associated data.

-   **Endpoint:** `DELETE /collections/{name}`
-   **Success Response:** `204 No Content`

#### Example

```bash
curl -X DELETE http://localhost:8080/collections/product_embeddings
```

## Vectors

Manage vectors within a collection.

### Insert Vectors

Inserts a batch of vectors into a collection.

-   **Endpoint:** `POST /collections/{name}/vectors`
-   **Success Response:** `200 OK`

#### Request Body

| Field     | Type    | Description                                      |
| --------- | ------- | ------------------------------------------------ |
| `vectors` | `array` | An array of vector objects to insert.            |

Each vector object has the following fields:

| Field     | Type      | Description                                      |
| --------- | --------- | ------------------------------------------------ |
| `id`      | `string`  | A unique identifier for the vector.              |
| `vector`  | `array`   | The vector data as an array of floats.           |
| `payload` | `object` (optional) | A JSON object containing metadata for the vector. |

#### Example

```bash
curl -X POST http://localhost:8080/collections/product_embeddings/vectors \
  -H "Content-Type: application/json" \
  -d 
{
    "vectors": [
      {
        "id": "product_1",
        "vector": [0.1, 0.2, ..., 0.4],
        "payload": { "name": "Laptop", "price": 999.99 }
      }
    ]
  }
```

### Search Vectors

Performs a similarity search for vectors in a collection.

-   **Endpoint:** `POST /collections/{name}/search`
-   **Success Response:** `200 OK`

#### Request Body

| Field        | Type      | Description                                                                 |
| -------------- | --------- | --------------------------------------------------------------------------- |
| `vector`     | `array`   | The query vector.                                                           |
| `top_k`      | `integer` (optional) | The number of most similar vectors to return. Defaults to `10`.             |
| `timeout_ms` | `integer` (optional) | The search timeout in milliseconds. Defaults to `1000`.                     |
| `filter`     | `object` (optional)  | A filter to apply to the search. See the documentation for filter syntax. |

#### Example

```bash
curl -X POST http://localhost:8080/collections/product_embeddings/search \
  -H "Content-Type: application/json" \
  -d 
{
    "vector": [0.1, 0.2, ..., 0.4],
    "top_k": 10
  }
```

### Batch Search

Performs a batch of search queries in a single request.

-   **Endpoint:** `POST /collections/{name}/batch_search`
-   **Success Response:** `200 OK`

## Tenants

Manage tenants for multi-tenancy.

### Create a Tenant

-   **Endpoint:** `POST /tenants`

### Get a Tenant

-   **Endpoint:** `GET /tenants/{id}`

### List Tenants

-   **Endpoint:** `GET /tenants`

### Update a Tenant

-   **Endpoint:** `PUT /tenants/{id}`

### Delete a Tenant

-   **Endpoint:** `DELETE /tenants/{id}`

## System

### Health Checks

-   **Liveness:** `GET /health/live` - Returns `200 OK` if the service is running.
-   **Readiness:** `GET /health/ready` - Returns `200 OK` if the service is ready to accept traffic.
-   **Detailed Health:** `GET /health` - Returns a JSON object with detailed health information.

### Metrics

-   **Endpoint:** `GET /metrics` - Exposes Prometheus metrics.