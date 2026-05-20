# ── Build Stage ──────────────────────────────────────────────
FROM rust:1.91-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev curl protobuf-compiler && rm -rf /var/lib/apt/lists/*
WORKDIR /app

# Copy everything and build
COPY . .
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
EXPOSE 3001

HEALTHCHECK --interval=10s --timeout=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

ENTRYPOINT ["/usr/local/bin/flowserver"]
