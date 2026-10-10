# All targets use repository-root build context.
ARG RUST_BUILD_BASE=rust:1-bookworm
FROM ${RUST_BUILD_BASE} AS source
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev curl ca-certificates unzip && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY tools/backend-cli ./tools/backend-cli
COPY apps/backend ./apps/backend
COPY apps/tenant-storage ./apps/tenant-storage
COPY apps/tenant-gateway ./apps/tenant-gateway

FROM source AS builder
ARG CARGO_BUILD_JOBS=2
ENV CARGO_BUILD_JOBS=$CARGO_BUILD_JOBS
RUN cargo build --locked --release -p tenant-gateway -p tenant-storage -p gaming-cafe-api -p arena360-tools --bin tenant-gateway --bin tenant-storage --bin gaming-cafe-api --bin demo_seed --bin tenant_events_setup --bin schema_rollout --bin tenant_recover --bin tenant_move --bin rebalance_cells --bin tenant_cold --bin platform_operator

FROM debian:bookworm-slim AS gateway
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/tenant-gateway /usr/local/bin/
RUN useradd --uid 10001 --create-home arena360
USER 10001:10001
EXPOSE 3000
CMD ["tenant-gateway"]

FROM debian:bookworm-slim AS storage-runtime
RUN apt-get update && apt-get install -y ca-certificates curl libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/tenant-storage /app/target/release/demo_seed /app/target/release/tenant_events_setup /app/target/release/schema_rollout /app/target/release/tenant_recover /app/target/release/tenant_move /app/target/release/rebalance_cells /app/target/release/tenant_cold /app/target/release/platform_operator /usr/local/bin/
RUN useradd --uid 10001 --create-home arena360 && mkdir -p /data/tenants /run/arena360-backup-keys && chown -R arena360:arena360 /data /run/arena360-backup-keys
USER 10001:10001

FROM storage-runtime AS storage
ENV PORT=3001
EXPOSE 3001
CMD ["tenant-storage"]

# Existing single-process deployment remains the default target.
FROM storage-runtime AS backend
COPY --from=builder /app/target/release/gaming-cafe-api /usr/local/bin/
EXPOSE 3000
CMD ["gaming-cafe-api"]
