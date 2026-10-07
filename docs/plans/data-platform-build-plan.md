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
- Archive and historical export are needed only once a tenant's data is older than 18 months.

## Working agreement

- **One milestone at a time.** A milestone starts only when the previous one meets its done check.
- **Branching.** M1–M5 change the persistence layer underneath every API, so they're built on a long-lived `platform-v2` branch. `main` stays on the current stack, and `platform-v2` merges after M5, when every business API runs on SQLite. From M6 onward, work merges to `main` per milestone.
- **Analytics gap.** Report endpoints return `503 ANALYTICS_UNAVAILABLE` from the M5 merge until M7 completes. That's acceptable before launch.
- **End of every milestone:**
  1. the done check passes;
  2. tests are green, including `cargo test` and Biome for any TypeScript touched;
  3. a short demo or test run shows the outcome;
  4. task checkboxes are updated here;
  5. work is committed with Conventional Commits (`type(scope): summary`).

  Then stop for review before the next milestone.
- **Decisions.** A milestone that lists "decisions needed" doesn't start until the owner has signed them off in ADR-0043.
- **Observability.** Metrics and alerts for each component (data-platform §60–61) are part of the milestone that builds the component.

## Milestone overview

| # | Milestone | Outcome | Depends on | Estimate |
|---|---|---|---|---|
| M0 | Baseline and groundwork | Golden report fixtures, new crates, module layout, local stack | — | 1–2 weeks |
| M1 | Control plane | Tenants, users, auth, and entitlements on the global PostgreSQL | M0 | 2 weeks |
| M2 | Ownership and routing | Leases with self-fencing and handoff; router for HTTP, SSE, and WebSocket | M1 | 1–2 weeks |
| M3 | Tenant storage foundation | Per-tenant SQLite handle, migrations, provisioning, outbox table | M2 | 2 weeks |
| M4 | Core venue operations on SQLite | Sessions, wallets, plan purchases, POS checkout, credit, realtime | M3 | 3 weeks |
| M5 | Back office on SQLite (operational cutover) | Every business API on SQLite; demo seed ported; `platform-v2` merged | M4 | 4 weeks |
| M6 | Analytics ingestion | Outbox → JetStream → per-tenant DuckDB, with rebuilds | M5 | 2 weeks |
| M7 | Reports on DuckDB | Every report matches the golden fixtures; ClickHouse removed | M6 | 2 weeks |
| M8 | Replication and recovery (launch gate) | WAL replication to Wasabi, snapshots, restore, cell-loss drill | M5 | 3–4 weeks |
| | **MVP: single cell, ready for paying venues (M0–M8)** | | | **~20–23 weeks** |
| M9 | Multiple cells | Tenant moves, rebalancing, staged migrations, capacity benchmarks | M8 | 3–4 weeks |
| M10 | Cold tenants | Inactive tenants live only in Wasabi until their next request | M8 | 1–2 weeks |
| M11 | Archive and historical exports | 18-month archive, batched purge, `hot/` Parquet, export jobs | M7, M8 | 5–6 weeks |
| | **Full baseline (M0–M11)** | | | **~29–35 weeks** |

M8 depends only on M5, so it can run before M6 and M7 if launch timing calls for it. M11 must be complete before the oldest paying tenant's data reaches 18 months.

---

## M0: Baseline and groundwork

**Goal:** lock today's report behaviour as test fixtures, and prepare the codebase and local stack for the new architecture.

- [x] OPS-0001: Record golden outputs of every current report (`/stats/*`, `/stats/business`, finance report, expense, credit, inventory, receipt, and waste summaries). Use the demo dataset seeded with `pnpm demo:seed --date 2026-10-02` and save the JSON fixtures under `apps/backend/tests/fixtures/reports/`. — S
  - Done: 31 fixtures (month, week, and day windows) captured with `pnpm report:fixtures` from the committed code, with Redis caching disabled. The script captures twice and fails on any difference. `generatedAt` is stripped as volatile. `/inventory/overview` `recentMovements` is ordered only by `createdAt`, so ties come back in any order; fixtures sort ties by `id`. The SQLite port should add `id` as a tie-breaker to every `ORDER BY "createdAt"` list.
  - Re-capture: migrate and seed an empty database, run `analytics_worker --backfill`, run the backend with `LEGACY_REST_ENABLED=true` and `REDIS_URL=` (empty), then `pnpm report:fixtures` (or `--check`).
- [x] API-0001: Add crates per ADR-0043: `sqlx` `sqlite` feature, `duckdb` (bundled), `object_store` (S3-compatible, for Wasabi). Measure the clean and incremental build-time impact. — S
  - Done: `duckdb` 1.10506 (bundled), `object_store` 0.14 (`aws`), `sqlx` `sqlite`. The first build after adding them took about 57 minutes on a heavily loaded development machine, almost all of it compiling DuckDB's bundled C++. Incremental builds are unchanged (15 s vs 22 s baseline). A later `cargo check` started another cold C++ build because Cargo uses a separate profile, so DuckDB is opt-in as `duckdb-analytics` until M6. Release, CI, and Docker builds with that feature need a cached Cargo target directory; if linking remains slow, link a prebuilt `libduckdb` locally instead of `bundled`.
- [x] API-0002: Module layout inside `apps/backend/src`, keeping the handler → service → repository layering:
  - `control/`: control-plane repositories and services;
  - `tenancy/`: tenant context, routing cache, lease client;
  - `storage/`: tenant SQLite handles, migration runner, outbox;
  - `analytics/`: DuckDB, replacing the ClickHouse client.

  Role flags for `control`, `cell`, and `router`, all enabled in one process by default. — M
  - Done: `ARENA_ROLES` (default `control,cell,router`) parsed into `config::Roles` and logged at startup; invalid lists stop startup. Module skeletons for `control`, `tenancy`, and `storage`.
- [x] OPS-0002: Local stack in Compose: PostgreSQL (control plane), NATS JetStream, and an S3-compatible local stand-in for Wasabi. ClickHouse stays until M7. — S
  - Done: `object-storage` (SeaweedFS 4.48, S3 on `127.0.0.1:8333`) plus `object-storage-init`, which creates the `arena360` bucket idempotently. MinIO no longer publishes community images, so SeaweedFS (Apache-2.0) replaced it. S3 auth is disabled locally; signed requests are exercised against Wasabi in M8.
- [x] TEST-0001: Benchmark harness skeleton that replays a synthetic 20-PC venue day (session starts and ends, checkouts, POS) against one tenant. Reused in M9. — S
  - Done: `pnpm bench:venue-day [--pcs 20] [--sessions 8] [--sales 6] [--json out.json]` drives the public HTTP API, so it works unchanged against the SQLite cells. It creates its own staff, shift, plan, product, stock, PCs, and players, then runs one concurrent lane per PC and reports p50, p95, p99, and maximum latency per operation. First run against the current backend: 800 operations, 0 failures, 24 s. The latencies (debug build, loaded machine, 20 lanes sharing one staff shift) are not a capacity baseline; M9 measures on a release build and production-like hardware.

**Done when:** fixtures for every report are committed; the backend builds with the new crates and the build-time impact is recorded; `docker compose up` starts the new local stack; the benchmark harness runs against the current backend.

---

## M1: Control plane

**Goal:** the global PostgreSQL holds tenants, users, and entitlements, and users sign in through it.

- [x] DB-0001: Control-plane schema (`migrations/control/`):
  - `cells`: id, address, state, capacity weights;
  - `tenants`: id, owner_cell, storage_engine, generation, schema_version, state, timezone (IANA, required; ADR-0043 decision 28);
  - `tenant_leases`: tenant_id, owner_cell, ownership_generation, expires_at;
  - `users`, `organization_memberships`, auth challenges;
  - `locations`;
  - minimal `subscriptions` and `licenses`;
  - `replication_generations` and `snapshot_manifests`. — M
  - Done: self-contained 12-table PostgreSQL baseline with tenant/cell state checks, lease expiry and generation constraints, global staff identities, memberships, location metadata, subscriptions, licences, and replication lineage. Applied and invariant-tested against an empty PostgreSQL database.
- [x] API-0010: Move auth and organization management to the control role, so JWTs carry `tenant_id`. Device tokens (ADR-0017/0018 behaviour) carry `tenant_id` and `location_id`. — M
  - Done: `CONTROL_DATABASE_URL` initializes and migrates a dedicated control-plane pool; staff, admin, panel MFA, live membership validation, and internal tenant provisioning use the control schema while player and business operations remain on the transitional operational pool. Staff JWTs carry role-matched tenant memberships and permissions; device/player tokens carry tenant and location context.
- [x] API-0013: Signed tenant entitlement (license) cached in the cell, so business operations never query the control plane (§51). The same cache carries the tenant time zone. — S
  - Done: control repository provisions the initial subscription and licence; HMAC-signed entitlements carry tenant ID, IANA timezone, revision, limits, validity, and grace period. The cell cache rejects tampering and revision rollback. Its integration test closes PostgreSQL after caching and continues successfully.
- [x] TEST-0011: A UTC-only check that fails if any API payload, outbox event, or stored timestamp is written without a `Z` offset or as a local time. — S
  - Done: one canonical formatter emits RFC 3339 timestamps with `Z`; outbox publication recursively rejects local or non-`Z` timestamp strings; contract tests cover API envelopes, outbox payloads, canonical formatter usage, and prohibit timezone-naive timestamp types in PostgreSQL migrations.

**Done when:** a tenant can be created with a time zone; staff and devices sign in and receive tokens carrying `tenant_id`; the cell validates entitlement from its signed cache with the control plane stopped.

M1 verified: control-plane integration tests cover tenant time zones, signed entitlement cache operation after PostgreSQL closes, staff tenant claims, live membership invalidation, and panel MFA storage; device/player claim tests cover tenant and location context.

---

## M2: Ownership and routing

**Goal:** exactly one cell can write a tenant, and every request reaches that cell.

- [x] API-0011: Lease client in the cell. It acquires and renews leases per ADR-0043 decision 30 (5-minute lease, renewal every 60 seconds, self-fencing 30 seconds before the measured expiry, reassignment only after expiry plus a 30-second skew margin) and refuses to open SQLite writable without a valid generation. — M
  - Done: the control repository serializes acquisition against renewal, advances the ownership generation on takeover, and enforces the reassignment skew margin. The cell client measures safety from request send time, renews all held leases concurrently, self-fences 30 seconds early, immediately fences definitive renewal rejection, and exposes the generation guard required before a future `TenantDb` can open writable.
- [x] API-0014: Explicit lease handoff for planned moves and decommissioning. The old owner stops writing, uploads its remaining spooled WAL, and releases the lease; the new owner acquires it in the same transaction as the routing change, without waiting for expiry. — S
  - Done: handoff first installs an in-memory fence that cannot be undone by an in-flight renewal, then runs the WAL flush/upload hook. The control transaction locks the tenant and lease, validates the source generation and active target, and atomically switches routing ownership, lease ownership, and the incremented generation without waiting for expiry. Failures leave the old client fenced and never switch ownership.
- [x] API-0012: Routing cache (in-memory, refreshed from the control plane, invalidated on ownership change), plus a router that proxies HTTP, SSE, and WebSocket traffic to the owner cell. In single-process mode it dispatches in-process. — M
  - Done: authenticated tenant requests resolve through a PostgreSQL-backed in-memory routing cache carrying owner, generation, schema version, and time zone. Transactional ownership notifications refresh entries immediately, with periodic full refresh for recovery. Local owners dispatch in-process; dedicated routers and non-owner cells stream HTTP/SSE request and response bodies or bridge WebSocket frames to the owner. A routed-request marker prevents stale-cache proxy loops.
- [x] TEST-0010: Two cell processes attempt to own one tenant. Only the current generation writes. A cell cut off from PostgreSQL stops writing before its lease expires. Reassignment never happens before expiry plus the skew margin. Lease timings are configurable so tests run in seconds. — M
  - Done: independent cell clients race on one tenant and exactly one generation becomes writable. A separate owner pool is then closed to simulate control-plane cutoff: writes continue inside the measured safety window, self-fence before database expiry, and replacement remains blocked until expiry plus skew before acquiring the next generation.

**Done when:** TEST-0010 passes; requests for a tenant reach its owner cell over HTTP, SSE, and WebSocket; stopping the control plane for less than the fencing window doesn't interrupt writes.

M2 verified: timed multi-cell fencing passes with second-scale configuration; HTTP/SSE streaming and WebSocket frame bridging are exercised against live local servers; routing decisions and ownership-change notifications are covered by control-plane integration tests.

---

## M3: Tenant storage foundation

**Goal:** tenants get their own SQLite database with migrations, provisioning, and an outbox, ready for the API port.

- [x] DB-0010a: Tenant SQLite baseline scaffolding and conventions (UUID v7 text, fixed-width UTC text timestamps, scale-4 integer money, JSON text checks, no `organizationId`), plus the `outbox_events` table (envelope in the DuckDB schema doc, `sequence INTEGER PRIMARY KEY AUTOINCREMENT`). — S
  - Done: accepted ADR-0044 fixes the tenant storage representation. Migration `tenant/0001_foundation.sql` creates a strict, physically tenant-isolated transactional outbox with canonical UUID text, fixed-width UTC timestamps, validated object JSON, boolean/schema constraints, and non-reused commit-order sequences. Migration tests reject malformed representations and tenant discriminator columns.
- [x] API-0020: `TenantDb` handle: one writer connection plus a small read pool per open tenant, WAL mode, bounded `busy_timeout`, foreground retry with jitter and background back-off (§55). Idle tenants are closed. Opening requires a valid lease (M2). — M
  - Done: lease-gated handles serialize each tenant's writer, bound the read pool and SQLite lock wait, re-check fencing after writer-lock acquisition, replace stale generations, and reap idle or lease-invalid handles. Foreground busy retries and capped background back-off record contention count and wait duration; integration tests cover lease denial, concurrent handle reuse, WAL writes, generation fencing, idle reaping, and transient lock recovery.
- [x] API-0021: Migration runner and orchestrator. Applies per tenant, records the version in the control plane, is resumable, and limits concurrency per cell (§52–53). It exposes pre- and post-migration hooks, which M8 uses for snapshots. — M
  - Done: the cell orchestrator discovers pending control-plane tenants, opens only lease-valid writers, applies embedded SQLx migrations with bounded concurrency, and fences the control-plane version update by cell and ownership generation. Connection-level pre/post hooks support M8 snapshots. A 50-tenant integration test injects both pre-migration and control-plane-recording failures, then verifies resumable completion without repeating the migration or pre-hook.
- [x] API-0022: Tenant provisioning: create the files, migrate, seed defaults (units, settings, roles), register the tenant, and acquire the lease. — S
  - Done: accepted ADR-0045 defines the bootstrap schema and retry semantics. Provisioning now registers or resumes a `PROVISIONING` control-plane tenant, safely creates and migrates its SQLite file, idempotently seeds eleven canonical units, seven access roles, and validated setting overrides, acquires a non-routing lease, then atomically records the schema version and activates routing. The cell state owns the database manager and provisioner; retries preserve customizations and unmanaged existing files are rejected.
- [x] API-0023: Money helpers (Decimal ↔ scale-4 integer) and timestamp helpers, with property tests. — S
  - Done: exact Decimal ↔ scale-4 integer helpers reject excess precision and overflow, while fixed-width SQLite timestamp helpers format and strictly parse microsecond UTC text. Property tests cover every generated `i64` money round trip and randomized instants across the supported four-digit-year range; provisioning now uses the canonical storage formatter.
- [x] API-0025: Outbox writer helper used inside repository transactions. — S
  - Done: tenant repositories can write UUID-v7 event snapshots through their existing SQLite transaction. The helper validates event names, schema version, object payloads, UTC timestamps, and forbidden credential fields before inserting the canonical envelope. Integration coverage proves a sample business row and its outbox event commit or roll back together.

**Done when:** provisioning creates a working tenant database; the orchestrator migrates 50 test tenants and resumes correctly after an injected failure; money helpers pass property tests; an outbox row is written in the same transaction as a sample business write.

---

## M4: Core venue operations on SQLite

**Goal:** a venue can run a normal day (play sessions, plan purchases, POS checkout, credit) on per-tenant SQLite, with live updates.

Every port replaces `FOR UPDATE` with `BEGIN IMMEDIATE` transactions, moves PL/pgSQL trigger logic into services, and writes outbox events in the same transaction.

- [x] DB-0010b: Schema for catalog and pricing, devices, sessions and wallets, transactions, ledger and credit, players and staff projection, plus the inventory stock and recipe tables that checkout deducts from. — M
  - Done: accepted ADR-0046 defines the core venue contract. Migration `tenant/0003_core_venue.sql` adds strict tenant-local catalog, pricing, location, device, player/staff projection, wallet, session, transaction, credit, kiosk, recipe, and checkout-stock tables with canonical representations, normalized location scope, immutable sale snapshots, restrictive foreign keys, and access-path indexes. Contract tests exercise schema isolation, credentials, constraints, relationships, uniqueness, payment reconciliation, and representative checkout and credit flows.
- [x] API-0024: Port catalog and pricing (`product_repo`, `plan_repo`, `pricing_policy_repo`, `unit_repo`, `product_recipe_repo`, `game_repo`, `settings_repo` price overrides). — L
  - Done: tenant SQLite repositories now serve products, plans, units, games, recipes, pricing policies, and `pricing.*` setting overrides through lease-gated request paths. Writes use fenced `BEGIN IMMEDIATE` transactions with atomic outbox events; location projection and authorization preserve tenant scope and hidden prices; scheduled pricing activation, hybrid inventory stock reads for the API-0024 transition, and combined configuration snapshots preserve current behavior. Integration tests cover catalog CRUD and filters, money, recipes, pricing lifecycle, settings revisions, outbox atomicity, location isolation, and lease fencing.
- [x] API-0026: Port devices, sessions, and wallets (`device_repo`, `session_repo`, `player_plan_repo`, `balance_repo`), including dynamic plan deduction. — L
  - Done: lease-fenced tenant SQLite repositories and service paths now cover device lifecycle and fingerprints, player plans, wallets, sessions, session-usage ledger entries, dynamic deduction snapshots, and staged realtime compatibility. Session start, heartbeat, and end atomically update device, wallet, ledger, session, and outbox state; retries are idempotent, session timezones are frozen, and in-use devices cannot be deleted. Live handlers remain on PostgreSQL until API-0027, API-0028, and API-0030 supply transaction, identity, and shift foreign-key dependencies; M5 performs the coordinated operational cutover.
- [x] API-0027: Port transactions, ledger, and credit (`transaction_repo`, `transaction_product_repo`, `ledger_repo`, `credit_repo`, `kiosk_order_repo`), including the stock deduction performed at checkout. — L
  - Done: lease-fenced tenant SQLite repositories and staged service paths now cover transactions, immutable line and option snapshots, plan-wallet ledger grants, credit limits and settlements, kiosk orders, and checkout stock deduction. Product checkout resolves server prices, recipes, and option ingredients inside one fenced writer transaction; exact payment reconciliation, stock movements, kiosk fulfillment, credit settlement, wallet grants, and outbox events commit or roll back together. Integration tests cover concurrent no-oversell, recipe and option deductions, immutable kiosk pricing, credit limits and settlement history, payment transitions, location guards, idempotent plan grants, lease fencing, and rollback behavior. PostgreSQL handlers remain active until the remaining M4/M5 dependencies are ported for coordinated cutover.
- [x] API-0028: Players and the staff membership projection used by tenant APIs (`user_repo` tenant parts). — M
  - Done: lease-fenced tenant SQLite repositories and staged service paths now own player registration, profile and credential updates, soft deletion, lookup, and kiosk authentication while projecting global staff memberships without credentials. Revisioned staff projections enforce non-template role subsets, exact location scope, atomic access revocation, idempotent tombstones, and secret-free outbox events; active-player checks now protect tenant plan assignment. Integration tests cover tenant isolation, player lifecycle and username reuse, authentication, projection ordering and replay, role escalation prevention, location access, lease rollback, and PostgreSQL-free staged execution. Live user, auth, and access handlers remain on PostgreSQL until the coordinated M5 cutover.
- [x] API-0032: Realtime: replace `PgListener` (`realtime/dispatcher.rs`) with in-process `tokio::broadcast` in the owning cell. — S
  - Done: the owning cell now uses a bounded in-process wake hub for committed PostgreSQL and tenant SQLite events; the business dispatcher no longer depends on `PgListener`, while routing notifications remain on PostgreSQL. PostgreSQL producers wake only after commit, tenant writers batch exact outbox sequences after fenced commit, and runtime lag recovery deduplicates pending work. Tenant projections preserve session, balance, device, kiosk, configuration, pricing, and staff-sale payload contracts while enforcing source-tenant ACLs and durable-delivery-before-send semantics. Tests cover commit and rollback notification boundaries, schema-zero migration bootstrap, projection allowlists and payload snapshots, tenant isolation, lag deduplication, and existing realtime ACLs. Process restart retains the prior non-replayed `LISTEN`/`NOTIFY` behavior; durable cross-process publishing remains API-0040.

**Done when:** staged SQLite integration tests cover kiosk login, session start, dynamic deduction, session end, plan purchase, POS checkout with stock deduction, credit sale, and credit settlement; the tenant outbox projects session and device events with the admin floor's realtime payload contract. Record the measured porting pace and re-estimate the remaining plan. The live admin floor demo against SQLite belongs to M5's operational cutover, because M4 intentionally leaves the HTTP handlers on PostgreSQL.

M4 verified (2026-10-07): `cargo test` passes, with database and external-service tests marked ignored by their existing harnesses. The staged SQLite repository suites cover kiosk player login, session lifecycle and weighted deduction, plan grants, POS stock deduction, credit sale and settlement. The realtime dispatcher tests prove that committed SQLite session events project to the staff channel. The M4 implementation commits for catalog through realtime span about five hours on 2026-10-07 and change 53 backend source/test files; that short commit window is not a reliable estimate of end-to-end cutover effort. M5 is provisionally increased from three to four weeks because its remaining domain ports also require coordinated HTTP and admin/kiosk cutover. Later milestone estimates are unchanged until M5 supplies a live benchmark.

---

## M5: Back office on SQLite (operational cutover)

**Goal:** every business API runs on SQLite, and PostgreSQL holds only the control plane.

- [x] DB-0010c: Schema for inventory and procurement, shifts, cash and expenses, settings, notifications, and kitchen. — M
  - Done: `tenant/0004_back_office.sql` adds strict tenant-local procurement, stock workflow, cash, expense, configuration, notification, and kitchen tables around the M4 shift and stock roots, plus a transactional purchase-order number counter. UUID text, UTC timestamps, scale-4 money, JSON, status, and relationship constraints follow the accepted tenant conventions. `tenant_back_office_schema` tests check table isolation, foreign keys, receipt accounting, duplicate invoices, and invalid references.
- [x] API-0029: Port inventory and procurement (`inventory_repo`, `vendor_repo`, procurement services). — L
  - Done: lease-fenced tenant SQLite paths cover suppliers, inventory locations, stock receipts, adjustments, transfer approvals and fulfillment, waste approvals, purchase-order lifecycle and version checks, reorder rules and suggestions, and movement lookup. Purchase-order receipts atomically commit stock movements, receipt snapshots, approved expenses, optional venue-matched cash entries, and outbox events. Scale-4 arithmetic, immutable unit snapshots, duplicate invoice protection, tenant isolation, concurrent fulfillment, rollback, cash scope, and lease fencing pass five integration tests; the full backend suite passes. Live HTTP handlers remain on PostgreSQL until the coordinated M5 cutover. Implementation: `828a90c`.
- [x] API-0030: Port shifts, cash, and expenses (`shift_repo`, `cash_register_repo`, `cash_deposit_repo`, `expense_repo`, `expense_category_repo`). — M
  - Done: fenced SQLite repositories and service paths cover atomic shift start/close/handover, cash ledger and carry-forward, deposit decisions, expense approval, and category CRUD. Cash sales and credit settlements write register entries in the same transaction. Exact scale-4 arithmetic, public closure states, rollback, concurrent decisions, and tenant isolation are verified by four finance and eight commerce integration tests; full `cargo test` passes (external-service tests retain their existing ignore gates). Commit: `e098ef7`. Live handler wiring remains in API-0033.
- [x] API-0031: Port settings, configuration, notifications, and kitchen (`settings_repo`, `config_repo`, `notification_repo`, `kitchen_service`). Replace the PostgreSQL advisory locks in `settings_repo`, `handlers/kitchen.rs`, and `handlers/access.rs` with `BEGIN IMMEDIATE` or control-plane locks, depending on which database each one guards. — M
  - Done: tenant settings, configuration, activity/inbox, kitchen, and access-edit paths use fenced `BEGIN IMMEDIATE` writers in place of PostgreSQL advisory locks. Revision checks and history are atomic; delete/recreate settings keep monotonically increasing revisions, and tenant-wide changes mirror the legacy configuration view atomically. Kiosk notifications and kitchen preparation commit with checkout snapshots; access edits preserve the last manager and exact location scopes. Migration 0006 preserves existing notification rows, adds kiosk activity kinds, access audit/module versions, and complete location metadata. Three back-office service tests, a migration preservation test, kitchen checkout tests, and catalog/identity/lease regressions pass. Full `cargo test` passes; final configuration consistency changes pass focused tests. Commit: `379a8cc`. Legacy handler branches are removed in API-0033.
- [ ] API-0033: Remove the shared-table compatibility code: tenant predicates, default organization IDs, and the non-default-organization block in `access/routes.rs`. — S
  - In progress: operational HTTP handlers require tenant SQLite; staff credentials/MFA stay global while current grants and secret-free profiles are projected locally. Authentication decodes before routing and authorizes on the owner; refresh continues from local grants during a control-plane outage. Staff/admin login shift actions execute on the owner. Session detail/end checks use the stored venue, and session creation validates an active matching shift atomically. Rooms and membership now commit in tenant SQLite with outbox events. Realtime projection, durable delivery/ACK and replay now persist in tenant SQLite; current account, room and venue grants are rechecked, authorization failures retry, and revoked deliveries cannot starve later permitted replay. Session activity now commits with session mutations, and kiosk inbox creation, reads and realtime replay check current venue grants. Staff allowance renewal now commits wallet, ledger and outbox atomically; staff kiosk credentials are verified globally and play tokens carry player capabilities with local shift/device checks. Shift, cash-register, deposit and expense lists and actions now use actual venue grants; expense venues persist independently of mutable inventory locations, cash approval checks the register venue, and stale expense actions fail inside the writer. Inventory and procurement now filter current venue grants before pagination and check actual locations inside mutations, including both ends of transfers and configured defaults. Transaction reads now restrict players to their own history and staff to current venue grants; credit settlement reads use the collecting shift venue before pagination. Eleven commerce tests pass. Kiosk lists/actions use the immutable session venue and status writes check an active matching shift atomically; kitchen lists and ticket updates use stored sale venues. Eleven commerce and five back-office tests pass. Remaining: remaining venue boundaries, activity preservation in other workflows, startup and shared-service compatibility removal. The full backend suite, owning-cell WebSocket tests and isolated control/SQLite authentication and recovery test pass; this task remains unchecked until the cutover is complete.
- [ ] OPS-0010: Port `pnpm demo:seed` to provision a tenant and write through the services, so outbox events are produced. — M
- [ ] TEST-0020: All backend integration tests run against temporary tenant SQLite files and a test control-plane database. — M
- [ ] OPS-0011: Retire the PostgreSQL operational schema and its 119 migrations, then merge `platform-v2` into `main`. — S

**Done when:** the admin and kiosk apps work end to end against the new backend on the ported demo seed; the admin floor view updates live from SQLite-backed sessions and devices; no `PgPool` is used outside the control plane; `platform-v2` is merged. Reports return `503 ANALYTICS_UNAVAILABLE` until M7.

---

## M6: Analytics ingestion

**Goal:** each tenant's DuckDB stays current from the outbox and can be rebuilt at any time.

- [ ] API-0040: Outbox publisher per cell. It publishes pending events per tenant every 250 ms–1 s or when a batch fills, deletes rows after acknowledgement, and exposes backlog metrics (§16–17). — M
  - Deletion must also wait until `realtime_projection_cursor.sequence` covers the source row, so analytics publishing cannot remove canonical events before realtime projection.
- [ ] OPS-0020: JetStream stream `ARENA_TENANT_EVENTS` on subjects `arena.tenant.*.events.v1`, with limits retention (7 days) for rebuild replay. — XS
- [ ] DB-0020: DuckDB schema v1 from `duckdb-analytics-schema.md`, including `_ingest_state` and its migration runner. — S
- [ ] API-0041: Analytics consumer. Buffers per tenant, flushes at 500–1,000 events or 1 s, collapses each batch to the last state per key, upserts or deletes, rebuilds `session_hours` for changed sessions, advances `last_sequence`, then acknowledges. Gap detection per ADR-0043 decision 20. — L
- [ ] API-0042: Rebuild: `VACUUM INTO` snapshot, T0, `ATTACH` the snapshot read-only, hot-window `INSERT … SELECT`, `session_hours`, `monthly_summary`, replay events after T0, then `READY`. Covers §32 cases A, B, and C. — L
- [ ] API-0044: Nightly retention: batched deletion of facts older than the hot window, and refresh of `monthly_summary`. — S
- [ ] API-0045: Time-zone change handling. Updating `tenants.timezone` marks the tenant's analytics `REBUILDING` and triggers a rebuild. — S
- [ ] API-0046: Per-cell background job scheduler enforcing the priorities in §54, used by the publisher, consumer, rebuild, and retention jobs. — M

**Done when:** a rebuild during concurrent writes loses no events after T0; a sequence gap stops ingestion; stopping NATS lets the outbox grow and it drains on recovery; duplicate deliveries don't change the data; DuckDB row counts and totals match SQLite on the demo seed.

---

## M7: Reports on DuckDB

**Goal:** every report reads from DuckDB and matches today's output, and ClickHouse is gone.

- [ ] API-0043: Rewrite the report queries for DuckDB:
  - `analytics/business.rs`;
  - `analytics/reports.rs`;
  - `services/stats_service.rs`;
  - the finance report.

  Replace `ReportScope` with location predicates and return `report temporarily rebuilding` when the tenant isn't `READY` (§36). Replace the hard-coded IST offset (`FixedOffset::east_opt(19_800)`) with the tenant's time zone. — L
- [ ] TEST-0030: Report parity against the M0 golden fixtures, plus the test cases listed in the schema doc. — M
- [ ] OPS-0021: Remove ClickHouse entirely: client, `schema.sql`, `schema.json`, the old worker binary, the Compose service, environment variables, tests, and the `docs/architecture/analytics.md` content. — S

**Done when:** every report matches its golden fixture; the admin analytics pages work on the demo seed; nothing in the repository references ClickHouse.

---

## M8: Replication and recovery (launch gate)

**Goal:** a cell can die and its venues come back with at most about 2 minutes of lost writes.

**Decisions needed first:** ADR-0043 open decisions 4 (backup encryption) and 5 (cell availability target).

- [ ] API-0053: WAL Replication Worker, built in, per ADR-0043 decision 26:
  - disable automatic checkpoints;
  - copy new WAL frames to the local NVMe spool, then checkpoint;
  - batch spooled frames into segments, compressed with zstd, encrypted and checksummed;
  - upload every 2 minutes, or earlier at the size threshold, to `replication/generations/{generation_id}/wal/{nnnnnnnnnn}.wal.zst` with immutable, never-reused keys;
  - verify each upload, then record it in the generation manifest, then delete it from the spool.

  Expose spool size, oldest unshipped frame age, and upload failures as metrics and alerts. — L
- [ ] API-0054: Replication generations. Start a new generation on a lease change, a restore, or a detected WAL gap, recording `ownership_generation` in the manifest and the current generation in the control plane. — M
- [ ] API-0050: Snapshots:
  - take a consistent snapshot daily, plus immediately before and after every tenant migration (through the API-0021 hooks);
  - compress with zstd, encrypt and compute a checksum;
  - upload to `replication/generations/{generation_id}/snapshots/{utc-timestamp}[-pre|post-migration-vNN].db.zst`;
  - verify, then record in the manifest (§46). — M
- [ ] API-0055: Retention job. Delete WAL segments and snapshots older than 90 days, but never the oldest snapshot still needed by WAL segments inside the window. — S
- [ ] API-0051: Restore path. Use the control plane's current generation, then its latest verified snapshot, then every WAL segment after it, in order, verifying checksums. Point-in-time restore stops replay at a chosen UTC instant. Run SQLite `integrity_check`, then open (§47). Refuse without a lease (§10). — M
- [ ] API-0052: Cell-loss recovery. List the affected tenants, reassign them, acquire leases, restore, bring operations online, then rebuild analytics (§49–50). — M
- [ ] API-0056: Disk pressure zones (§57) covering SQLite, WAL, spool, DuckDB, and temporary files, with automatic pausing of background work. — S
- [ ] TEST-0051: Kill the cell process mid-traffic and confirm nothing is lost, because the spool survives. Delete the cell's disk, restore on another cell, and confirm only writes since the last upload (≤ about 2 minutes) are lost. Restore to a point in time 30 days back. — M
- [ ] OPS-0030: Automated weekly restore drill per cell, plus a documented runbook. Publish the measured recovery time. — S

**Done when:** TEST-0051 passes; a full cell-loss drill on staging restores every tenant within the published recovery time; replication alerts fire in a simulated Wasabi outage. **This is the gate for onboarding paying venues.**

---

## M9: Multiple cells

**Goal:** tenants can be moved between cells, and cell capacity is measured, not guessed.

- [ ] API-0060: Tenant move state machine (`PREPARING_MOVE` → `COPYING` → `CUTOVER` → `VERIFYING` → `ACTIVE`), with a short write gate, explicit lease handoff (API-0014), an atomic ownership and routing switch, and a temporary copy kept on the old cell (§13). — L
- [ ] API-0061: Rebalancing command, driven by weighted cell capacity. — M
- [ ] API-0062: Staged migration rollout (canary, 1%, 10%, 25%, 100%) with automatic halt on failure (§52). — M
- [ ] TEST-0050: Cell capacity benchmark using the M0 harness, recording weighted capacity per hardware profile. — M

**Done when:** a tenant moves between two cells under live traffic with only the write-gate pause; a cell can be drained and decommissioned; capacity numbers are recorded for the production hardware profile.

---

## M10: Cold tenants

**Goal:** inactive tenants cost nothing on cells and come back on their next request.

- [ ] API-0070: `COLD` state. Snapshot, release the lease, and remove the local files after verification. On the first request, assign a cell, acquire the lease, download, verify, hydrate, and set `ACTIVE` (§11). — M

**Done when:** a tenant goes cold and back with no data loss, and the first-request hydration time is measured and recorded.

---

## M11: Archive and historical exports

**Goal:** data older than 18 months moves to Wasabi Parquet safely, and any period can be exported.

Must complete before the oldest paying tenant's data reaches 18 months.

- [ ] DB-0070: Archive, backfill, and export manifests and state machines in the control plane (§26, §30, §41). — S
- [ ] API-0074: `hot/` Parquet writer. A JetStream consumer writes monthly Parquet partitions to `tenants/{id}/hot/{yyyy}/{mm}/`, with a backfill from SQLite for months written before it existed. It is non-authoritative and rebuildable (ADR-0043 decision 31). — M
- [ ] API-0071: Archive worker: month-sized chunks exported from SQLite to Parquet in `archive/`, uploaded, verified, then purged in small batches with back-off driven by OLTP p99. No automatic `VACUUM` (§24–28). — L
- [ ] API-0071a: Archive hand-off. After an `archive/` month is verified against SQLite, delete the matching `hot/` month. — XS
- [ ] API-0072: Historical export jobs: an isolated worker pool that reads Wasabi Parquet (`hot/` and `archive/`) with DuckDB and writes CSV, CSV.gz, or Parquet (optionally XLSX) to `exports/`, served through a signed download, with concurrency limits per cell, tenant, and platform (§38–42). — L
- [ ] API-0073: SQLite backfill from archive: staging, validation, idempotent bounded batches, checkpoints (§29–31). — M

**Done when:** archiving a test tenant's oldest month purges SQLite only after verification; operational write p99 stays within target during purge; an export spanning archived and hot months matches the source totals.

---

## Risk register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Repository porting runs long (461 queries) | High | High | Port by domain behind tests; re-estimate at the end of M4 |
| `platform-v2` drifts from `main` during M1–M5 | Medium | Medium | Freeze feature work on `main` during the port, or port any change made there immediately |
| Report regressions after the DuckDB rewrite | Medium | High | Golden fixtures (M0) and parity tests (M7) before ClickHouse removal |
| Split-brain ownership bugs | Low | Critical | Lease self-fencing, generation in manifests, TEST-0010, conflict alerts |
| Payment loss on cell failure | Low | High | WAL replication every 2 minutes bounds loss to about 2 minutes (M8), TEST-0051, restore drills |
| Wasabi outage fills the spool | Low | Medium | Spool disk alerts; pause background work; writes continue until the disk zones in §57 are reached |
| DuckDB build time slows development | Medium | Low | Measure in M0; isolate DuckDB behind a feature flag for fast local builds if needed |
| Archive purges delete unverified data | Low | Critical | Purge only from `VERIFIED` manifests (Invariant 4); verification tests |
