# Stage 1: Chef Planner
FROM --platform=$BUILDPLATFORM lukemathwalker/cargo-chef:latest-rust-alpine AS planner
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 2: Dependency Builder
FROM lukemathwalker/cargo-chef:latest-rust-alpine AS builder
WORKDIR /app
RUN apk add --no-cache build-base musl-dev sqlite-dev pkgconfig
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Stage 3: Application Compiler
COPY . .
RUN cargo build --release --bin keepup

# Final minimal & hardened runtime image
FROM alpine:3.21 AS runtime

RUN apk upgrade --no-cache && \
    apk add --no-cache ca-certificates tzdata sqlite-libs && \
    addgroup -S keepup && adduser -S keepup -G keepup && \
    mkdir -p /data /app && \
    chown keepup:keepup /data && \
    chmod 0700 /data

WORKDIR /app

# Binary is owned by root:root and read/execute-only (0555) so runtime user cannot modify it
COPY --from=builder --chown=root:root --chmod=0555 /app/target/release/keepup /app/keepup

USER keepup

ENV PORT=8080
ENV KEEPUP_DB_PATH=/data/keepup.db
EXPOSE 8080

ENTRYPOINT ["/app/keepup"]
