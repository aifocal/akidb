# Deploying AkiDB to Production

This guide provides recommendations for deploying AkiDB to a production environment.

## Target Platforms

AkiDB is optimized for ARM-based platforms. The recommended platforms for production deployments are:

-   **Apple Silicon:** Mac Minis or Mac Studios with M-series chips offer excellent performance per watt.
-   **NVIDIA Jetson:** For edge deployments, the NVIDIA Jetson family (Orin Nano, Orin NX, AGX Orin) provides powerful AI inference capabilities in a small form factor.
-   **ARM Cloud Servers:** Cloud providers like AWS (Graviton), Oracle (A1), and Azure (Cobalt) offer ARM-based instances that are cost-effective and power-efficient.

## Deployment Architecture

A typical production deployment consists of:

1.  **AkiDB Instances:** One or more stateless AkiDB instances running on your chosen ARM platform. You can run multiple instances behind a load balancer for high availability and scalability.
2.  **MinIO Cluster:** A production-grade MinIO cluster for S3-compatible object storage. It is recommended to run MinIO in a distributed setup for high availability and data redundancy.
3.  **Monitoring Stack:** A Prometheus server for scraping metrics from the AkiDB `/metrics` endpoint, and a Grafana instance for visualizing those metrics.

## Configuration

It is crucial to configure your AkiDB instances securely for production.

### Environment Variables

Set the following environment variables:

-   `AKIDB_AUTH_ENABLED=true`: Ensure that authentication is enabled.
-   `AKIDB_API_KEYS`: A comma-separated list of strong, unique API keys.
-   `AKIDB_JWT_SECRET`: A strong, unique secret for signing and verifying JWTs for RBAC.
-   `MINIO_ENDPOINT`: The URL of your MinIO cluster.
-   `MINIO_ACCESS_KEY` and `MINIO_SECRET_KEY`: Credentials for accessing your MinIO cluster.

**Security Note:** Use a secrets management system like HashiCorp Vault to manage your API keys, JWT secret, and MinIO credentials.

## High Availability

To achieve high availability:

-   **Run Multiple AkiDB Instances:** Run at least two AkiDB instances behind a load balancer. Since the instances are stateless, you can easily scale the number of instances up or down based on your workload.
-   **Use a Distributed MinIO Cluster:** A distributed MinIO setup will ensure that your data is still available even if one of the storage nodes fails.
-   **Leverage MinIO Site Replication:** For disaster recovery, you can use MinIO's site replication feature to replicate your data to a different geographical location.

## Monitoring

AkiDB exposes a `/metrics` endpoint with Prometheus-compatible metrics. Set up a Prometheus server to scrape this endpoint and use Grafana to create dashboards for monitoring the health and performance of your AkiDB cluster. Key metrics to monitor include:

-   `akidb_api_requests_total`: The total number of API requests.
-   `akidb_query_latency_seconds`: The latency of search queries.
-   `akidb_wal_size_bytes`: The size of the Write-Ahead Log.

## Backups

Since AkiDB uses MinIO as its primary storage, you can use standard MinIO backup strategies. You can take snapshots of your S3 buckets or use MinIO's client (`mc`) to mirror your data to a backup location.
