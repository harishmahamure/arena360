# ADR-0009: Rust Axum backend replaces NestJS

**Status**: Accepted
**Date**: 2026-05-27
**Deciders**: Platform team (gaming-cafe consolidation working group)

## Context

The existing backend (`arena360-backend/`) is a NestJS application on
Fastify with TypeORM for Postgres access. During the monorepo
consolidation, the decision was taken to rewrite the backend in Rust
rather than port the NestJS codebase, for the following reasons:

1. **Performance**: A Rust binary eliminates GC pauses and delivers
   sub-millisecond p99 latency on CRUD endpoints. The gaming-cafe
   session-poll endpoint (`US-SESSION-004`) benefits directly.
2. **Memory footprint**: A Rust backend uses 10-30 MB RSS vs 200+ MB
   for a Node.js process, enabling cheaper deployment on the single-node
   cluster.
3. **Type safety**: Rust's ownership system and `sqlx` compile-time
   query checking eliminate entire categories of runtime errors
   (null pointer, SQL injection, type mismatches).
4. **SSE support**: Axum + tokio provide first-class async streaming
   with backpressure, ideal for the planned Server-Sent Events feature.
5. **Simplified deployment**: A single static binary in a scratch
   Docker image (~20 MB) vs a Node.js image (~200+ MB).

The existing Postgres database schema (created by TypeORM migrations)
is reused as-is. The Rust backend connects to the same database and
must produce API responses compatible with the existing admin frontend.

## Decision

We will write a **new Rust backend** using:

- **Axum 0.7** as the HTTP framework
- **SQLx 0.7** for async Postgres access (compile-time checked queries)
- **utoipa** for OpenAPI spec generation (source of truth for
  `@gaming-cafe/api-types`)
- **jsonwebtoken** for JWT validation (compatible with existing tokens)
- **tokio::broadcast** for SSE event distribution
- **tower-http** for CORS, tracing, compression middleware

### Architecture (layered)

```
src/
├── config/         # App settings, DB pool
├── handlers/       # HTTP handlers (routes)
├── services/       # Business logic
├── repositories/   # Data access (SQLx queries)
├── models/         # Domain models (FromRow)
├── dto/            # Request/Response (serde + utoipa)
├── middleware/     # Auth, logging, error handling
├── error/          # AppError enum
└── sse/            # Event broadcaster
```

### API compatibility

The Rust backend must produce the same response envelope:

```json
{
  "data": <T>,
  "statusCode": 200,
  "message": "OK"
}
```

Error responses use the same `ErrorCode` enum values from
`@gaming-cafe/contracts`.

### JWT compatibility

The backend validates existing JWTs signed with HS256 using the same
`JWT_SECRET`. Existing admin sessions continue to work without
re-authentication.

## Consequences

### Positive

- **10-100x lower latency** on hot paths vs NestJS.
- **10x smaller Docker image** (~20 MB vs ~200 MB).
- **Compile-time SQL checking** — no runtime SQL errors in production.
- **Zero GC pauses** — consistent latency under load.
- **Native SSE support** — no additional WebSocket gateway needed.
- **Memory efficiency** — enables running on smaller/cheaper VMs.

### Negative

- **Higher learning curve** — team must know Rust.
- **Slower iteration** — compile times longer than TypeScript hot-reload.
- **No ORM migrations** — schema changes require manual SQL migrations
  (SQLx migrations or raw SQL files).
- **Ecosystem maturity** — fewer middleware/plugins than Express/NestJS
  ecosystem (mitigated by Axum/tower's composability).

### Risks

- **Risk**: Existing TypeORM schema has conventions (e.g. column naming,
  JSON columns) that are awkward to map in SQLx.
  **Mitigation**: Use `sqlx::FromRow` with `#[sqlx(rename_all)]` and
  explicit column mappings where needed.
- **Risk**: Admin frontend relies on undocumented backend behaviors.
  **Mitigation**: Run admin against Rust backend in integration tests;
  fix discrepancies iteratively.
- **Risk**: Compile-time query checking requires a live DB connection
  during `cargo build` (or offline mode with `sqlx-data.json`).
  **Mitigation**: CI uses `sqlx prepare` to generate offline data; local
  dev connects to a local Postgres.

## Alternatives considered

### Port NestJS to the monorepo as-is

- Pros: No rewrite effort; familiar codebase.
- Cons: Carries forward all existing tech debt, no performance gains,
  no SSE without additional gateway.
- **Why rejected**: Misses the opportunity to solve performance and
  deployment issues.

### Port NestJS to the monorepo + add SSE via Socket.io gateway

- Pros: Less effort than full rewrite; gets SSE.
- Cons: Two processes to deploy (API + gateway); Node.js overhead;
  TypeORM tech debt persists.
- **Why rejected**: Adds operational complexity without addressing root
  performance and deployment concerns.

### Go (Gin/Echo) backend

- Pros: Fast compilation, simple deployment, good performance.
- Cons: Less type safety than Rust; no compile-time SQL checking;
  cargo already available on the team's machines (Tauri work).
- **Why rejected**: Rust already in the team's toolchain; SQLx compile-
  time checking is a significant advantage over Go's runtime SQL.

## Implementation notes

- The Rust backend lives at `apps/backend/` in the monorepo.
- It reuses the same Postgres database — no data migration needed.
- The OpenAPI spec is generated by `utoipa` and exported as
  `apps/backend/docs/openapi.json` for the `@gaming-cafe/api-types`
  codegen pipeline.
- The admin frontend (`apps/admin`) calls the Rust backend at the same
  `/api/v1/*` path prefix.
- The `pnpm-workspace.yaml` does not include `apps/backend` (it's a
  Cargo project, not a Node package).
- CI runs `cargo build`, `cargo test`, `cargo clippy` for the backend.

## References

- ADR-0004: Shared API types via OpenAPI (now generated from Rust/utoipa)
- ADR-0008: Runtime contracts package (ErrorCode enum consumed by Rust
  error handler for API compatibility)
- `docs/ARCHITECTURE.md` — updated with Rust architecture
