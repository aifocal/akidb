# syntax=docker/dockerfile:1.6

ARG RUST_VERSION=1.77
ARG APP_NAME=akidb-api

FROM rust:${RUST_VERSION}-slim-bullseye AS builder

ENV DEBIAN_FRONTEND=noninteractive
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
        ca-certificates \
        build-essential \
    && rm -rf /var/lib/apt/lists/*

RUN rustup component add clippy rustfmt

# Copy manifests first to leverage Docker layer caching.
COPY Cargo.toml Cargo.toml
COPY crates/akidb-core/Cargo.toml crates/akidb-core/Cargo.toml
COPY crates/akidb-storage/Cargo.toml crates/akidb-storage/Cargo.toml
COPY crates/akidb-index/Cargo.toml crates/akidb-index/Cargo.toml
COPY crates/akidb-query/Cargo.toml crates/akidb-query/Cargo.toml
COPY services/akidb-api/Cargo.toml services/akidb-api/Cargo.toml

RUN mkdir -p crates/akidb-core/src \
    crates/akidb-storage/src \
    crates/akidb-index/src \
    crates/akidb-query/src \
    services/akidb-api/src

RUN cargo fetch

# Copy the rest of the workspace.
COPY . .

# Build the release binary for the API service.
RUN cargo build --package ${APP_NAME} --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

ENV APP_USER=akidb \
    APP_HOME=/home/akidb \
    RUST_LOG=info

RUN useradd -m -d "${APP_HOME}" "${APP_USER}"

WORKDIR ${APP_HOME}

COPY --from=builder /app/target/release/${APP_NAME} /usr/local/bin/akidb-server

USER ${APP_USER}

EXPOSE 8080

CMD ["akidb-server"]
