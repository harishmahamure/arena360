# DRAFT-0043: Redis cache layer for hot read paths

**Status**: Proposed
**Date**: 2026-06-20
**Deciders**: Platform team

**Relates to**: [ADR-0009](0009-rust-axum-backend.md), [ADR-0013](0013-realtime-websocket-channel.md)

## Context

Admin and kiosk flows repeatedly read reference data (plans, products, units,
configurations, games), session state, auth profiles, and computed aggregates.
PostgreSQL handles all reads today. As backend replicas scale horizontally
(HPA 2–4 pods), identical queries hit Postgres from every pod.

ADR-0013 rejected Redis for realtime pub/sub at single-node scale but noted it
as a future cross-pod fanout option. This ADR adopts Redis for **read caching**
only; realtime remains on Postgres outbox + `pg_notify`.

Query/index optimizations (DRAFT-0042, B-tree indexes, query rewrites) reduce
per-query cost but do not deduplicate reads across pods.

## Decision

1. Add **Redis 7** as a new infrastructure service (docker-compose locally,
   Helm chart in `infra/helm/redis/`).
2. Use **`deadpool-redis`** + **`redis`** crates in the Axum backend for async
   connection pooling.
3. Implement **cache-aside** at the service layer via a `CacheService` trait
   with `RedisCache` (production) and `NoopCache` (tests / Redis unavailable).
4. **Write-through invalidation**: on every mutation, DEL affected keys and
   `PUBLISH` to channel `cache:invalidate` for cross-pod busting.
5. **Long TTLs** (hours) as safety nets; freshness comes from invalidation.
6. **Graceful degradation**: Redis failure logs a warning and falls through to
   Postgres; readiness probe reports Redis status but liveness does not fail.

### Cache domains (phased)

| Phase | Keys | TTL | Invalidation |
|-------|------|-----|--------------|
| 1 | plans, products, units, configs, expense categories, games | 24 hr | On CRUD |
| 2 | open sessions, active balances | 1 hr | On session/balance mutation |
| 3 | user by username/id, JWT blacklist, OTP rate limit | 12 hr / token life / 60s | On user update |
| 4 | credit outstanding, cash register totals, stock levels | 2 hr | On relevant writes |

## Consequences

### Positive

- Dramatically reduced Postgres read load for reference data and hot paths.
- Cross-pod cache coherence via Redis pub/sub invalidation.
- Enables PgBouncer transaction pooling without multiplying identical reads.

### Negative

- New operational dependency (Redis persistence, memory limits, monitoring).
- Cache invalidation bugs can serve stale data until TTL expiry.
- Additional complexity in service layer.

### Risks

- Redis outage → degraded performance, not outage (graceful fallback).
- Memory pressure → `maxmemory-policy allkeys-lru` evicts cold keys.
- Stale reads if invalidation missed → mitigated by long-but-finite TTLs.

## Alternatives Considered

### In-process LRU only

Rejected: no cross-pod sharing; each replica hits DB independently.

### Postgres materialized views

Rejected: refresh lag, no cross-request dedup within a pod, heavier writes.

### CDN / HTTP cache headers

Rejected: admin API is authenticated; not suitable for shared edge cache.

## Implementation Notes

- Config: `REDIS_URL` env var; optional (NoopCache when unset).
- Serialization: JSON via `serde_json` for debug visibility.
- Helm: single-node Redis StatefulSet with AOF persistence.
- Local: `docker-compose.yml` with Postgres, PgBouncer (transaction mode), Redis.

## References

- Query/index audit plan (2026-06-20)
- [ADR-0013](0013-realtime-websocket-channel.md) — Redis deferred for realtime
- `apps/backend/src/cache/` — implementation module
