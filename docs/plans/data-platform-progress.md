# Data Platform Progress

**Updated:** 2026-10-08
**Branch:** `codex/m6-analytics-ingestion`
**Source plan:** `docs/plans/data-platform-build-plan.md`  
**Architecture source:** `docs/architecture/data-platform.md`

## Current position

- [x] M0 — Baseline and groundwork
- [x] M1 — Control plane
- [x] M2 — Ownership and routing
- [x] M3 — Tenant storage foundation
- [x] M4 — Core venue operations on SQLite
- [x] M5 — Back office on SQLite
- [ ] M6 — Analytics ingestion
- [ ] M7 — Reports on DuckDB
- [ ] M8 — Replication and recovery
- [ ] M9 — Multiple cells
- [ ] M10 — Cold tenants
- [ ] M11 — Archive and historical exports

## Completed through API-0041

- M0–M5 are complete; operational API cutover and local merge are verified.
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
- In-process owning-cell realtime dispatch with commit-only tenant SQLite wakes,
  runtime lag deduplication, source-tenant ACL enforcement, immutable event snapshots, and
  durable-delivery-before-send ordering.

- Strict tenant back-office schema and fenced inventory/procurement paths, with exact purchase-order snapshots and atomic receipts, expenses, cash entries, and outbox events.

- Tenant SQLite shifts, cash registers, deposits, expense categories, and expense approvals, with atomic handover and financial source entries.

- Tenant settings/configuration, notification inbox, kitchen preparation, and access editing with atomic revisions, audit history, and checkout snapshots.

## Current task

- [ ] `API-0042` — Consistent snapshot rebuild, derived summaries and replay after T0.

### API-0033 completed

- Operational startup, handlers, services, access checks, repositories, rooms, and realtime use tenant SQLite. The application has no operational PostgreSQL pool.
- Shared repositories/services, default organization/venue IDs, organization predicates, and legacy realtime wake paths are removed.
- Staff credentials/MFA remain global; current secret-free identities, local grants, and venue boundaries are checked on the owning cell.
- Atomic business, activity, inbox, and canonical outbox writes are verified. Reports remain `ANALYTICS_UNAVAILABLE` until M7.
- Full suite: 359 passing tests, 15 infrastructure gates. Seven isolated control/lease/bootstrap/staff tests pass separately.
- Implementation commits: `9286c5b`, `167c17f`, `4c3f880`.

### OPS-0010 completed

- `pnpm demo:seed` provisions a tenant on a registered cell and writes through fenced operational SQLite commands.
- Completion markers preserve successful repeat runs and reject interrupted/occupied targets. Existing operators keep their credentials; player login is explicitly configurable.
- Real binary seed/repeat test verifies 430 sales, 275 sessions (five active), two kiosk orders, stock/wallet/cash reconciliation, secret-free events, and unchanged operator credentials.
- Six Node tests and three provisioning/schema tests pass. The actual `pnpm` dry-run path is checked.
- M0 deterministic report generator is retained; operational v2 report parity remains an M7 gate.

### TEST-0020 completed

- `pnpm backend:test:integration` and backend CI run 360 normal tests plus all eight control-backed checks against temporary tenant files and a disposable control database.
- Local server discovery/startup, explicit admin server targeting, cleanup on interruption and missing Cargo, and removal of all temporary roots are verified.
- Obsolete operational PostgreSQL trigger tests are retired; all nine live report routes are tested for 503 and current local access revocation for 403.
- Five optional external ClickHouse/Redis gates remain. JetStream and report parity suites are rebuilt in M6/M7.

### OPS-0011 completed

- Removed 118 tracked operational SQL migrations, operational PostgreSQL configuration/import tools and the legacy worker. Control and tenant migration families remain active.
- Deployment uses explicit control migrations and durable tenant storage; owning-cell chart guards and production render/lint pass.
- Backend 368, admin 124 and kiosk 123 checks pass; both client typechecks pass. Actual kiosk HTTP flow passes against the native API and seeded SQLite tenant. The authenticated admin browser floor updates 5 → 6 → 5 without refresh. Native Tauri window behavior was not exercised.
- `platform-v2` is merged locally into `main`, based on existing `master`. Remote default and deployment remain unchanged. Reports remain unavailable until M7.

### API-0040 completed

- Ordered per-cell publication, stable tenant/event message IDs, explicit stream checks, persistent ACK cursors and realtime-safe fenced deletion. Network waits do not hold the SQLite writer.
- Pending tenants survive handle eviction; empty polling permits eviction and no publisher acquires ownership. Backlog and publication metrics are available.
- 373 backend checks and a separate real JetStream check pass (374 total). Missing-stream retention, replay deduplication, partial failure/recovery, fencing, gaps, exact JSON numbers, tombstones and writer availability are verified.
- Rebuild boundary documentation uses the committed AUTOINCREMENT watermark after source cleanup.

### OPS-0020 completed

- Idempotent native stream setup, seven-day file/limits replay retention, explicit NATS deployment configuration and read-only checks. Existing incompatible streams/messages remain unchanged.
- Three configuration tests and the actual native setup/server test pass. Consumer ACKs retain history. The actual pnpm check command and production Helm/YAML checks pass.
- The runner/CI exercise both gates against fresh disposable servers. Success, missing Cargo and interruption cleanup are verified. The normal/control suite (373), setup unit checks (3) and two live gates total 378 passing checks across the recorded runs.
- API-0040 implementation: `497c3e2`.

### DB-0020 completed

- Tenant-local DuckDB schema and transactional checksum migrations preserve exact decimals, generated values and existing checkpoints. All storage operations are lease fenced; fresh state is REBUILDING.
- Six schema/rollback/fencing/timezone tests pass, and the complete SQLite/DuckDB/control/JetStream run passes 384 checks. Native SDK versions/checksums are pinned in CI, and loader paths are assigned in Cargo test runners.
- OPS-0020 implementation: `a46c943`. DB-0020 implementation: `da6379c`. Next: `API-0041` analytics consumer.

### API-0041 completed

- Exact canonical snapshots commit with operational writes; per-tenant consumers collapse batches, atomically update facts, session hours and checkpoints, then ACK. Persistent gaps request rebuilds. Lease fencing and idle eviction remain effective.
- All 394 regular/control/live checks pass, including replay after commit-before-ACK, parent collections, stock history, exact decimals, timezone hour splits and invalid events.
- Next: `API-0042` consistent rebuild and production activation.

## M5 queue

- [x] `DB-0010c` — Back-office SQLite schema (`0a350c0`).
- [x] `API-0029` — Inventory and procurement (`828a90c`).
- [x] `API-0030` — Shifts, cash, and expenses (`e098ef7`).
- [x] `API-0031` — Settings, notifications, access locks, and kitchen (`379a8cc`).
- [x] `API-0033` — Operational cutover and shared-table compatibility removal.
- [x] `OPS-0010` — Tenant service demo seed.
- [x] `TEST-0020` — SQLite/control-plane integration harnesses.
- [x] `OPS-0011` — Retire operational PostgreSQL and merge (`7ae41d7`).

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
- Latest completed item: `API-0041`; 394 backend checks pass. OPS-0020 commit: `a46c943`; integration harness: `2492b35`.
- Five inventory/procurement SQLite integration tests pass, including atomic receipt financial links, duplicate invoice rollback, concurrent fulfillment, and lease fencing.

- Four finance and eight commerce integration tests pass, covering concurrent start/approval, handover rollback, deposit reversal after closure, and atomic sale/settlement cash entries.

- Settings/configuration, notification retention, access edit, migration preservation, and kitchen checkout tests pass; full backend suite and final focused configuration regressions pass.

## Update rule

After each task:

1. Update the source build plan checkbox and completion note.
2. Update this file's date, current task, milestone checklist, and verification status.
3. Record the implementation commit.
