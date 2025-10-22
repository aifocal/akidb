#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="${PROJECT_ROOT}/.env"
ENV_EXAMPLE="${PROJECT_ROOT}/.env.example"

if ! command -v docker &>/dev/null; then
  echo "docker is required but is not installed or not in PATH." >&2
  exit 1
fi

if ! command -v docker compose &>/dev/null; then
  echo "docker compose plugin is required but not available." >&2
  exit 1
fi

if [[ ! -f "${ENV_FILE}" ]]; then
  if [[ -f "${ENV_EXAMPLE}" ]]; then
    echo "Creating .env from template..."
    cp "${ENV_EXAMPLE}" "${ENV_FILE}"
  else
    echo "Environment template ${ENV_EXAMPLE} not found." >&2
    exit 1
  fi
fi

if [[ -n "${1:-}" && "$1" == "--force-recreate" ]]; then
  FORCE_ARGS="--force-recreate --build"
else
  FORCE_ARGS=""
fi

echo "Bringing up development stack..."
docker compose --project-directory "${PROJECT_ROOT}" up -d ${FORCE_ARGS}

set -a
source "${ENV_FILE}"
set +a

COMPOSE_PROJECT="${COMPOSE_PROJECT_NAME:-akidb}"
MINIO_NETWORK="${COMPOSE_PROJECT}_default"

echo "Ensuring MinIO bucket ${AKIDB_S3_BUCKET} exists..."
docker run --rm \
  --network "${MINIO_NETWORK}" \
  -e "MC_HOST_local=http://${MINIO_ROOT_USER}:${MINIO_ROOT_PASSWORD}@minio:9000" \
  minio/mc:RELEASE.2024-05-28T16-37-28Z \
  mb --ignore-existing "local/${AKIDB_S3_BUCKET}"

echo "Development environment is ready."
