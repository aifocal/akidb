# Multi-arch Dockerfile for AkiDB 2.0
# Supports: linux/amd64, linux/arm64
# Optimized for: Apple Silicon (M1/M2/M3), AWS Graviton, NVIDIA Jetson, Oracle Cloud ARM

# ============================================================================
# Stage 1: Builder
# ============================================================================
FROM rust:1.75-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /build

# Copy manifests first for better layer caching
COPY Cargo.toml Cargo.lock ./
COPY crates/akidb-core/Cargo.toml crates/akidb-core/
COPY crates/akidb-metadata/Cargo.toml crates/akidb-metadata/
COPY crates/akidb-embedding/Cargo.toml crates/akidb-embedding/
COPY crates/akidb-index/Cargo.toml crates/akidb-index/
COPY crates/akidb-storage/Cargo.toml crates/akidb-storage/
COPY crates/akidb-service/Cargo.toml crates/akidb-service/
COPY crates/akidb-proto/Cargo.toml crates/akidb-proto/
COPY crates/akidb-grpc/Cargo.toml crates/akidb-grpc/
COPY crates/akidb-rest/Cargo.toml crates/akidb-rest/

# Create dummy source files to cache dependencies
RUN mkdir -p crates/akidb-core/src && echo "fn main() {}" > crates/akidb-core/src/lib.rs && \
    mkdir -p crates/akidb-metadata/src && echo "fn main() {}" > crates/akidb-metadata/src/lib.rs && \
    mkdir -p crates/akidb-embedding/src && echo "fn main() {}" > crates/akidb-embedding/src/lib.rs && \
    mkdir -p crates/akidb-index/src && echo "fn main() {}" > crates/akidb-index/src/lib.rs && \
    mkdir -p crates/akidb-storage/src && echo "fn main() {}" > crates/akidb-storage/src/lib.rs && \
    mkdir -p crates/akidb-service/src && echo "fn main() {}" > crates/akidb-service/src/lib.rs && \
    mkdir -p crates/akidb-proto/src && echo "fn main() {}" > crates/akidb-proto/src/lib.rs && \
    mkdir -p crates/akidb-grpc/src && echo "fn main() {}" > crates/akidb-grpc/src/main.rs && \
    mkdir -p crates/akidb-rest/src && echo "fn main() {}" > crates/akidb-rest/src/main.rs

# Copy proto files (needed for build)
COPY crates/akidb-proto/proto crates/akidb-proto/proto

# Build dependencies only (caching layer)
RUN cargo build --release -p akidb-rest -p akidb-grpc || true

# Remove dummy files
RUN find crates -name "*.rs" -type f -delete

# Copy all source code
COPY crates crates

# Build release binaries
RUN cargo build --release --workspace

# ============================================================================
# Stage 2: Runtime
# ============================================================================
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    sqlite3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash akidb && \
    mkdir -p /data /etc/akidb && \
    chown -R akidb:akidb /data /etc/akidb

# Set working directory
WORKDIR /app

# Copy binaries from builder
COPY --from=builder /build/target/release/akidb-rest /app/
COPY --from=builder /build/target/release/akidb-grpc /app/

# Copy example configuration
COPY config.example.toml /etc/akidb/config.toml

# Set ownership
RUN chown -R akidb:akidb /app /etc/akidb

# Switch to non-root user
USER akidb

# Expose ports
EXPOSE 8080 9090

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Default to REST server
# Override with: docker run ... /app/akidb-grpc
CMD ["/app/akidb-rest"]

# Labels
LABEL org.opencontainers.image.title="AkiDB" \
      org.opencontainers.image.description="ARM-optimized vector database with tiered storage" \
      org.opencontainers.image.version="2.0.0" \
      org.opencontainers.image.vendor="AkiDB Team" \
      org.opencontainers.image.licenses="Apache-2.0" \
      org.opencontainers.image.source="https://github.com/yourusername/akidb2" \
      org.opencontainers.image.documentation="https://docs.akidb.com"
