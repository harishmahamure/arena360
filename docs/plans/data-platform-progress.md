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
- [x] M4 — Core venue operations on SQLite
- [ ] M5 — Back office on SQLite
- [ ] M6 — Analytics ingestion
- [ ] M7 — Reports on DuckDB
- [ ] M8 — Replication and recovery
- [ ] M9 — Multiple cells
- [ ] M10 — Cold tenants
- [ ] M11 — Archive and historical exports

## Completed through API-0031

- 29 planned tasks completed.
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
- Strict tenant-local core venue schema for catalog, pricing, devices, identities, wallets,
  sessions, transactions, credit, kiosk checkout, recipes, and checkout stock.
- Lease-fenced tenant SQLite catalog and pricing repositories with transactional outbox events,
  location-scoped authorization, scheduled policy activation, and tenant-aware configuration
  snapshots.
- Tenant SQLite device, player-plan, wallet, and session repositories with atomic dynamic
  deduction, ledger/outbox writes, idempotent retries, and staged realtime compatibility.
- Tenant SQLite transaction, credit, kiosk-order, and checkout repositories with atomic stock
  deduction, immutable sale snapshots, exact payment reconciliation, idempotent plan grants,
  historical credit settlements, and transactional outbox events.
- Tenant SQLite player lifecycle and kiosk authentication, plus a credential-free, revisioned
  global staff membership projection with exact role and location scope, atomic revocation,
  idempotent tombstones, and secret-free outbox events.
- In-process owning-cell realtime dispatch with commit-only PostgreSQL and tenant SQLite wakes,
  runtime lag deduplication, source-tenant ACL enforcement, immutable event snapshots, and
  durable-delivery-before-send ordering.

- Strict tenant back-office schema and fenced inventory/procurement paths, with exact purchase-order snapshots and atomic receipts, expenses, cash entries, and outbox events.

- Tenant SQLite shifts, cash registers, deposits, expense categories, and expense approvals, with atomic handover and financial source entries.

- Tenant settings/configuration, notification inbox, kitchen preparation, and access editing with atomic revisions, audit history, and checkout snapshots.

## Current task

- [ ] `API-0033` — Wire the operational cutover and remove shared-table compatibility.

## M5 queue

- [x] `DB-0010c` — Back-office SQLite schema (`0a350c0`).
- [x] `API-0029` — Inventory and procurement (`828a90c`).
- [x] `API-0030` — Shifts, cash, and expenses (`e098ef7`).
- [x] `API-0031` — Settings, notifications, access locks, and kitchen (`379a8cc`).
- [ ] `API-0033` — Operational cutover and shared-table compatibility removal.
- [ ] `OPS-0010` — Tenant service demo seed.
- [ ] `TEST-0020` — SQLite/control-plane integration harnesses.
- [ ] `OPS-0011` — Retire operational PostgreSQL and merge.

## M4 queue

- [x] `DB-0010b` — Core venue SQLite schema.
- [x] `API-0024` — Catalog and pricing repositories.
- [x] `API-0026` — Devices, sessions, and wallets.
- [x] `API-0027` — Transactions, ledger, credit, kiosk orders, and checkout stock deduction.
- [x] `API-0028` — Players and staff membership projection.
- [x] `API-0032` — In-process realtime dispatch.

## Accepted architecture decisions

- `ADR-0043` — Storage Cells with per-tenant SQLite and DuckDB.
- `ADR-0044` — Tenant SQLite baseline and outbox schema.
- `ADR-0045` — Tenant provisioning bootstrap schema and defaults.
- `ADR-0046` — Core venue SQLite schema.

## Verification

- Full backend test suite passes.
- Tenant venue, catalog, commerce, identity, realtime, schema, provisioning, migration, and lease
  regression tests pass.
- Latest completed implementation commit: `379a8cc` (`API-0031`).
- Five inventory/procurement SQLite integration tests pass, including atomic receipt financial links, duplicate invoice rollback, concurrent fulfillment, and lease fencing.

- Four finance and eight commerce integration tests pass, covering concurrent start/approval, handover rollback, deposit reversal after closure, and atomic sale/settlement cash entries.

- Settings/configuration, notification retention, access edit, migration preservation, and kitchen checkout tests pass; full backend suite and final focused configuration regressions pass.

## Update rule

After each task:

1. Update the source build plan checkbox and completion note.
2. Update this file's date, current task, milestone checklist, and verification status.
3. Record the implementation commit.
