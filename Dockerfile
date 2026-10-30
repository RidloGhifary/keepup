# ==============================================================================
# Stage 1: Builder
# ==============================================================================
FROM rust:1.82-slim AS builder

WORKDIR /usr/src/keepup

# Install essential build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Dependency layer caching: Copy manifest files and build dependencies with a dummy entrypoint
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src static migrations && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy real project sources, migrations, and static frontend assets
COPY migrations ./migrations
COPY static ./static
COPY src ./src

# Touch main.rs to ensure Cargo recompiles the real binary rather than using the dummy build
RUN touch src/main.rs && cargo build --release

# ==============================================================================
# Stage 2: Minimal Production Runtime
# ==============================================================================
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# Install runtime utilities:
# - ca-certificates: essential for reqwest HTTPS probe verification
# - tzdata: accurate UTC/local timestamping for incident logs
# - curl: container healthcheck probing
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    tzdata \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Dedicated persistent storage directory for the SQLite database
RUN mkdir -p /app/data

# Copy compiled standalone executable from Builder stage
COPY --from=builder /usr/src/keepup/target/release/keepup /app/keepup

# Expose internal Axum web server port
EXPOSE 3000

# Default environment configuration
ENV PORT=3000 \
    RUST_LOG=info,keepup=debug \
    KEEPUP_DB_PATH=/app/data/keepup.db \
    DATABASE_URL=sqlite:///app/data/keepup.db?mode=rwc \
    ADMIN_USER=admin \
    ADMIN_PASSWORD=keepup123

# Launch the single-binary application
ENTRYPOINT ["/app/keepup"]
