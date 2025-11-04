# AkiDB Security

AkiDB is designed with security in mind, especially for deployment in sensitive, air-gapped environments. This document outlines the key security features and best practices for securing your AkiDB installation.

**Recent Security Improvements (v1.1.0):**

The v1.1.0 release includes several critical security fixes. It is highly recommended to upgrade to this version to ensure your deployment is secure.

## Authentication

All API requests to AkiDB must be authenticated.

### API Key Authentication

AkiDB uses API keys for authentication. You can provide an API key in one of two ways:

-   **`X-API-Key` Header (Recommended):**

    ```bash
    curl -H "X-API-Key: <your-api-key>" http://localhost:8080/collections
    ```

-   **`Authorization` Header:**

    ```bash
    curl -H "Authorization: Bearer <your-api-key>" http://localhost:8080/collections
    ```

**Security Note:** API keys are configured via the `AKIDB_API_KEYS` environment variable. To prevent timing attacks, AkiDB uses constant-time comparison for validating API keys.

### JWT-based Authentication for RBAC

For Role-Based Access Control (RBAC), AkiDB uses JSON Web Tokens (JWTs). The JWT should be passed in the `Authorization` header.

## Authorization

AkiDB implements Role-Based Access Control (RBAC) to control access to resources.

-   **Permissions:** AkiDB has a granular set of permissions, such as `CollectionCreate`, `VectorSearch`, and `TenantAdmin`.
-   **Roles:** Roles are collections of permissions. You can create roles like `ReadOnlyUser` or `DataScientist`.
-   **Users:** Users are assigned roles, which grant them the permissions associated with those roles.

This ensures that users can only perform actions that they are explicitly authorized to do.

## Multi-Tenancy Security

AkiDB's multi-tenancy features are designed to be secure:

-   **Tenant ID Required:** All requests must include a tenant ID, either in the `X-Tenant-ID` header or inferred from the API key. This prevents accidental cross-tenant data access.
-   **Tenant Status:** Tenants can have a status of `Active`, `Suspended`, or `Deleted`. Requests from suspended or deleted tenants are blocked.
-   **Quota Enforcement:** Resource quotas are strictly enforced to prevent abuse and ensure fair resource allocation among tenants.

## Data Security

-   **S3 Integration:** By leveraging S3-compatible storage like MinIO, you can take advantage of features like:
    -   Server-Side Encryption (SSE) with KMS.
    -   Object Lock (WORM) for immutable index segments.
    -   Versioning for data rollback and recovery.
-   **Write-Ahead Log (WAL):** The WAL ensures that your data is durable and that the database can recover from crashes without data loss.

## Best Practices

-   **Upgrade to the Latest Version:** Always run the latest version of AkiDB to benefit from the latest security patches.
-   **Use a Secrets Manager:** Store API keys and JWT secrets in a secure secrets manager like HashiCorp Vault.
-   **Configure API Keys Securely:** Ensure that `AKIDB_AUTH_ENABLED` is set to `true` in production and that `AKIDB_API_KEYS` is populated with strong, unique keys.
-   **Network Policies:** Restrict access to the AkiDB API endpoint and the `/metrics` endpoint using network policies or firewall rules.
-   **Secure your S3 Bucket:** Follow security best practices for your S3-compatible storage, including bucket policies, access control, and encryption.
