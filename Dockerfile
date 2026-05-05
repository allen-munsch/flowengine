# ── Build Stage ──────────────────────────────────────────────
FROM rust:1.77-slim-bookworm AS builder

WORKDIR /app

# Copy workspace manifest and lock
COPY Cargo.toml Cargo.lock ./
COPY crates/flowcore/Cargo.toml crates/flowcore/
COPY crates/flowpersist/Cargo.toml crates/flowpersist/
COPY crates/flowruntime/Cargo.toml crates/flowruntime/
COPY crates/flownodes/Cargo.toml crates/flownodes/
COPY crates/flowserver/Cargo.toml crates/flowserver/
COPY crates/flowcli/Cargo.toml crates/flowcli/

# Dummy source stubs for dependency resolution
RUN mkdir -p crates/flowcore/src crates/flowpersist/src \
    crates/flowruntime/src crates/flownodes/src \
    crates/flowserver/src crates/flowcli/src && \
    for d in flowcore flowpersist flowruntime flownodes flowserver flowcli; do \
        echo 'fn main() {}' > crates/$d/src/lib.rs; \
    done && \
    echo '' > crates/flowserver/src/main.rs && \
    echo '' > crates/flowcli/src/main.rs

# Build dependencies (cached layer)
RUN cargo build --release -p flowserver 2>/dev/null; true

# Copy real source
COPY crates/flowcore/src crates/flowcore/src/
COPY crates/flowpersist/src crates/flowpersist/src/
COPY crates/flowruntime/src crates/flowruntime/src/
COPY crates/flownodes/src crates/flownodes/src/
COPY crates/flowserver/src crates/flowserver/src/
COPY crates/flowcli/src crates/flowcli/src/

# Build release
RUN cargo build --release -p flowserver && \
    strip target/release/flowserver

# ── Runtime Stage ────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/flowserver /usr/local/bin/flowserver

EXPOSE 3000

HEALTHCHECK --interval=10s --timeout=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

ENTRYPOINT ["/usr/local/bin/flowserver"]
