# Multi-Tenancy in AkiDB

AkiDB provides multi-tenancy features that allow you to isolate data and manage resources for different users or applications within a single AkiDB deployment.

## Tenant Isolation

Data isolation is a core aspect of AkiDB's multi-tenancy. Each tenant's data is stored separately from other tenants' data.

-   **API Key Authentication:** Each tenant is assigned a unique API key. All API requests must be authenticated with a valid tenant API key.
-   **Storage Isolation:** In the S3-compatible storage backend, all of a tenant's data, including their collections and vectors, is stored under a unique prefix that corresponds to the tenant ID. This ensures that one tenant cannot access another tenant's data.

## Quota Management

AkiDB allows you to set quotas for each tenant to control resource usage.

### Quota Types

You can define the following quotas for a tenant:

-   **Max Collections:** The maximum number of collections a tenant can create.
-   **Max Vectors:** The total number of vectors a tenant can store across all their collections.
-   **Max Storage:** The total amount of storage a tenant's data can consume.

### Quota Enforcement

Quotas are enforced at the API layer. When a tenant makes a request that would exceed their quota (e.g., creating a new collection when they have reached their maximum, or inserting vectors that would exceed their storage limit), the request is rejected with a `429 Too Many Requests` error.

## Managing Tenants

You can manage tenants through the REST API.

-   **Create a Tenant:** `POST /tenants`
-   **Get a Tenant:** `GET /tenants/{id}`
-   **Update a Tenant:** `PUT /tenants/{id}` (e.g., to change their quotas)
-   **Delete a Tenant:** `DELETE /tenants/{id}`

When you create a tenant, you can specify their initial quotas. If no quotas are specified, the tenant will have unlimited resources.

### Example: Creating a Tenant with Quotas

```bash
curl -X POST http://localhost:8080/tenants \
  -H "Content-Type: application/json" \
  -d \
'{
    "name": "limited-tenant",
    "quotas": {
      "max_collections": 10,
      "max_vectors": 1000000
    }
  }'
```

This will create a new tenant named `limited-tenant` that can create up to 10 collections and store a maximum of 1,000,000 vectors.
