# AkiDB Development Environment

This guide describes how to provision the Phase 1 development environment with Docker and the helper scripts provided in this repository.

## Prerequisites
- Docker Engine 24+ with the Compose plugin.
- Rust toolchain (stable channel) for local builds outside containers.
- `bash` for running the helper scripts.

## Environment Variables
The repository includes `.env.example` with sane defaults:

```
COMPOSE_PROJECT_NAME=akidb
MINIO_ROOT_USER=akidb
MINIO_ROOT_PASSWORD=akidbsecret
AKIDB_S3_ENDPOINT=http://minio:9000
AKIDB_S3_BUCKET=akidb
AKIDB_BIND_ADDRESS=0.0.0.0:8080
AKIDB_LOG_LEVEL=info
```

Create your working copy:

```bash
cp .env.example .env
```

Modify any values as needed. The `.env` file is ignored by Git.

## Bootstrapping with Docker

```bash
./scripts/dev-init.sh
```

The script performs the following:
- Ensures `.env` exists (creates it from the template if missing).
- Brings up the Docker Compose stack (`minio`, `akidb-server`, optional dev tools).
- Creates the configured MinIO bucket using the MinIO client container.

> Re-run with `--force-recreate` to rebuild images after changing dependencies:  
> `./scripts/dev-init.sh --force-recreate`

### Services
- **minio** – S3-compatible object storage for snapshots and WAL files. Console exposed on `${MINIO_CONSOLE_PORT:-9001}`.
- **akidb-server** – Rust service built via the multi-stage `Dockerfile`. Currently a placeholder awaiting full implementation.
- **devtools** *(optional)* – Interactive Rust toolchain container. Start it with:

  ```bash
  docker compose --profile devtools up -d devtools
  docker compose exec devtools bash
  ```

### Ports
- API: `localhost:${AKIDB_PORT:-8080}`
- MinIO API: `localhost:${MINIO_API_PORT:-9000}`
- MinIO Console: `localhost:${MINIO_CONSOLE_PORT:-9001}`

## Working with MinIO
- Access the web console at `http://localhost:9001` (use the root credentials from `.env`).
- Additional buckets can be created with the MinIO client:

  ```bash
  docker run --rm \
    --network "${COMPOSE_PROJECT_NAME:-akidb}_default" \
    -e "MC_HOST_local=http://${MINIO_ROOT_USER}:${MINIO_ROOT_PASSWORD}@minio:9000" \
    minio/mc mb local/another-bucket
  ```

## Local Development Workflow
1. Make code changes.
2. Run validations: `./scripts/dev-test.sh`.
3. Build release artifact when ready: `./scripts/build-release.sh`.
4. Tear down the stack if needed: `docker compose down -v`.

## Release Builds
The multi-stage `Dockerfile` produces a slim runtime image. To build locally:

```bash
docker build \
  --build-arg RUST_VERSION=1.77 \
  --build-arg APP_NAME=akidb-api \
  -t akidb/server:dev .
```

The release script places the binary at `dist/akidb-server`.

## Troubleshooting
- **MinIO bucket missing** – Run `./scripts/dev-init.sh --force-recreate`.
- **Port collisions** – Override the port variables in `.env`.
- **Slow recompiles** – Use the `devtools` profile; it mounts the workspace and caches Cargo artifacts.

Automate everything, monitor everything, break nothing.
