# ==============================================================================
# Stage 1: Builder (Rust Alpine for Rust Edition 2024 & Modern Crates Support)
# ==============================================================================
FROM rust:alpine AS builder

WORKDIR /app

# Install build dependencies for static musl compilation
RUN apk add --no-cache musl-dev pkgconfig openssl-dev openssl-libs-static

# Dependency layer caching: Copy manifest files and pre-compile dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src static migrations && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy real project sources, migrations, and static frontend assets
COPY . .

# Recompile standalone binary with real project source code
RUN touch src/main.rs && cargo build --release

# ==============================================================================
# Stage 2: Minimal Production Runtime (Alpine 3.21)
# ==============================================================================
FROM alpine:3.21 AS runtime

WORKDIR /app

# Install runtime utilities:
# - ca-certificates: TLS probe verification
# - tzdata: UTC/local timestamping for incident logs
# - curl: container healthcheck probing
RUN apk add --no-cache ca-certificates tzdata curl

# Create persistent storage directory for the SQLite database
RUN mkdir -p /app/data

# Copy compiled standalone executable from Builder stage
COPY --from=builder /app/target/release/keepup /usr/local/bin/keepup

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
ENTRYPOINT ["/usr/local/bin/keepup"]
