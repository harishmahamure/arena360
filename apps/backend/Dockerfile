FROM rust:1-slim AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev curl ca-certificates && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock* ./
RUN mkdir -p src/bin && echo "fn main() {}" > src/main.rs && echo "fn main() {}" > src/bin/openapi_gen.rs
RUN cargo build --release && rm -rf src

COPY src ./src
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/gaming-cafe-api /usr/local/bin/

EXPOSE 3000

CMD ["gaming-cafe-api"]
