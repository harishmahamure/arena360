# DRAFT-0042: pg_trgm GIN indexes for admin list search

**Status**: Proposed
**Date**: 2026-06-20
**Deciders**: Platform team

**Relates to**: [ADR-0009](0009-rust-axum-backend.md)

## Context

Admin list endpoints filter rows with substring `ILIKE '%term%'` on text
columns (users, credit players, plans, products, devices). Leading wildcards
prevent B-tree indexes from being used, so PostgreSQL performs sequential scans
as tables grow.

Repositories affected in the first pass:

- `user_repo.rs` — `username`, `"phoneNumber"`
- `credit_repo.rs` — `username`, `"firstName"`, `"lastName"`, `"phoneNumber"`
- `plan_repo.rs` — `name`, `description`
- `product_repo.rs` — `name`
- `device_repo.rs` — `name`, `location`

Games, units, vendors, expense categories, and configurations use the same
`ILIKE` pattern but are deferred to a follow-up migration.

## Decision

1. Enable the **`pg_trgm`** extension (`CREATE EXTENSION IF NOT EXISTS pg_trgm`).
2. Add **partial GIN indexes** (`gin_trgm_ops`) on the hot search columns above,
   each with `WHERE "deletedAt" IS NULL` to match list-query filters.
3. **Keep existing `ILIKE` queries unchanged** — PostgreSQL uses trigram GIN
   indexes automatically for `ILIKE '%pattern%'` when `pg_trgm` is enabled.

## Consequences

### Positive

- Faster admin search on growing `users`, `products`, `plans`, and `devices`
  tables without API or repository changes.
- `pg_trgm` ships with the standard `postgres:16` image used in
  `infra/helm/postgresql`.

### Negative

- Additional index storage and slightly higher write cost on indexed columns.
- Index build locks tables briefly during migration (acceptable at current scale;
  `CREATE INDEX CONCURRENTLY` can be used manually on very large prod tables).

### Risks

- `pg_trgm` typically requires search terms of length ≥ 3 for index use;
  shorter terms may still seq-scan.
- Nullable text columns (`"firstName"`, `description`, `location`) index NULL
  rows in the partial index only when non-deleted; NULL values are not matched
  by `ILIKE` anyway.

## Alternatives Considered

### Keep sequential scans

Rejected: search latency will degrade as player and product catalogs grow.

### Prefix-only B-tree (`term%`)

Rejected: breaks substring search UX expected by admin filters.

### Full-text search (`tsvector` GIN)

Rejected: optimized for token/word search, not arbitrary substring `ILIKE`.

### External search engine (e.g. Meilisearch)

Rejected for now: operational overhead exceeds need for modest admin list
filters; can revisit if cross-entity unified search is required.

## Implementation Notes

Migration: `apps/backend/migrations/20260620000001_pg_trgm_search_indexes.sql`

Down migration drops indexes only; the `pg_trgm` extension remains installed.

Verify with `EXPLAIN (ANALYZE, BUFFERS)` on representative `ILIKE` queries;
expect `Bitmap Index Scan` on `*_trgm` indexes for terms of length ≥ 3.

## References

- `apps/backend/src/repositories/user_repo.rs` — `apply_filters`
- `apps/backend/src/repositories/credit_repo.rs` — player credit search
- `apps/backend/src/repositories/plan_repo.rs` — plan search
- `apps/backend/src/repositories/product_repo.rs` — product name filter
- `apps/backend/src/repositories/device_repo.rs` — device name/location filter
- [PostgreSQL pg_trgm documentation](https://www.postgresql.org/docs/current/pgtrgm.html)
