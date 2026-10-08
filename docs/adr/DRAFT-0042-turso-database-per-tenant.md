# DRAFT-0042: Turso database per tenant

**Status**: Rejected (2026-10-05)
**Date**: 2026-10-03
**Deciders**: Founder / backend owner
**Would have superseded**: ADR-0009 (PostgreSQL and SQLx portion only)

## Outcome

Rejected by the owner on 2026-10-05 in favour of staying on PostgreSQL. ADR-0009 and the shared-table tenancy in `docs/architecture/tenancy.md` remain in force. The cost of rewriting the schema and repositories outweighs the database savings at the current stage. Revisit if physical per-tenant isolation becomes a contractual requirement or per-tenant database cost becomes material.

The analysis below is kept for reference.

## Context

Arena360 is sold per PC (₹149 per PC per month; a typical venue has about 20 PCs) to many independent venue businesses. Tenant isolation is a product requirement: one business's data must not be reachable from another business's queries, backups, or restores.

The backend currently uses one PostgreSQL database with shared tables. Migrations `20261003150000` through `20261003210000` added `organizationId` to operational tables, but runtime reads still depend on every query carrying a tenant predicate, and non-default organizations are still blocked in `access/routes.rs`.

Constraints that drive this decision:

- **Cost at low revenue per tenant.** Per-tenant PostgreSQL (Neon project per tenant) costs roughly $9.50 per venue per month because venue databases stay awake during opening hours, about 28% of revenue. Turso bills rows read, rows written, and storage, which suits always-on, low-volume venues: an estimated $0.50 to $1.00 per venue per month at 1,000 venues (1.5–3% of revenue), with a free tier and a $4.99 Developer plan before that.
- **Operational load.** The company is a single developer. Managed databases are preferred over self-hosting database servers.
- **Isolation.** Physically separate databases remove the class of bugs where a missing `organizationId` predicate leaks data, and make per-tenant export, deletion, and restore simple.

Analytics (outbox to NATS JetStream to legacy reporting pipeline) and caching (Redis) stay as they are and remain on the application VM for now.

## Decision

1. **One Turso (libSQL) database per organization** holds that organization's operational data: locations, devices, players, sessions, transactions, inventory, shifts, cash, expenses, pricing, settings, notifications, and the tenant's analytics and realtime outboxes.
2. **One control-plane Turso database** holds global data: `users`, `organizations`, `organization_memberships`, authentication challenges, device-to-organization routing, and the tenant registry (database name, URL, schema version, status).
3. **Driver.** The backend uses the `libsql` Rust crate instead of `sqlx` with PostgreSQL. Repositories keep their current shape and receive a tenant connection instead of a `PgPool`.
4. **Tenant routing.** Middleware resolves the organization from the user or device JWT, looks it up in the tenant registry (cached in memory and Redis), and attaches a tenant database handle to the request. Requests without a resolvable tenant are rejected.
5. **Migrations.** Two embedded migration sets, `migrations/control/` and `migrations/tenant/`, start from a fresh SQLite baseline. A migration orchestrator applies tenant migrations to every tenant database, records the schema version per tenant in the registry, and retries failed tenants. New tenants are provisioned by creating the database, applying all tenant migrations, and seeding defaults.
6. **Type mapping from PostgreSQL to SQLite:**
   - UUIDs are generated in Rust (UUID v7) and stored as `TEXT`.
   - Timestamps are stored as `TEXT` in RFC 3339 UTC.
   - Money and quantities are stored as `INTEGER` fixed-point at scale 4 (value × 10,000), preserving the current `decimal(19,4)` precision and keeping SQL `SUM` exact. Conversion to `rust_decimal::Decimal` happens in the repository layer.
   - `jsonb` columns become `TEXT` with a `json_valid` check.
   - `plpgsql` triggers (audit columns, `updatedAt`, outbox capture) move into repository code, or into plain SQLite triggers where the logic is a single statement.
   - `SELECT ... FOR UPDATE` becomes a write transaction started with `BEGIN IMMEDIATE`. Turso serializes writers per database, which is acceptable at venue scale.
7. **Realtime.** PostgreSQL `LISTEN`/`NOTIFY` is replaced by Redis pub/sub after commit, with the per-tenant `realtime_outbox` table kept for durable replay.
8. **Analytics.** Each tenant database keeps an `analytics_outbox` written in the same transaction as the business change. The analytics worker iterates active tenants from the registry, polls each outbox every 5–10 seconds, adds the organization ID, and publishes to JetStream as today. The PostgreSQL advisory lock that keeps a single worker active is replaced by a Redis lease. legacy reporting pipeline projections and report queries are unchanged.
9. **Plans.** Turso Free during development, Developer ($4.99/month) from the first paying venue, Scaler once Developer quotas are exceeded (about 40–50 venues).

## Consequences

### Positive

- Physical tenant isolation; a missing tenant predicate can no longer leak another business's data.
- Per-tenant export, deletion, and point-in-time restore without touching other tenants.
- Near-zero database cost before revenue and an estimated 1.5–3% of revenue at 1,000 venues.
- No database servers to operate.
- The SQLite file format allows a later move to self-hosted SQLite or libSQL if Turso pricing or direction changes.

### Negative

- One-time rewrite of 119 migrations into a new baseline and porting of 461 runtime SQLx queries across 26 repositories (about 69 files reference PostgreSQL types).
- SQLite `ALTER TABLE` limits mean some future migrations require table rebuilds.
- Schema changes fan out to every tenant database; partial failure and version drift must be handled.
- No cross-tenant SQL. Platform-wide reporting comes only from legacy reporting pipeline.
- Two databases per request path (control plane for identity, tenant for data), mitigated by caching.
- Turso bills rows scanned, so unindexed queries and polling directly increase cost.

### Risks

| Risk | Mitigation |
|---|---|
| Money precision loss during migration | Fixed-point integer columns at scale 4; property tests comparing ported calculations against current results |
| Failed migration on some tenants | Orchestrator records per-tenant version, retries idempotently, blocks requests for tenants on an incompatible version |
| Row-read cost spikes | Index every hot query, serve dashboards from legacy reporting pipeline and Redis, alert on Turso usage |
| Free-plan 1-day restore window | Move to Developer (10-day restore) before onboarding the first paying venue |
| Turso vendor or product direction changes | Keep SQL portable SQLite; document export and self-host path (SQLite with Litestream) |
| Outbox polling cost and lag at many tenants | 5–10 second interval, indexed outbox, only poll tenants marked active |

## Alternatives Considered

### Shared PostgreSQL with row-level security
- Pros: matches the current code; low cost; no rewrite.
- Cons: logical isolation only; per-tenant restore and deletion are harder.
- Why rejected: physical isolation is a product requirement.

### PostgreSQL database per tenant (self-hosted or one Neon project with many databases)
- Pros: keeps all current SQL, triggers, and the analytics pipeline.
- Cons: one connection pool per tenant; tuning and operations grow past a few hundred tenants; shared compute means noisy neighbours.
- Why rejected: operational load for a single developer at the target tenant count.

### Neon project per tenant
- Pros: PostgreSQL compatible, managed, scale to zero.
- Cons: venues stay awake during opening hours, about $9.50 per venue per month (about 28% of revenue).
- Why rejected: cost.

### Self-hosted libSQL server or SQLite with Litestream
- Pros: no metered billing; cheapest at scale.
- Cons: backups, restore, high availability, and upgrades become our responsibility; libSQL server releases have stalled since February 2025.
- Why rejected for now: operational load. Kept as the exit path if Turso costs grow.

### Schema per tenant in PostgreSQL
- Pros: one database server; isolation by schema.
- Cons: slow migrations and catalog bloat at thousands of schemas; still logical isolation inside one server.
- Why rejected: weaker isolation for similar effort.

## Implementation Notes

Not implemented. The estimated effort was 9–12 developer-weeks: a 1–2 week spike porting sessions and transactions, foundations, repository porting, realtime and analytics changes, and a cutover with data reconciliation.

## References

- `docs/architecture/tenancy.md` (current shared-table tenancy)
- `docs/architecture/analytics.md` (outbox, JetStream, legacy reporting pipeline pipeline)
- `apps/backend/src/realtime/dispatcher.rs` (`PgListener` usage to replace)
- Turso pricing: https://turso.tech/pricing (checked 2026-10-03)
- Neon pricing: https://neon.com/pricing (checked 2026-10-03)
- The ADR-0009 text is not present in this repository; its scope is taken from the ADR index in `.cursor/rules/20-adr-discipline.mdc`.
