# Data Platform Progress

**Updated:** 2026-10-08
**Branch:** `codex/m8-replication-recovery`
**Source plan:** `docs/plans/data-platform-build-plan.md`  
**Architecture source:** `docs/architecture/data-platform.md`

## Current position

- [x] M0 — Baseline and groundwork
- [x] M1 — Control plane
- [x] M2 — Ownership and routing
- [x] M3 — Tenant storage foundation
- [x] M4 — Core venue operations on SQLite
- [x] M5 — Back office on SQLite
- [x] M6 — Analytics ingestion
- [x] M7 — Reports on DuckDB
- [ ] M8 — Replication and recovery
- [ ] M9 — Multiple cells
- [ ] M10 — Cold tenants
- [ ] M11 — Archive and historical exports

## Completed through M7

- M0–M7 are complete; operational cutover, analytics ingestion and local milestone merges are verified.
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

- [ ] `TEST-0050` — Capacity benchmark workflow and hardware measurements (M9); production profile, staging connection and workload/latency targets are pending.
- Capacity workflow is prepared and three harness checks pass. Actual measurements await environment details; local implementation continues with API-0070 while this external gate is pending.
- M8 staging launch gate remains pending: full-cell operational/analytics recovery within published RTO and actual alert firing during a Wasabi outage. No staging cell IDs, control-plane URL or backup credentials are configured locally.
- Owner approved ADR-0043 decisions 4 and 5 on 2026-10-08: per-tenant encryption keys destroyed on deletion; single-node cells with recovery time measured in restore drills and published. Implementation continues in plan order.

### API-0062 completed

- Durable canary/1%/10%/25%/100% cohorts have per-stage observation intervals, exact current ownership admission, advisory execution locks, automatic global failure halt and explicit resume.
- Replication-enabled cells run one P6 migration at a time through the existing verified pre/post snapshot hooks. No rollout enrollment means no automatic fleet migration on startup.
- All 18 selected rollout/runner/replication checks pass, including a real SQLite canary failure/resume and soak timing. Native analytics library/binary check passes. CLI and runbook: `docs/operations/schema-rollouts.md`.
- Implementation commit: `e2e2037`.

### API-0061 completed

- Measured multi-resource placement uses benchmark profiles and fresh complete tenant/cell overhead inputs. Incoming copies reserve target resources; retained source storage is never treated as immediately freed.
- The preview/apply/drain/decommission command locks budgets and ownership, recomputes placement before atomic enqueue, and rejects occupied-cell decommissioning. Three tests pass, including concurrent apply without overbooking, rollback and retention accounting; the move regression also passes. Production hardware measurements remain TEST-0050.
- Runbook: `docs/operations/cell-rebalancing.md`.
- Implementation commit: `009f33f`.

### API-0060 completed

- Planned moves pre-copy verified encrypted backups while source writes continue, catch up final WAL under a bounded asynchronous write gate, and atomically transfer ownership, lease generation, routing and phase.
- Source timeout/restart resumes only its recorded ownership; target quarantine and committed completion resume safely. Schema mismatch and stale readiness block cutover. Source copies remain seven days; cleanup checks ownership and concurrent/newer moves.
- Request/status/cancel CLI and `docs/operations/tenant-moves.md` describe execution and recovery. All 28 selected control/storage/replication/move checks pass; local successful write gate was 774 ms. Production hardware and representative traffic remain M9 launch checks.
- Implementation commit: `fa60985`. Native analytics library/binary build check passes. Subsequent rebalancer regression also passes the move fixture (1,023 ms local write gate).

### OPS-0030 completed

- Replication-enabled cells automatically restore all assigned ACTIVE tenants weekly into private staging using their existing leases. Durable reports retain selected tenant count, per-tenant position/bytes/errors and elapsed time; failed runs retry hourly and cell-level advisory locking prevents duplicates.
- P10 admission, per-tenant deadlines and cancellation cleanup bound resource use. Successful images are deleted without changing operational ownership, routing, generation or live facts.
- Status, elapsed time and completion UTC are published through Prometheus and reloaded from PostgreSQL after restart. Failure/overdue alert rules and `docs/operations/restore-drills.md` cover routine inspection, staging cell loss and Wasabi outage evidence.
- All 16 replication checks pass, including corrupt-object failure/retry, scheduling, locking, metric reload and unchanged live facts. Local one-tenant restore-only measurement: 109 ms. This excludes operational activation and analytics; full staging RTO remains unmeasured.

### TEST-0051 completed

- A child process drives actual TenantDb transactions and outbox writes through an HTTP S3 fixture, while the parent kills it with SIGKILL. All 6,500 acknowledged commits survived local SQLite recovery; the spool remained on disk.
- Deleting the source disk and recovering on a second cell restored verified uploads with a measured 14.6-second recovery-point lag. An explicitly dated encrypted backup chain restores to 30 days ago; it is fixture evidence, not aged staging data.
- The crash test exposed corruption from VACUUM INTO renumbering source pages. Snapshots now use SQLite's backup API to preserve physical pages, including freelist and reader-pinned WAL pages.
- The process test and all 15 replication checks pass. Implementation commit: `1e62893`. Staging full-cell recovery time and real Wasabi outage alert evidence remain pending.

### API-0056 completed

- Fifteen-second filesystem probes and bounded allocation scans cover SQLite/outbox storage, WAL/SHM, raw/encoded spool batches, DuckDB and snapshot/recovery/rebuild/archive/export temporary files. Symlinks are skipped and hard-linked inodes counted once.
- 70/80/90-percent warning/corrective/critical zones use two percentage points of recovery hysteresis. Corrective pressure pauses new backfills, exports, archive staging and maintenance, with daily snapshots deferred before tenant opening; critical/unknown pressure also pauses analytics ingestion. Operations, outbox, WAL transport, recovery and verified archive purges remain eligible.
- Prometheus gauges and seven replication/disk alert rules are checked in. Failed recovery removes consumed staging images and copy temporaries; paused analytics recovery remains bounded and retryable.
- Three disk checks, 23 focused scheduler/tenant/replication regressions, native library/binary compilation, integration-runner syntax and alert YAML parsing pass. Isolated fixtures explicitly disable the unrelated host occupancy probe and exercise forced zones themselves.
- API-0052 implementation commit: `8f159fc`.

### API-0052 completed

- Explicit `tenant_recover` listing/recovery command and startup/minute resume use expiry-plus-skew lease acquisition directly into RESTORING, excluded from routing. P2 recovery preserves normal lease renewal.
- Durable PostgreSQL phases and local quarantine markers resume verified installation, restore generation rotation, baseline verification, pre/post schema upgrades, current staff projection and final integrity checks. Ordinary opens stay blocked until activation and marker removal.
- Operations activate before native SQLite/retained-stream/shadow analytics rebuilding. Analytics failures leave operations online and retry the existing ownership generation/job.
- 26 focused checks pass: 15 replication/recovery, seven tenant database and four control-plane checks. A real local source-directory loss restores uploaded writes on another cell and excludes the unuploaded write; tests also cover former lease refusal, analytics outage/retry and an activation-to-marker-cleanup crash. Native all-target compilation and command help/read-only listing pass.
- Implementation follows API-0051 commit `cb70ddb`. Live staging full-cell RTO and storage outage alerts remain required launch-gate evidence.

### API-0051 completed

- Lease-gated restore selects only the control plane's current generation and latest live verified snapshot before the requested instant. It verifies encrypted/source checksums and every ordered WAL capture, rejects missing/retired dependencies and checks SQLite integrity before returning a private staged image.
- Point-in-time recovery reports its actual durable capture boundary; SQLite WAL has no transaction UTC timestamps. Nonmonotonic capture UTC and unsupported pre-snapshot/outside-90-day instants fail closed.
- Shared generation pins exclude concurrent retirement while permitting lease renewal. Final local/control ownership and generation checks prevent stale-owner recovery results.
- All 14 replication checks pass, including real PostgreSQL and SQLite latest/PIT batch replay, effective retention pins, missing/retired WAL rejection, corruption cleanup and lease guards. Operational installation/activation follows in API-0052.
- API-0055 implementation commit: `0c4d58a`.

### API-0055 completed

- Hourly P10 maintenance preserves the verified pre-cutoff snapshot anchor and every subsequent WAL batch, including older batches required to reach the 90-day window. No anchor means no current-history deletion; the idle current generation keeps its baseline.
- Expired sealed generations can retire their remaining history. Unverified rows, source spool and keys are excluded. Retirement and completion receipts remain in PostgreSQL for audit, numbering and interrupted-deletion retries; restore must honor retirement even when the remote index is older.
- At most 100 objects per pass; each bounded delete holds freshly checked tenant/lease rows and verifies remote absence before completion. Failed deletion retains its durable queue.
- Three retention planning checks and all 13 replication integration checks pass, including real PostgreSQL dependency preservation, unverified guards, interrupted receipt recovery and expired/stale owner rejection.
- Implementation follows WAL batching fix `2f94c0d`.

### API-0050 completed

- Every replication generation gets a verified consistent baseline before WAL publication; active tenants get daily snapshots with a 24-hour deduplication guard.
- API-0021 hooks take and verify pre/post snapshots around real migrations using the already gated writer. Ownership loss rejects immediately without reopening or deadlocking the migration handle.
- Snapshot metadata captures schema version, durable event watermark, WAL capture position, UTC instant, source/encrypted checksums and size. Source copies, authenticated streaming zstd/AES chunks, immutable conditional uploads and downloaded-stream hashing precede manifest/control verification and local cleanup.
- Snapshot/WAL manifests share monotonically allocated revisions, with migration support for existing WAL-only immutable revision names. Keys fail closed before temporary copies; snapshot temporary directories are private.
- Verified: ten replication checks, including real PostgreSQL baseline/daily/pre/post manifest records, decoding a valid SQLite image, lease-change/restore/gap rotation, missing keys, legacy manifests, stream corruption/truncation and migration ownership loss. The 50-tenant migration/retry regression also passes.
- Live Wasabi and staging recovery evidence remain part of the M8 launch gate.

### API-0054 completed

- Control-plane-selected generations rotate on lease changes, explicit restores and detected capture-number/frame gaps. Previous generations retain their sealed/restored/gapped state; manifests include the ownership generation and start reason.
- Persisted restore transition UUIDs make retries idempotent and reject superseded transitions. Multiple lineages within one ownership lease are supported.
- Tenant/lease row locks are followed by current database-clock and local monotonic-lease checks; former owners cannot reserve or verify manifest entries.
- Eight replication checks pass, including real PostgreSQL restore/gap rotation, manifest ordering, retry identity, superseded transition rejection and cached-owner fencing.
- New-generation snapshot baselines are the next dependency (API-0050); no recovery readiness is asserted before that baseline is verified.

### API-0053 completed

- Built-in per-cell capture and per-tenant upload workers disable automatic/close checkpoints, validate the committed WAL-index boundary and every frame checksum, and durably spool before checkpointing.
- Persisted capture ordering survives restarts and clock changes. Reader-blocked checkpoints deduplicate; closed tenants with pending spool reopen only under an existing lease.
- zstd plus AES-256-GCM authenticates the tenant object key. Separate durable per-tenant key mount; persisted ciphertext makes retries exact. Immutable uploads, read-back verification, conditional generation manifest updates and PostgreSQL lease fencing precede spool deletion.
- Exposes spool bytes, oldest unshipped capture age and failures; alert rules and configuration/runbook are checked in. The object-store outage retains local evidence.
- Verified 2026-10-08: eight replication tests (including the isolated PostgreSQL guard) and seven tenant database regressions pass. Captured WAL restores a real SQLite database with `integrity_check=ok`; no live Wasabi or staging drill is claimed.
- Follow-up verified 2026-10-08: two-minute uploads combine contiguous captures in one persisted, authenticated batch; interrupted verified cleanup resumes, and reader-pinned acknowledged prefixes do not upload again. Immutable manifest revisions store compact deltas. All 12 replication checks pass, including real PostgreSQL.
- Snapshots and the remaining M8 launch gates follow in plan order.

### M7 completed

- `API-0043`, `TEST-0030`, and `OPS-0021` are complete. All fourteen report routes use fenced tenant DuckDB readers with current venue grants, hot-window/readiness/cache guards, exact money and tenant calendars.
- All 31 immutable M0 golden reports pass with documented observation-time, UTC and tied-boundary normalization; schema upgrades preserve sealed summaries and older snapshots trigger rebuilding.
- Removed the retired reporting engine code, configuration, Compose service and instructions. Repository searches find no engine references.
- Final native integration run: 427 backend/control/live checks pass. Default all-target compilation passes; 13 admin analytics tests and actual overview/eleven-subpage browser checks pass.
- A fresh operational v2 demo drained 2,348 events and served native analytics. Source parity independently matches 1,419 rows across 27 projections, all exact money totals and 486,000 occupied seconds.
- API/report parity implementation: `88ab8cb`. M7 is merged locally; no remote push or deployment was performed.

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
- The former external reporting gates are replaced by native report tests in M7; JetStream gates run against disposable servers.

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
- Implementation: `3b8fe64`. Next: `API-0042` consistent rebuild and production activation.

### API-0042 completed

- Consistent private SQLite snapshots, read-only signed-extension attachment, exact bulk projections, Rust calendar labels, session hours, monthly summaries and retained-stream replay build shadow files while operations continue. Fenced atomic file replacement occurs after replay reaches the post-backfill source watermark.
- Schema v2 persists the pre-snapshot broker position to skip superseded deliveries after restore/restart. Failed builds preserve canonical facts; corrupt/old files are quarantined, newer schemas remain intact, and old monthly aggregates survive ordinary rebuilds.
- All 401 backend/control/live checks pass. Native demo parity: 1,419 rows across 27 projections, every money-column total, and 486,000 occupied seconds match SQLite. Concurrent replay, failure preservation, purge-safe T0, self-fencing and superseded retained events are verified.
- Deployment compiles analytics and caches the matching signed extension. The real setup binary, Biome and YAML checks pass; Docker image execution is unverified on this host.
- Implementation: `d0e112e`.

### API-0044 completed

- Nightly tenant-calendar maintenance seals expiring summaries and deletes facts/child collections in bounded, fenced transactions. Open and crossing work survives; checkpoints and sealed history remain intact. Pending SQLite watermarks delay maintenance until ingestion catches up.
- Full regression: 404 backend/control/live checks pass. Five targeted retention checks pass, including two final source-backlog/DST cases: 406 distinct passing checks across the runs.
- Implementation: `c1e1eca`.

### API-0045 completed

- Control timezone revisions project locally without synchronous business dependence on PostgreSQL. Live and restarted analytics preserve old facts/checkpoints in REBUILDING until labels are derived again from unchanged UTC instants. Signed entitlement revisions advance; stale/conflicting updates cannot revert calendars.
- 20 targeted checks, nine disposable control checks and five live JetStream gates pass, including demo parity and a timezone change during a rebuild. Historical aggregates from a different calendar require archived facts for accurate relabeling (M11).
- Implementation: `fc68082`.

### API-0046 and M6 completed

- Per-cell priority/FIFO admission reserves outbox capacity and bounds background/backfill concurrency. Foreground writers bypass queues; pull buffers and DuckDB opens reserve capacity before allocation. Cancelled and fenced jobs cannot leak reservations or commit stale data. Queue/active/admission/release/wait metrics are exported by priority.
- Full regression: 417 backend/control/live checks pass. Final targeted safeguards (21 checks) and all six live gates pass: 418 distinct checks across the runs. The actual NATS stop/restart preserves SQLite writes and drains the outbox without duplicate messages. Native demo counts/money/session-hour parity and scheduled concurrent rebuild/replay pass.
- Implementation: `a8469b9`; M6 merged locally to `main`. No remote push or deployment.
- Next: M7 `API-0043` report migration and M0 fixture parity.

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
- Latest completed milestone: M7; the final native run passes 427 backend/control/live checks, plus 13 admin analytics checks.
- Five inventory/procurement SQLite integration tests pass, including atomic receipt financial links, duplicate invoice rollback, concurrent fulfillment, and lease fencing.

- Four finance and eight commerce integration tests pass, covering concurrent start/approval, handover rollback, deposit reversal after closure, and atomic sale/settlement cash entries.

- Settings/configuration, notification retention, access edit, migration preservation, and kitchen checkout tests pass; full backend suite and final focused configuration regressions pass.

## Update rule

After each task:

1. Update the source build plan checkbox and completion note.
2. Update this file's date, current task, milestone checklist, and verification status.
3. Record the implementation commit.
