# Build Plan: Arena360 Data Platform 2.0

Implements `docs/architecture/data-platform.md` under `docs/adr/0043-storage-cells-sqlite-duckdb.md`. The analytical schema is `docs/architecture/duckdb-analytics-schema.md`.

## Goal

Replace the shared PostgreSQL operational database and the ClickHouse pipeline with:

- a PostgreSQL control plane,
- Storage Cells holding one SQLite and one DuckDB database per tenant,
- an outbox → JetStream → DuckDB analytics path,
- Wasabi for WAL replication, snapshots, hot Parquet copies, archives, and exports (layout: ADR-0043 decision 31).

The public HTTP API (OpenAPI) stays stable, so the admin and kiosk apps keep working.

## Assumptions

- No production customer data exists; the demo seed is the only dataset to carry over.
- One developer, full time.
- Analytics endpoints may return `503 ANALYTICS_UNAVAILABLE` between Phase 2 and Phase 3, which is acceptable before launch.
- Archive and historical export are needed only once a tenant's data is older than 18 months.

## Milestones

| Phase | Outcome | Estimate |
|---|---|---|
| 0. Groundwork | Golden report fixtures, crates, module layout, local stack | 1–2 weeks |
| 1. Control plane and routing | Tenants, leases, auth, router on PostgreSQL | 2–3 weeks |
| 2. Tenant SQLite | Every business API on per-tenant SQLite | 6–8 weeks |
| 3. Analytics | Outbox → JetStream → DuckDB; ClickHouse removed | 4–5 weeks |
| 4. Backup and recovery | WAL Replication Worker to Wasabi (2-minute batches), daily and migration snapshots, 90-day retention, restore, cell-loss drill | 3–4 weeks |
| **MVP (single cell, production-ready)** | | **~16–22 weeks** |
| 5. Multiple cells | Tenant moves, rebalancing, staged migrations, capacity benchmarks | 3–4 weeks |
| 6. Cold tenants | Hydration from Wasabi on first request | 1–2 weeks |
| 7. Archive and exports | Parquet archive, batched purge, historical exports, SQLite backfill | 5–6 weeks |
| **Full baseline** | | **~26–34 weeks** |

Observability (data-platform §60–61) is built alongside each phase, not as a separate phase.

## Phase 0: Groundwork

- [ ] OPS-0001: Record golden outputs of every current report (`/stats/*`, `/stats/business`, finance report, expense, credit, inventory, receipt, and waste summaries). Use the demo dataset seeded with `pnpm demo:seed --date 2026-10-02` and save the JSON fixtures under `apps/backend/tests/fixtures/reports/`. These are the parity baseline after ClickHouse is gone. — S
- [ ] API-0001: Add crates per ADR-0043: `sqlx` `sqlite` feature, `duckdb` (bundled), `object_store` (S3-compatible, for Wasabi). Measure the clean and incremental build-time impact. — S
- [ ] API-0002: Module layout inside `apps/backend/src`, keeping the handler → service → repository layering:
  - `control/`: control-plane repositories and services;
  - `tenancy/`: tenant context, routing cache, lease client;
  - `storage/`: tenant SQLite handles, migration runner, outbox;
  - `analytics/`: DuckDB, replacing the ClickHouse client.

  Role flags for `control`, `cell`, and `router`, all enabled in one process by default. — M
- [ ] OPS-0002: Local stack in Compose: PostgreSQL (control plane), NATS JetStream, and MinIO as a local stand-in for Wasabi. Keep ClickHouse until Phase 3 finishes. — S
- [ ] TEST-0001: Benchmark harness skeleton that replays a synthetic 20-PC venue day (session starts and ends, checkouts, POS) against one tenant. Reused for the cell capacity work in Phase 5. — S

## Phase 1: Control plane and routing

- [ ] DB-0001: Control-plane schema (`migrations/control/`):
  - `cells`: id, address, state, capacity weights;
  - `tenants`: id, owner_cell, storage_engine, generation, schema_version, state, timezone (IANA, required; ADR-0043 decision 28);
  - `tenant_leases`: tenant_id, owner_cell, ownership_generation, expires_at;
  - `users`, `organization_memberships`, auth challenges;
  - `locations`;
  - minimal `subscriptions` and `licenses`;
  - `backup_manifests`.

  Lease semantics follow ADR-0043 decision 30: a 5-minute lease, renewal every 60 seconds, self-fencing 30 seconds before the measured expiry, and reassignment only after expiry plus a 30-second skew margin. — M
- [ ] API-0010: Move auth and organization management to the control role, so JWTs carry `tenant_id`. Device tokens (ADR-0017/0018 behaviour) carry `tenant_id` and `location_id`. — M
- [ ] API-0011: Lease client in the cell. It acquires and renews leases, self-fences (closes writers) on expiry, and refuses to open SQLite writable without a valid generation. — M
- [ ] API-0012: Routing cache (in-memory, refreshed from the control plane, invalidated on ownership change), plus a router that proxies HTTP, SSE, and WebSocket traffic to the owner cell. In single-process mode it dispatches in-process. — M
- [ ] API-0013: Signed tenant entitlement (license) cached in the cell, so business operations never query the control plane (§51). The same cache carries the tenant time zone. — S
- [ ] TEST-0011: A UTC-only check that fails if any API payload, outbox event, or stored timestamp is written without a `Z` offset or as a local time. — S
- [ ] API-0014: Explicit lease handoff for planned moves and decommissioning. The old owner stops writing, uploads its remaining spooled WAL, and releases the lease; the new owner acquires it in the same transaction as the routing change, without waiting for expiry. — S
- [ ] TEST-0010: Two cell processes attempt to own one tenant. Only the current generation writes. A cell cut off from PostgreSQL stops writing before its lease expires. Reassignment never happens before expiry plus the skew margin. Lease timings are configurable so tests run in seconds. — M

## Phase 2: Tenant SQLite

- [ ] DB-0010: Tenant SQLite baseline (`migrations/tenant/`), applying the ADR-0043 conventions (UUID v7 text, fixed-width UTC text timestamps, scale-4 integer money, JSON text checks), with no `organizationId`. One task per domain:
  - catalog and pricing;
  - devices, sessions, and wallets;
  - transactions, ledger, and credit;
  - inventory and procurement;
  - shifts, cash, and expenses;
  - settings, notifications, and kitchen;
  - players and staff projection;
  - `outbox_events`. — L
- [ ] API-0020: `TenantDb` handle: one writer connection plus a small read pool per open tenant, WAL mode, bounded `busy_timeout`, foreground retry with jitter and background back-off (§55). Idle tenants are closed. — M
- [ ] API-0021: Migration runner and orchestrator. Applies per tenant, records the version in the control plane, is resumable, and limits concurrency per cell (§52–53). — M
- [ ] API-0022: Tenant provisioning: create the files, migrate, seed defaults (units, settings, roles), register the tenant, and acquire the lease. — S
- [ ] API-0023: Money helpers (Decimal ↔ scale-4 integer) and timestamp helpers, with property tests. — S
- [ ] API-0024 to API-0031: Port repositories and services one domain at a time, in the same order as DB-0010. Replace `FOR UPDATE` with `BEGIN IMMEDIATE` transactions and move PL/pgSQL trigger logic into services. Each domain writes its outbox events (envelope in the DuckDB schema doc) in the same transaction. — L each, 8 tasks
- [ ] API-0032: Realtime: replace `PgListener` (`realtime/dispatcher.rs`) with in-process `tokio::broadcast` in the owning cell. — S
- [ ] API-0033: Remove the shared-table compatibility code: tenant predicates, default organization IDs, and the non-default-organization block in `access/routes.rs`. — S
- [ ] OPS-0010: Port `pnpm demo:seed` to provision a tenant and write through the services, so outbox events are produced. — M
- [ ] TEST-0020: All backend integration tests run against temporary tenant SQLite files and a test control-plane database. — M

## Phase 3: Analytics

- [ ] API-0040: Outbox publisher per cell. It publishes pending events per tenant every 250 ms–1 s or when a batch fills, deletes rows after acknowledgement, and exposes backlog metrics (§16–17). — M
- [ ] OPS-0020: JetStream stream `ARENA_TENANT_EVENTS` on subjects `arena.tenant.*.events.v1`, with limits retention (7 days) for rebuild replay. — XS
- [ ] DB-0020: DuckDB schema v1 from `duckdb-analytics-schema.md`, including `_ingest_state` and its migration runner. — S
- [ ] API-0041: Analytics consumer. Buffers per tenant, flushes at 500–1,000 events or 1 s, collapses each batch to the last state per key, upserts or deletes, rebuilds `session_hours` for changed sessions, advances `last_sequence`, then acknowledges. Gap detection per ADR-0043 decision 20. — L
- [ ] API-0042: Rebuild: `VACUUM INTO` snapshot, T0, `ATTACH` the snapshot read-only, hot-window `INSERT … SELECT`, `session_hours`, `monthly_summary`, replay events after T0, then `READY`. Covers §32 cases A, B, and C. — L
- [ ] API-0043: Rewrite the report queries for DuckDB:
  - `analytics/business.rs`;
  - `analytics/reports.rs`;
  - `services/stats_service.rs`;
  - the finance report.

  Replace `ReportScope` with location predicates and return `report temporarily rebuilding` when the tenant isn't `READY` (§36). Replace the hard-coded IST offset (`FixedOffset::east_opt(19_800)` in `analytics/business.rs`) with the tenant's time zone for calendar windows and buckets. — L
- [ ] API-0045: Time-zone change handling. Updating `tenants.timezone` in the control plane marks the tenant's analytics `REBUILDING` and triggers a rebuild, because the derived calendar labels depend on it. — S
- [ ] API-0044: Nightly retention: batched deletion of facts older than the hot window, and refresh of `monthly_summary`. — S
- [ ] TEST-0030: Report parity against the Phase 0 golden fixtures, plus the test cases listed in the schema doc. — M
- [ ] OPS-0021: Remove ClickHouse entirely: client, `schema.sql`, `schema.json`, the old worker binary, the Compose service, environment variables, tests, and the `docs/architecture/analytics.md` content. — S

## Phase 4: Backup and recovery

Open decisions 4 (encryption) and 5 (cell availability target) in ADR-0043 need sign-off first. WAL replication and the Wasabi layout are decided (ADR-0043 decisions 26 and 31).

- [ ] API-0053: WAL Replication Worker, built in, per ADR-0043 decision 26:
  - disable automatic checkpoints;
  - copy new WAL frames to the local NVMe spool, then checkpoint;
  - batch spooled frames into segments, compressed with zstd, encrypted and checksummed;
  - upload every 2 minutes, or earlier at the size threshold, to `replication/generations/{generation_id}/wal/{nnnnnnnnnn}.wal.zst` with immutable, never-reused keys;
  - verify each upload, then record it in the generation manifest, then delete it from the spool.

  Expose spool size, oldest unshipped frame age, and upload failures as metrics and alerts. — L
- [ ] API-0054: Replication generations. Start a new generation on a lease change, a restore, or a detected WAL gap, recording `ownership_generation` in the manifest and the current generation in the control plane. — M
- [ ] API-0050: Snapshots:
  - take a consistent snapshot daily, plus immediately before and after every tenant migration (hooked into the API-0021 orchestrator);
  - compress with zstd, encrypt and compute a checksum;
  - upload to `replication/generations/{generation_id}/snapshots/{utc-timestamp}[-pre|post-migration-vNN].db.zst`;
  - verify, then record in the manifest (§46). — M
- [ ] API-0055: Retention job. Delete WAL segments and snapshots older than 90 days, but never the oldest snapshot still needed by WAL segments inside the window. — S
- [ ] API-0051: Restore path. Use the control plane's current generation, then its latest verified snapshot, then every WAL segment after it, in order, verifying checksums. Point-in-time restore stops replay at a chosen UTC instant. Run SQLite `integrity_check`, then open (§47). Refuse without a lease (§10). — M
- [ ] TEST-0051: Kill the cell process mid-traffic and confirm nothing is lost, because the spool survives. Delete the cell's disk, restore on another cell, and confirm only writes since the last upload (≤ about 2 minutes) are lost. Restore to a point in time 30 days back. — M
- [ ] API-0052: Cell-loss recovery. List the affected tenants, reassign them, acquire leases, restore, bring operations online, then rebuild analytics (§49–50). — M
- [ ] OPS-0030: Automated weekly restore drill per cell, plus a documented runbook. Publish the measured recovery time. — S

## Phase 5: Multiple cells

- [ ] API-0060: Tenant move state machine (`PREPARING_MOVE` → `COPYING` → `CUTOVER` → `VERIFYING` → `ACTIVE`), with a short write gate, an atomic ownership and routing switch, and a temporary copy kept on the old cell (§13). — L
- [ ] API-0061: Rebalancing command, driven by weighted cell capacity. — M
- [ ] API-0062: Staged migration rollout (canary, 1%, 10%, 25%, 100%) with automatic halt on failure (§52). — M
- [ ] TEST-0050: Cell capacity benchmark using the Phase 0 harness, recording weighted capacity per hardware profile. — M

## Phase 6: Cold tenants

- [ ] API-0070: `COLD` state: snapshot, release the lease, and remove the local files after verification. On the first request, assign a cell, acquire the lease, download, verify, hydrate, and set `ACTIVE` (§11). — M

## Phase 7: Archive and exports

Schedule this to finish before the oldest paying tenant's data reaches 18 months.

- [ ] DB-0070: Archive, backfill, and export manifests and state machines in the control plane (§26, §30, §41). — S
- [ ] API-0071: Archive worker: month-sized chunks exported to Parquet, uploaded, verified, then purged in small batches with back-off driven by OLTP p99. No automatic `VACUUM` (§24–28). — L
- [ ] API-0074: `hot/` Parquet writer. A JetStream consumer writes monthly Parquet partitions to `tenants/{id}/hot/{yyyy}/{mm}/`, with a backfill from SQLite for months written before it existed. It is non-authoritative and rebuildable (ADR-0043 decision 31). — M
- [ ] API-0071a: Archive hand-off. After an `archive/` month is verified against SQLite, delete the matching `hot/` month. — XS
- [ ] API-0072: Historical export jobs: an isolated worker pool that reads Wasabi Parquet (`hot/` and `archive/`) with DuckDB and writes CSV, CSV.gz, or Parquet (optionally XLSX) to `exports/`, served through a signed download, with concurrency limits per cell, tenant, and platform (§38–42). — L
- [ ] API-0073: SQLite backfill from archive: staging, validation, idempotent bounded batches, checkpoints (§29–31). — M

## Cross-cutting

- Metrics and alerts from §60–61, added in the phase that introduces each component.
- Background job priorities P0–P10 (§54) enforced through a per-cell job scheduler, introduced in Phase 3 and extended as workers are added.
- Disk pressure zones and automatic pausing of background work (§57), introduced in Phase 4.

## Risk register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Repository porting runs long (461 queries) | High | High | Port by domain behind tests; measure pace after the first two domains and re-plan |
| Report regressions after the DuckDB rewrite | Medium | High | Golden fixtures (OPS-0001) and parity tests before ClickHouse removal |
| Split-brain ownership bugs | Low | Critical | Lease self-fencing, generation in manifests, TEST-0010, conflict alerts |
| Payment loss on cell failure | Low | High | WAL replication every 2 minutes bounds loss to about 2 minutes (API-0053), TEST-0051, restore drills |
| Wasabi outage fills the spool | Low | Medium | Spool disk alerts; pause background work; writes continue until the disk zones in §57 are reached |
| DuckDB build time slows development | Medium | Low | Measure in API-0001; isolate DuckDB behind a feature flag for fast local builds if needed |
| Archive purges delete unverified data | Low | Critical | Purge only from `VERIFIED` manifests (Invariant 4); verification tests |
