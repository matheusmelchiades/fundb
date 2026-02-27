# ── Stage 1: builder ──────────────────────────────────────────────────────────
FROM rust:1.82-slim AS builder

# Install system deps needed for linking
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy workspace manifests first for better layer caching
COPY Cargo.toml Cargo.lock ./
COPY crates/fundb-core/Cargo.toml         crates/fundb-core/Cargo.toml
COPY crates/fundb-storage/Cargo.toml      crates/fundb-storage/Cargo.toml
COPY crates/fundb-indexes/Cargo.toml      crates/fundb-indexes/Cargo.toml
COPY crates/fundb-sql/Cargo.toml          crates/fundb-sql/Cargo.toml
COPY crates/fundb-optimizer/Cargo.toml    crates/fundb-optimizer/Cargo.toml
COPY crates/fundb-executor/Cargo.toml     crates/fundb-executor/Cargo.toml
COPY crates/fundb-cognitive/Cargo.toml    crates/fundb-cognitive/Cargo.toml
COPY crates/fundb-causal/Cargo.toml       crates/fundb-causal/Cargo.toml
COPY crates/fundb-semantic/Cargo.toml     crates/fundb-semantic/Cargo.toml
COPY crates/fundb-learning/Cargo.toml     crates/fundb-learning/Cargo.toml
COPY crates/fundb-raft/Cargo.toml         crates/fundb-raft/Cargo.toml
COPY crates/fundb-cluster/Cargo.toml      crates/fundb-cluster/Cargo.toml
COPY crates/fundb-protocol/Cargo.toml     crates/fundb-protocol/Cargo.toml
COPY crates/fundb-server/Cargo.toml       crates/fundb-server/Cargo.toml
COPY crates/fundb-cli/Cargo.toml          crates/fundb-cli/Cargo.toml

# Create stub lib/main files so cargo can resolve the dependency graph
RUN for crate in fundb-core fundb-storage fundb-indexes fundb-sql fundb-optimizer \
        fundb-executor fundb-cognitive fundb-causal fundb-semantic fundb-learning \
        fundb-raft fundb-cluster fundb-protocol; do \
      mkdir -p crates/$crate/src && echo "// stub" > crates/$crate/src/lib.rs; \
    done && \
    mkdir -p crates/fundb-server/src && echo "fn main(){}" > crates/fundb-server/src/main.rs && \
    mkdir -p crates/fundb-cli/src    && echo "fn main(){}" > crates/fundb-cli/src/main.rs

# Pre-fetch + compile deps only (cache layer)
RUN cargo build --release -p fundb-server -p fundb-cli 2>/dev/null || true

# Now copy the real source and rebuild
COPY crates/ crates/

# Touch all main/lib files so cargo notices the change
RUN find crates -name "*.rs" -exec touch {} \;

RUN cargo build --release -p fundb-server -p fundb-cli

# ── Stage 2: runtime ──────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -r -s /bin/false -m -d /data fundb

# Copy binaries
COPY --from=builder /build/target/release/fundb-server /usr/local/bin/fundb-server
COPY --from=builder /build/target/release/fundb        /usr/local/bin/fundb

# Data directory for WAL / SSTable files
RUN mkdir -p /data && chown fundb:fundb /data

USER fundb
WORKDIR /data

# PG wire protocol
EXPOSE 5433
# HTTP REST API
EXPOSE 8080

ENV RUST_LOG=info
ENV FUNDB_DATA_DIR=/data
ENV FUNDB_PG_PORT=5433
ENV FUNDB_HTTP_PORT=8080
ENV FUNDB_NODE_ID=1

ENTRYPOINT ["/usr/local/bin/fundb-server"]
