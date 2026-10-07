# ADR-0043: Storage Cells with per-tenant SQLite and DuckDB

**Status**: Accepted (2026-10-06, owner)
**Date**: 2026-10-06
**Deciders**: Founder / backend owner
**Supersedes**: ADR-0009 (PostgreSQL and SQLx-on-PostgreSQL portion; Axum, utoipa, and the handler → service → repository layering remain)
**Replaces**: shared-table tenancy (`docs/architecture/tenancy.md`), the ClickHouse reporting pipeline (`docs/architecture/analytics.md`), and the rejected `DRAFT-0042`

## Context

Arena360 is sold per PC to many independent venue businesses. Physical tenant isolation, cheap commodity servers, and tenant portability are product requirements. The owner adopted `docs/architecture/data-platform.md` (Version 2.0) as the production architecture baseline and the source of truth. Where earlier ADRs or architecture documents conflict with it, that document wins.

The current backend runs one shared PostgreSQL database (119 migrations, 76 tables, 461 runtime SQLx queries across 26 repositories) with tenancy by `organizationId` column, and reports from ClickHouse fed by a PostgreSQL outbox and NATS JetStream.

## Decision

Adopt `docs/architecture/data-platform.md` in full. The implementation decisions below make it buildable in this repository.

### Process and deployment shape

1. Three service roles built from `apps/backend`:
   - **control**: auth, tenant management, subscriptions, licensing, location metadata, placement, ownership leases. Uses the global PostgreSQL.
   - **cell**: every tenant business API, the realtime/SSE streams, report endpoints, and the cell workers (outbox publisher, analytics consumer, backup, migration, archive, export, hydration).
   - **router**: authenticates the token, resolves tenant → owner cell from a cached routing table, and proxies HTTP, SSE, and WebSocket traffic.
2. The MVP runs all three roles in one process on one machine. The router path is still exercised so the shape does not change when a second cell is added.
3. The public HTTP contract (utoipa OpenAPI, ADR-0004) stays stable. The admin and kiosk apps should not need changes beyond new export endpoints.

### Tenancy model

4. **Tenant = organization.** Venue locations live inside the tenant SQLite. Staff identities are global (control plane); each tenant SQLite keeps the membership and profile projection it needs. Players are tenant-local customers.
5. Tenant SQLite schemas drop `organizationId`. Location scoping stays (`venueLocationId` / `locationId`).

### Storage engines and crates

6. **Control plane**: PostgreSQL through `sqlx` (`postgres` feature), migrations in `apps/backend/migrations/control/`.
7. **Tenant OLTP**: SQLite through `sqlx` (`sqlite` feature), migrations in `apps/backend/migrations/tenant/`. Each open tenant has one writer connection and a small read pool, in WAL mode with a bounded `busy_timeout`.
8. **Tenant analytics**: DuckDB through the `duckdb` crate (bundled build). Schema is defined in `docs/architecture/duckdb-analytics-schema.md`.
9. **Object storage**: Wasabi (owner decision 2026-10-06, replacing Cloudflare R2) through an S3-compatible client (`object_store`). Every "R2" in `data-platform.md` reads as Wasabi. Layout and retention: decision 31.
10. **Messaging**: NATS JetStream through `async-nats` (already a dependency).
11. **Removed**: ClickHouse (client, schema, worker, Compose service, environment variables, tests) and the PostgreSQL operational schema with its 119 migrations.

### SQLite conventions

12. IDs are UUID v7 generated in Rust and stored as `TEXT`.
13. Timestamps are `TEXT` in fixed-width RFC 3339 UTC (`YYYY-MM-DDTHH:MM:SS.ffffffZ`), so lexical order equals time order. No local times are stored (decision 27).
14. Money and decimal quantities are `INTEGER` fixed-point at scale 4 (value × 10,000), preserving today's `decimal(19,4)` precision. Conversion to `rust_decimal::Decimal` happens only in repositories.
15. JSON is `TEXT` with `CHECK (json_valid(...))`.
16. Business rules currently in PL/pgSQL triggers move into Rust services. Plain SQLite triggers are allowed only for single-statement bookkeeping such as `updatedAt`.
17. Writes that lock rows today (`SELECT ... FOR UPDATE`) run as `BEGIN IMMEDIATE` transactions.

### Outbox and analytics

18. `outbox_events.sequence` is `INTEGER PRIMARY KEY AUTOINCREMENT`. SQLite has one writer, so sequence order equals commit order. The publisher never needs to handle out-of-order commits.
19. Events are published to subject `arena.tenant.<tenant_id>.events.v1`. The stream uses limits retention (time-based, initially 7 days) instead of work-queue retention, so rebuilds can replay events after the boundary T0. Outbox rows are deleted only after JetStream acknowledges publication.
20. DuckDB ingestion is idempotent by per-tenant `sequence`: events at or below `_ingest_state.last_sequence` are skipped. A gap (sequence > last + 1) stops ingestion for that tenant and triggers a re-fetch, then a rebuild if the gap persists.
21. Initial build and rebuild (data-platform §32–35):
    1. Take a consistent SQLite snapshot with `VACUUM INTO` a temporary file and read `MAX(sequence)` from it as T0.
    2. `ATTACH` the snapshot to DuckDB read-only and transform the last 18 months into the analytical schema.
    3. Replay JetStream events with sequence > T0, then mark the tenant `READY`.

    Direct scans of the live SQLite file are not used, because table-by-table reads would not be one consistent snapshot.
22. Report endpoints run in the cell process that owns the tenant DuckDB. Each tenant DuckDB is opened lazily and closed when idle. The analytics consumer holds the single writer connection; report queries use read connections in the same process.
23. Reports keep their current definitions, with calendar buckets in the tenant's time zone (decision 28). `ReportScope` SQL rewriting is replaced by plain location predicates, because the tenant boundary is the database itself.

### Realtime

24. PostgreSQL `LISTEN`/`NOTIFY` is replaced by in-process `tokio::broadcast` in the owning cell (the existing SSE pattern). Clients reconnect through the router when a tenant moves.

### Data migration

25. No customer data exists in production, so there is no PostgreSQL-to-SQLite data migration. The demo seed (`pnpm demo:seed`) is ported to the new schemas.

### Continuous WAL shipping (owner decision, 2026-10-06)

26. The MVP replicates every tenant's SQLite WAL to Wasabi continuously, on top of daily snapshots (data-platform §48). Owner decisions, 2026-10-06:
    - **Worker:** a WAL Replication Worker per cell, built into the backend rather than run as a Litestream sidecar, because the object layout and ownership fencing below differ from Litestream's.
    - **Local spool:** automatic checkpoints are disabled. The worker copies new WAL frames into a local NVMe spool, then checkpoints. Spool files stay on disk until their upload is verified (§57 disk zones apply to the spool).
    - **Upload cadence:** batched spool frames are uploaded **every 2 minutes**, or earlier when the spool reaches a size threshold. Tenants with no writes upload nothing.
    - **Recovery point:** losing a whole cell (disk or server) loses **up to about 2 minutes** of writes. A process crash loses nothing, because the spool survives on disk.
    - **Immutability:** WAL segments and snapshots are immutable and never overwritten. Each segment has a unique, monotonically numbered key within its generation, and its checksum and frame range are recorded in the generation manifest.
    - **Snapshots:** daily, plus an extra snapshot immediately before and after every tenant migration.
    - **Retention:** WAL segments and snapshots are both kept for **90 days**. Any point in the last 90 days can be restored, and expired objects are deleted by a retention job.
    - **Replication generations:** a new generation starts whenever the WAL lineage breaks: a new owner (lease change), a restore, or a detected gap. Each generation's manifest records the lease's `ownership_generation`. The control plane records which generation is current, and restores use only that one, never "the newest folder", so a fenced-out former owner can't contribute writes.
    - **Restore:** the latest verified snapshot in the current generation, plus every WAL segment after it, in order.
    - **Encryption and integrity:** snapshots and segments are encrypted with the per-tenant key (open decision 4) and checksummed.

### Time (owner decision, 2026-10-06)

27. **All timestamps are UTC everywhere:** PostgreSQL, SQLite, DuckDB, outbox events, JetStream, object-storage manifests, logs, and API payloads (RFC 3339 with `Z`). No local-time timestamp is stored anywhere.
28. **Each tenant's IANA time zone lives in the global PostgreSQL** (`tenants.timezone`). Cells cache it with the routing and entitlement data. It's used only to turn UTC instants into calendar labels: report day and hour buckets, and the start and end of a requested calendar date range. Display conversion is the frontend's job.
29. DuckDB's `local_date`, `local_hour`, and `weekday` columns are derived labels computed in Rust (`chrono-tz`) from UTC timestamps at ingest. They aren't times, and the underlying timestamp columns stay UTC. Changing a tenant's time zone triggers an analytics rebuild. The hard-coded IST offset in `analytics/business.rs` (`FixedOffset::east_opt(19_800)`) is replaced by the tenant's time zone.

### Ownership lease (owner decision, 2026-10-06)

30. **Lease length is 5 minutes.** This resolves data-platform §7 (one writer) against §51 (control-plane outage):
    - **Renewal:** the owning cell renews every 60 seconds.
    - **Self-fencing:** the cell measures the lease from when it *sent* the renewal request, using a monotonic clock. It stops writing the tenant, closing the SQLite writer and rejecting writes, 30 seconds before that measured expiry if no renewal has succeeded.
    - **Reassignment after a failure:** only after the lease has expired in PostgreSQL plus a 30-second clock-skew margin, so about 5½ minutes after the last successful renewal. The new owner gets a higher `ownership_generation`.
    - **PostgreSQL outages:** cells keep serving writes through control-plane outages of up to about 4½ minutes. Longer outages pause tenant writes until PostgreSQL returns. Reads may continue.
    - **Planned moves (§13) and decommissioning** don't wait for expiry. The old owner stops writing, uploads its remaining spooled WAL, and releases the lease explicitly, and the new owner acquires it in the same control-plane transaction as the routing change.
    - **Licensing:** the signed entitlement cache (§51) is separate and keeps its own longer grace period.
    - **Operational consequence:** venues on a cell that dies unexpectedly are unavailable for at least about 5½ minutes plus restore time. The global PostgreSQL should run with automatic failover.

### Object storage layout and retention (owner decision, 2026-10-06)

31. Bucket `wasabi://arena360/`, one prefix per tenant:

    ```text
    tenants/{tenant_id}/
      replication/
        generations/{generation_id}/
          manifest.json                      # ownership_generation, segment list, checksums
          snapshots/2026-10-06T00-00-00Z.db.zst          # daily
          snapshots/2026-10-06T14-12-09Z-pre-migration-v52.db.zst
          wal/0000000001.wal.zst
          wal/0000000002.wal.zst
      hot/{yyyy}/{mm}/…parquet               # derived copy of the last ~18 months
      archive/{yyyy}/{mm}/…parquet           # authoritative copy of purged data, with manifest and checksum
      exports/{export_id}/…                  # generated downloads
    ```

    | Data | Policy |
    |---|---|
    | Live SQLite | Local NVMe |
    | WAL spool | Local NVMe until uploaded and verified |
    | WAL upload | Every 2 minutes, or earlier at the size threshold |
    | WAL segments and snapshots | Immutable; retained 90 days |
    | Snapshots | Daily, plus before and after every migration |
    | Recent business data | ~18 months in local SQLite and DuckDB; Parquet copy in `hot/` |
    | Older data | `archive/` Parquet with manifest and checksum |
    | Old analytics | Export job: download Parquet → DuckDB → export file |
    | Disaster restore | Snapshot + WAL replay |

    - **`hot/` is derived and non-authoritative**, like DuckDB. A consumer writes monthly Parquet partitions from JetStream events. It can be rebuilt from SQLite, and it lets exports for any period read only from Wasabi without loading the tenant's live databases.
    - **`archive/` is produced from SQLite** (the source of truth), not copied from `hot/`. It's verified before any SQLite purge (Invariant 4). Once an archive month is verified, the matching `hot/` month is deleted.
    - **Billing.** Wasabi has no egress or request fees under its fair-use policy, but bills every object for at least 90 days and charges a 1 TB monthly minimum. The 90-day WAL and snapshot retention matches the minimum. Short-lived objects (exports, temporary recovery files) are still billed for 90 days.

## Open decisions (owner sign-off required before the phase that needs them)

| # | Decision | Proposed default | Needed by |
|---|---|---|---|
| 4 | Backup and archive encryption | Per-tenant data key; tenant deletion destroys the key | M8 (replication and recovery) |
| 5 | Cell availability target | Single-node cells; recovery time measured in restore drills and published as the target | M8 (replication and recovery) |

Decided: 1 (WAL replication to Wasabi every 2 minutes, decision 26), 2 (5-minute lease, decision 30), 3 (time zone and UTC, decisions 27–29), and the object storage provider and layout (decisions 9 and 31).

## Consequences

### Positive

- Physical tenant isolation; no shared-table predicate can leak data across businesses.
- Commodity servers; capacity grows by adding Storage Cells.
- Tenants can be moved, restored, and hydrated individually.
- Operational APIs never depend on analytics, NATS, or Wasabi availability.
- Analytics and history stay bounded (18-month hot window, archive in Wasabi Parquet).
- Any point in the last 90 days can be restored per tenant.

### Negative

- Ground-up rewrite of the persistence layer, the analytics pipeline, and deployment.
- The platform now owns ownership leases, backups, recovery, migrations across many databases, and archive correctness, all of which were previously managed by one database.
- No cross-tenant SQL; platform metrics need a dedicated consumer.
- The bundled DuckDB build adds significant compile time to the backend.

### Risks

| Risk | Mitigation |
|---|---|
| Split-brain writes to one tenant | Lease with expiry and self-fencing; ownership generation recorded in every backup manifest; conflict alerts |
| Payment loss on cell failure | WAL replication every 2 minutes bounds loss to about 2 minutes (decision 26); restore drills |
| Wasabi outage | Spool retains WAL locally; operational writes continue; alert on spool age and size |
| Money precision errors after the type change | Scale-4 integers; property tests; reconciliation of the ported demo dataset |
| Analytics drift between SQLite and DuckDB | Sequence-based idempotency, gap detection, scheduled sampled reconciliation against SQLite |
| Fan-out migration failures | Per-tenant resumable migrations, canary rollout (§52) |
| Scope too large for one developer | Phased plan with an MVP that runs a single cell; archive and export work deferred until data ages past 18 months |

## Alternatives Considered

- **Shared PostgreSQL with row-level security**: cheapest and no rewrite, but logical isolation only.
- **Turso database per tenant (`DRAFT-0042`, rejected)**: managed, but metered billing, vendor dependency, and no control over placement or recovery.
- **PostgreSQL database per tenant**: keeps the SQL, but connection pools and operations grow with tenant count and do not fit cheap commodity cells.
- **Keep ClickHouse for analytics**: proven, but it is a shared cross-tenant store that conflicts with physical isolation and adds a stateful service per environment.

## Implementation Notes

The build sequence is in `docs/plans/data-platform-build-plan.md`. The DuckDB schema and report mapping are in `docs/architecture/duckdb-analytics-schema.md`.

## References

- `docs/architecture/data-platform.md` (source of truth)
- `docs/architecture/duckdb-analytics-schema.md`
- `docs/plans/data-platform-build-plan.md`
- `docs/adr/DRAFT-0042-turso-database-per-tenant.md` (rejected)
- ADR-0009 text is not present in this repository; its scope is taken from the ADR index in `.cursor/rules/20-adr-discipline.mdc`.
