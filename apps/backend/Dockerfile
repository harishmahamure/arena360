FROM rust:1-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev curl ca-certificates unzip && rm -rf /var/lib/apt/lists/*

COPY scripts/install-duckdb-sdk.sh /tmp/install-duckdb-sdk.sh
RUN sh /tmp/install-duckdb-sdk.sh /opt/duckdb-sdk
ENV DUCKDB_LIB_DIR=/opt/duckdb-sdk LD_LIBRARY_PATH=/opt/duckdb-sdk

COPY Cargo.toml Cargo.lock* build.rs ./
COPY proto ./proto
RUN mkdir -p src/bin && echo "pub fn placeholder() {}" > src/lib.rs && echo "fn main() {}" > src/main.rs && echo "fn main() {}" > src/bin/openapi_gen.rs
RUN cargo build --release --no-default-features --features duckdb-analytics && rm -rf src

COPY src ./src
COPY migrations ./migrations
COPY proto ./proto
COPY build.rs ./build.rs
RUN touch src/main.rs src/lib.rs src/bin/*.rs && cargo build --release --no-default-features --features duckdb-analytics --bins

RUN DUCKDB_EXTENSION_DIR=/opt/duckdb-extensions target/release/analytics_extensions_setup

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates curl libstdc++6 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /opt/duckdb-sdk/libduckdb.so /usr/local/lib/
COPY --from=builder /opt/duckdb-extensions /opt/duckdb-extensions
ENV LD_LIBRARY_PATH=/usr/local/lib DUCKDB_EXTENSION_DIR=/opt/duckdb-extensions

COPY --from=builder /app/target/release/gaming-cafe-api /app/target/release/demo_seed /app/target/release/tenant_events_setup /app/target/release/analytics_extensions_setup /app/target/release/archive_month /app/target/release/archive_backfill /app/target/release/export_worker /usr/local/bin/

EXPOSE 3000

CMD ["gaming-cafe-api"]
