# Data Platform Progress

**Updated:** 2026-10-07  
**Branch:** `platform-v2`  
**Source plan:** `docs/plans/data-platform-build-plan.md`  
**Architecture source:** `docs/architecture/data-platform.md`

## Current position

- [x] M0 — Baseline and groundwork
- [x] M1 — Control plane
- [x] M2 — Ownership and routing
- [x] M3 — Tenant storage foundation
- [ ] M4 — Core venue operations on SQLite
- [ ] M5 — Back office on SQLite
- [ ] M6 — Analytics ingestion
- [ ] M7 — Reports on DuckDB
- [ ] M8 — Replication and recovery
- [ ] M9 — Multiple cells
- [ ] M10 — Cold tenants
- [ ] M11 — Archive and historical exports

## Completed through M3

- 19 planned tasks completed.
- PostgreSQL control plane with tenant registry, global staff identities, memberships,
  subscriptions, licences, and signed entitlement caching.
- Tenant-aware staff, device, and player authentication.
- Ownership leases with renewal, self-fencing, reassignment protection, and explicit handoff.
- Tenant routing cache and HTTP, SSE, and WebSocket proxying.
- Per-tenant SQLite baseline with strict storage conventions and transactional outbox.
- Lease-gated `TenantDb` handles with one writer, bounded readers, WAL mode, retries, and idle
  reaping.
- Resumable, concurrency-limited tenant migration orchestration.
- Resumable tenant provisioning with canonical units, access roles, and setting overrides.
- Exact scale-4 money and fixed-width UTC timestamp helpers with property tests.
- Transactional tenant outbox writer with atomic commit/rollback coverage.

## Current task

- [ ] `DB-0010b` — Add the M4 tenant schema for catalog and pricing, devices, sessions and
  wallets, transactions, ledger and credit, players and staff projection, plus checkout-critical
  inventory and recipe tables.

## M4 queue

- [ ] `DB-0010b` — Core venue SQLite schema.
- [ ] `API-0024` — Catalog and pricing repositories.
- [ ] `API-0026` — Devices, sessions, and wallets.
- [ ] `API-0027` — Transactions, ledger, credit, kiosk orders, and checkout stock deduction.
- [ ] `API-0028` — Players and staff membership projection.
- [ ] `API-0032` — In-process realtime dispatch.

## Accepted architecture decisions

- `ADR-0043` — Storage Cells with per-tenant SQLite and DuckDB.
- `ADR-0044` — Tenant SQLite baseline and outbox schema.
- `ADR-0045` — Tenant provisioning bootstrap schema and defaults.

## Verification

- Full backend test suite passes.
- Working tree was clean at this checkpoint.
- Latest completed implementation commit: `72ba6f3` (`API-0025`).

## Update rule

After each task:

1. Update the source build plan checkbox and completion note.
2. Update this file's date, current task, milestone checklist, and verification status.
3. Record the implementation commit.
