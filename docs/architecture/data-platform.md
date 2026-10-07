# Arena360 Data Platform Architecture

**Status:** Production Architecture Baseline  
**Version:** 2.0

This document is the source of truth for the Arena360 data platform. Decisions, open questions, and implementation constraints are recorded in `docs/adr/0043-storage-cells-sqlite-duckdb.md`. The DuckDB analytical schema is specified in `docs/architecture/duckdb-analytics-schema.md`.

## Amendments

- **2026-10-06, Wasabi replaces Cloudflare R2.** All object storage (replication, hot Parquet copies, archive, exports) uses Wasabi. Every "R2" below reads as Wasabi. The bucket layout in §22 is replaced by ADR-0043 decision 31.
- **2026-10-06, continuous WAL replication in the MVP.** §48's "future" incremental WAL replication is part of the first production release:
  - A WAL Replication Worker spools WAL frames on local NVMe and uploads them in batches to Wasabi every 2 minutes, or earlier at a size threshold.
  - Segments are immutable.
  - Snapshots are daily, plus before and after every migration.
  - WAL and snapshots are kept for 90 days, so any point in that window can be restored.
  - A cell failure loses up to about 2 minutes of writes instead of everything since the last snapshot.

  Details: ADR-0043 decision 26.
- **2026-10-06, UTC everywhere.** Every stored and transmitted timestamp is UTC. Each tenant's IANA time zone is stored in the global PostgreSQL and used only to derive calendar labels (report days and hours) and to resolve calendar date ranges. Details: ADR-0043 decisions 27–29.

## Technology

- **Global Services:** Rust
- **Control Plane:** PostgreSQL
- **Tenant OLTP:** SQLite
- **Tenant Analytics:** DuckDB
- **Messaging:** NATS JetStream
- **Object Storage:** Wasabi (amended 2026-10-06; originally Cloudflare R2)
- **Storage:** Local NVMe on Storage Cells

---

# 1. Architecture Goals

Arena360 is designed around:

- physical tenant data isolation,
- inexpensive commodity/cheap servers,
- horizontal Storage Cell scaling,
- easy movement of tenants between servers,
- local low-latency SQLite OLTP,
- asynchronous analytics,
- limited hot-data retention,
- cheap long-term R2 storage,
- rebuildable DuckDB databases,
- deterministic disaster recovery,
- no dependency on analytics for operational availability.

The fundamental scaling unit is not one huge database cluster.

It is:

```text
Storage Cell
+
N isolated tenant databases
```

---

# 2. High-Level Architecture

```text
                    GLOBAL CONTROL PLANE

                    ┌─────────────────────┐
                    │ Global Rust Service │
                    │                     │
                    │ Auth                │
                    │ Tenant Management   │
                    │ Subscription        │
                    │ Licensing           │
                    │ Location Management │
                    │ Routing             │
                    │ Storage Placement   │
                    └──────────┬──────────┘
                               │
                               ▼
                    ┌─────────────────────┐
                    │ Global PostgreSQL   │
                    │                     │
                    │ tenants             │
                    │ subscriptions       │
                    │ licenses            │
                    │ locations           │
                    │ storage ownership   │
                    │ schema versions     │
                    │ tenant generations  │
                    └──────────┬──────────┘


===========================================================

                       DATA PLANE

                         Router
                           │
                  tenant → owner cell
                           │
          ┌────────────────┼────────────────┐
          ▼                ▼                ▼

      Storage Cell 1   Storage Cell 2   Storage Cell N
          │                │                │
          ▼                ▼                ▼
     Rust Storage      Rust Storage      Rust Storage
        Service           Service           Service
          │
    ┌─────┴────────────┐
    │                  │
    ▼                  ▼
SQLite              DuckDB
OLTP                Analytics
    │                  ▲
    │ Outbox           │
    ▼                  │
Publisher             │
    │                  │
    ▼                  │
NATS JetStream ────────┘
    │
    ├────────► Analytics consumers
    ├────────► R2 historical datasets
    └────────► future consumers


SQLite backups ──────────────────────────► R2
Historical Parquet ──────────────────────► R2
Exports ─────────────────────────────────► R2
```

---

# 3. Data Ownership

There are four distinct classes of state.

## Global PostgreSQL

Authoritative for:

```text
tenant
subscription
license
location metadata
storage placement
tenant ownership
storage engine
schema version
tenant generation
billing metadata
feature entitlement
```

PostgreSQL is the **control-plane source of truth**.

---

## Tenant SQLite

Authoritative for operational data:

```text
customers
memberships
sessions
bookings
payments
POS
inventory
staff
pricing
machines
locations
configuration
outbox events
```

SQLite is the **tenant operational source of truth**.

---

## Tenant DuckDB

Contains derived analytical data.

It is:

```text
rebuildable
replaceable
non-authoritative
```

Losing DuckDB must never mean losing business transactions.

---

## R2

R2 contains:

```text
SQLite backups
archived historical data
Parquet analytical files
event/archive datasets
generated exports
temporary recovery objects
```

---

# 4. Tenant Storage Unit

Every tenant is treated as a portable storage unit.

Conceptually:

```text
tenant-01922/
│
├── main.sqlite
├── analytics.duckdb
└── runtime metadata
```

Only this file is mandatory for operational recovery:

```text
main.sqlite
```

DuckDB is reconstructable.

---

# 5. Storage Cell

A Storage Cell contains:

```text
Storage Cell
│
├── Rust Storage Service
│
├── local NVMe
│
├── tenant-A.sqlite
├── tenant-A.duckdb
├── tenant-B.sqlite
├── tenant-B.duckdb
├── tenant-C.sqlite
├── tenant-C.duckdb
│
├── Outbox Publisher
├── Analytics Consumer
├── Backup Worker
├── Archive Worker
├── Export Worker
├── Migration Worker
└── Recovery/Hydration Worker
```

The tenant capacity of a cell is determined from benchmark measurements.

Never define:

```text
1 cell = exactly 200 tenants
```

Instead calculate weighted capacity using:

```text
write TPS
read QPS
SQLite size
DuckDB size
WAL generation
analytics CPU
RAM
NVMe latency
NVMe space
background workload
```

---

# 6. Tenant Routing

PostgreSQL stores:

```text
tenant_id
owner_cell
storage_engine
generation
schema_version
state
```

Example:

```text
tenant_id       TEN-01922
owner_cell      CELL-07
storage_engine  SQLITE
generation      947
schema_version  51
state           ACTIVE
```

Rust routing caches this information.

```text
request
   ↓
authenticate
   ↓
tenant_id
   ↓
routing cache
   ↓
owner Storage Cell
```

---

# 7. Tenant Ownership Lease

Two Storage Cells must never independently write the same tenant.

Invalid:

```text
                   Tenant 123

Cell 07                           Cell 21
   │                                │
tenant.sqlite                   tenant.sqlite
   │                                │
writes                            writes

              X INVALID
```

PostgreSQL maintains an ownership lease.

Conceptually:

```text
tenant_id
owner_cell
ownership_generation
lease_state
```

Only the holder of the current ownership generation may open SQLite in writable mode.

Example:

```text
tenant = 123

owner_cell           = CELL-07
ownership_generation = 9822
```

CELL-21 cannot simply restore Tenant 123 from R2 and begin writing.

It must first acquire ownership.

---

# 8. Tenant Generations

Every recoverable tenant snapshot receives a generation.

Example:

```text
generation 150
generation 151
generation 152
```

R2:

```text
tenant-123/
    backups/
        gen-150.sqlite.zst
        gen-151.sqlite.zst
        gen-152.sqlite.zst
```

The manifest contains:

```text
tenant_id
generation
schema_version
checksum
created_at
last_event_sequence
source_cell
```

This prevents ambiguity during recovery.

---

# 9. Storage Cell Replacement

Storage servers should be disposable.

Desired property:

```text
Global PostgreSQL
       +
R2
       +
available Storage Cell

       ↓

Tenant can be restored
```

A Storage Cell should not represent permanent tenant ownership.

---

# 10. Local File Resolution

When a Storage Cell receives an assigned tenant:

```text
tenant request
      │
      ▼
Does valid local SQLite exist?
      │
   ┌──┴──┐
  YES    NO
   │      │
 OPEN     ▼
       acquire/verify ownership
             │
             ▼
       resolve latest generation
             │
             ▼
          download R2
             │
             ▼
       decompress + verify
             │
             ▼
       validate SQLite
             │
             ▼
             OPEN
```

File absence alone must **never** grant ownership.

---

# 11. Cold Tenant Hydration

Inactive tenants may eventually be stored only in R2.

Example:

```text
Global PostgreSQL

tenant = 500
state  = COLD
```

R2 contains:

```text
latest verified SQLite snapshot
```

No local Storage Cell needs to permanently hold that tenant.

First request:

```text
request
   ↓
tenant is COLD
   ↓
assign Storage Cell
   ↓
acquire ownership
   ↓
download latest SQLite
   ↓
verify
   ↓
hydrate
   ↓
ACTIVE
```

This allows cheap servers to hold primarily active tenants.

---

# 12. Active Tenant Policy

Active tenants use:

```text
Local NVMe SQLite = primary
R2                 = backup/recovery
```

Do not download an active tenant from R2 on every request.

---

# 13. Tenant Movement Between Servers

Tenant migration state:

```text
ACTIVE
   ↓
PREPARING_MOVE
   ↓
COPYING
   ↓
CUTOVER
   ↓
VERIFYING
   ↓
ACTIVE
```

Migration:

```text
Old Storage Cell
       │
       ▼
consistent SQLite snapshot
       │
       ▼
New Storage Cell
       │
       ▼
verify
       │
       ▼
synchronize latest changes
       │
       ▼
short write gate if required
       │
       ▼
atomic ownership/routing change
       │
       ▼
New Cell ACTIVE
       │
       ▼
retain old copy temporarily
```

A server can therefore be replaced gradually.

Example:

```text
Cheap Cell 01
   ↓
move Tenant 1
move Tenant 2
move Tenant 3
...
   ↓
Cell 01 empty
   ↓
decommission
```

---

# 14. Operational Write Flow

Business request:

```text
POST /sessions/end
```

One SQLite transaction:

```text
BEGIN

business mutations

+

INSERT outbox_event

COMMIT
```

Then:

```text
API returns success
```

NATS and DuckDB are not part of the synchronous transaction.

---

# 15. Transactional Outbox

SQLite contains:

```text
outbox_events
```

Every analytically significant business mutation generates an event in the **same SQLite transaction**.

Example:

```text
session.started
session.completed
payment.completed
membership.renewed
inventory.adjusted
booking.created
```

Required fields:

```text
event_id
tenant_id
location_id
aggregate_id
event_type
occurred_at
schema_version
payload
```

---

# 16. Outbox → JetStream

Rust periodically publishes pending events.

```text
SQLite
   ↓
Outbox Publisher
   ↓
NATS JetStream
```

Suggested initial cadence:

```text
250 ms – 1 second

or

publish when batch threshold reached
```

The exact configuration is benchmark driven.

---

# 17. JetStream Failure

If JetStream becomes unavailable:

```text
Operational writes
       ↓
SQLite
       ↓
continue normally
```

Outbox grows.

When JetStream returns:

```text
Rust drains backlog
```

Monitor:

```text
pending event count
oldest event age
outbox bytes
publish failures
```

---

# 18. DuckDB Ingestion

Do not perform one DuckDB transaction per event.

Instead:

```text
JetStream
    ↓
analytics consumer
    ↓
buffer
    ↓
N events OR maximum wait
    ↓
DuckDB transaction
```

Example initial configuration:

```text
500–1000 events

OR

1 second
```

Benchmark and adjust.

---

# 19. DuckDB Ownership

One controlled process owns writes to each tenant DuckDB.

Avoid:

```text
API Process 1 ─┐
API Process 2 ─┼─> same DuckDB file
API Process 3 ─┘
```

Prefer:

```text
Analytics Worker
       │
       ▼
Tenant DuckDB
```

---

# 20. Hot Data Policy

Arena360 keeps approximately:

```text
18 months
```

of hot operational/analytical data.

Conceptually:

```text
NOW
 │
 │ 18 months
 ▼

HOT DATA
```

Hot data is available for:

```text
normal application screens
dashboards
interactive reports
analytics
```

---

# 21. Data Lifecycle

The intended lifecycle is:

```text
                 CURRENT DATA

                     │
                     ▼

             ┌─────────────────┐
             │ HOT — 18 MONTHS │
             │                 │
             │ SQLite          │
             │ DuckDB          │
             └────────┬────────┘
                      │
               age > 18 months
                      │
                      ▼
             ┌─────────────────┐
             │ ARCHIVE — OLD   │
             │                 │
             │ R2 / Parquet    │
             └────────┬────────┘
                      │
                      ▼
             ┌─────────────────┐
             │ ARCHIVE — OLDER │
             │                 │
             │ R2 / Parquet    │
             │ lifecycle tier  │
             └─────────────────┘
```

R2 becomes the long-term historical system.

---

# 22. R2 Structure

> **Amended 2026-10-06:** replaced by the Wasabi layout in ADR-0043 decision 31 (`tenants/{id}/replication/generations/{generation_id}/{snapshots,wal}`, `hot/`, `archive/`, `exports/`). The original recommendation is kept below for reference.

Recommended:

```text
arena/
│
├── tenants/
│   └── tenant-123/
│
│       ├── backups/
│       │   ├── gen-150.sqlite.zst
│       │   ├── gen-151.sqlite.zst
│       │   └── gen-152.sqlite.zst
│       │
│       ├── archive/
│       │   │
│       │   ├── year=2024/
│       │   │   ├── month=01/
│       │   │   ├── month=02/
│       │   │   └── ...
│       │   │
│       │   ├── year=2025/
│       │   └── ...
│       │
│       ├── exports/
│       │
│       └── manifest/
│
├── temp/
└── system/
```

---

# 23. R2 Logical Tiers

Logical data states:

## HOT

```text
last 18 months
```

Primary representation:

```text
SQLite
+
DuckDB
```

R2 may still contain backup copies.

---

## ARCHIVE

Older than approximately 18 months.

Primary historical representation:

```text
R2 Parquet
```

No requirement to keep those rows in SQLite.

---

## DEEP ARCHIVE / OLD ARCHIVE

Very old historical datasets remain in R2 and may use cheaper R2 lifecycle/storage policies where appropriate.

Application code should not care which R2 archival lifecycle tier an object currently occupies.

---

# 24. Archival Process

Archive is always:

```text
EXPORT
  ↓
UPLOAD
  ↓
VERIFY
  ↓
PURGE
```

Never:

```text
DELETE
  ↓
hope backup exists
```

---

# 25. Archive Granularity

Business policy may say:

```text
archive yearly
```

Physical implementation should use:

```text
year
 ↓
month
 ↓
smaller chunks when required
```

Example:

```text
2024
 ├ Jan
 ├ Feb
 ├ Mar
 ...
 └ Dec
```

This avoids huge write transactions.

---

# 26. Archive Manifest

Track:

```text
archive_id
tenant_id
period_start
period_end
schema_version
row_count
checksum
R2 objects
status
created_at
verified_at
purged_at
```

States:

```text
PLANNED
   ↓
EXPORTING
   ↓
UPLOADED
   ↓
VERIFIED
   ↓
PURGING
   ↓
COMPLETE
```

Never purge unless state is:

```text
VERIFIED
```

---

# 27. SQLite Purge Policy

Never:

```sql
DELETE FROM sessions
WHERE created_at < ...;
```

for millions of rows in one transaction.

Instead:

```text
delete small batch
COMMIT
yield

delete next batch
COMMIT
yield
```

The system optimizes for **maximum writer-lock duration**, not number of rows.

If archival activity increases operational write p99:

```text
reduce batch
```

If pressure continues:

```text
pause archival
```

Operational transactions always win.

---

# 28. VACUUM Policy

Archival must **not** automatically trigger a full:

```sql
VACUUM;
```

SQLite can reuse freed pages.

Example:

```text
20 GB file
10 GB active
10 GB free pages
```

This is acceptable.

Future data can reuse the free pages.

Physical shrinking is optional maintenance.

---

# 29. Hot SQLite Backfill

Sometimes old or corrected historical records need to be restored into the operational database.

This is a **SQLite backfill**.

Flow:

```text
R2 archive
    ↓
DuckDB / transformation worker
    ↓
staging dataset
    ↓
validate
    ↓
small SQLite transaction batches
    ↓
operational DB
```

Never directly perform:

```text
millions of archive rows
      ↓
one SQLite transaction
```

Backfill uses:

```text
small bounded batches
idempotency keys
conflict detection
schema conversion
foreign-key validation
progress checkpoints
```

---

# 30. SQLite Backfill State Machine

```text
PLANNED
   ↓
DOWNLOADING
   ↓
TRANSFORMING
   ↓
VALIDATING
   ↓
BACKFILLING
   ↓
VERIFYING
   ↓
COMPLETE
```

Store:

```text
backfill_id
tenant_id
source_archive
target_schema_version
last_processed_key
rows_processed
rows_failed
started_at
completed_at
```

Backfills are resumable.

---

# 31. Backfill Priority

Backfills are background jobs.

Priority:

```text
customer operations
       ↑
outbox publishing
       ↑
backup
       ↑
normal analytics
       ↑
migration backfill
       ↑
archive/backfill
```

If OLTP latency increases:

```text
pause backfill
```

---

# 32. DuckDB Backfill

DuckDB must support deterministic rebuild/backfill.

Three cases exist.

## Case A — New Storage Cell

SQLite has been restored but DuckDB does not exist.

```text
SQLite restored
      ↓
tenant operational
      ↓
create empty DuckDB
      ↓
backfill hot 18 months
      ↓
replay subsequent JetStream events
      ↓
analytics READY
```

Operational availability does not wait for DuckDB.

---

## Case B — DuckDB Corruption

```text
DuckDB corrupted
      ↓
mark analytics REBUILDING
      ↓
delete/recreate DuckDB
      ↓
backfill current hot data
      ↓
replay event gap
      ↓
READY
```

---

## Case C — Analytics Schema Change

```text
analytics schema v8
       ↓
create v9 tables
       ↓
backfill
       ↓
validate
       ↓
switch analytical views
       ↓
remove v8 later
```

---

# 33. DuckDB Backfill Source

For the normal hot analytics database:

```text
SQLite
   ↓
last 18 months
   ↓
DuckDB
```

DuckDB should not automatically reload the entire historical life of the tenant.

Therefore:

```text
DuckDB hot database
=
approximately 18 months
```

This keeps analytical databases bounded.

---

# 34. DuckDB Backfill Boundary

Suppose:

```text
today = October 2028
```

The normal rebuild window might be:

```text
April 2027 → October 2028
```

Anything older stays in:

```text
R2 archive
```

and is not automatically loaded into the tenant's hot DuckDB database.

---

# 35. DuckDB Rebuild Consistency

Backfill needs a boundary.

Example:

```text
T0 = backfill snapshot boundary
```

Process:

```text
1. remember event sequence at T0

2. backfill SQLite hot dataset

3. finish DuckDB bulk load

4. replay JetStream events > T0

5. catch up

6. mark analytics READY
```

This prevents the rebuild from missing transactions that happen during the backfill.

---

# 36. Analytics Availability States

Track:

```text
READY
LAGGING
REBUILDING
FAILED
```

Operational APIs ignore these states.

Analytics endpoints can return:

```text
report temporarily rebuilding
```

without affecting the café.

---

# 37. Historical Analytics Policy

Interactive analytics cover the hot window only.

Default:

```text
last 18 months
```

This means normal dashboard queries do NOT scan years of R2 archive data.

---

# 38. Older Historical Data

Older data is available through **exports only**.

Example user request:

```text
Download revenue report
January 2022 – December 2024
```

Arena360 does not pull the whole history back into the normal DuckDB database.

Instead it launches an Export Job.

---

# 39. Historical Export Architecture

```text
User requests old report
         │
         ▼
     Export API
         │
         ▼
    Export Job Queue
         │
         ▼
      DuckDB Worker
         │
         ▼
   Read R2 Parquet directly
         │
         ▼
 filter / aggregate / join
         │
         ▼
 generate export
         │
         ▼
        R2
         │
         ▼
   signed download
```

DuckDB therefore powers historical analytics **without rehydrating old data into the hot database**.

---

# 40. Export Formats

Initially support:

```text
CSV
Parquet
```

Optionally:

```text
XLSX
```

for smaller business-friendly exports.

Very large exports should prefer:

```text
CSV.gz
or
Parquet
```

---

# 41. Export Job State

```text
QUEUED
  ↓
PREPARING
  ↓
SCANNING_ARCHIVE
  ↓
GENERATING
  ↓
UPLOADING
  ↓
READY
```

Failures:

```text
FAILED
CANCELLED
EXPIRED
```

Store:

```text
export_id
tenant_id
requested_by
date_range
filters
format
status
R2_result_path
created_at
expires_at
```

---

# 42. Export Isolation

Historical exports can be CPU and I/O intensive.

Do not execute large exports inside the request-handling process.

Use a controlled pool:

```text
Export Worker 1
Export Worker 2
Export Worker 3
```

Limit concurrency per:

```text
Storage Cell
tenant
global platform
```

One customer requesting ten years of exports must not slow operational APIs.

---

# 43. Download-Only Historical Analytics

Product rule:

```text
0–18 months
→ dashboards + interactive analytics

>18 months
→ export/download
```

This is an intentional architecture decision.

It prevents R2 archive scans from becoming part of normal dashboard latency.

---

# 44. Optional Historical Summary Tables

If users commonly need long-term comparisons such as:

```text
5-year revenue trend
```

Arena360 may retain tiny aggregated summaries in hot DuckDB:

```text
monthly revenue
monthly utilization
membership counts
yearly totals
```

Example:

```text
2023 → 12 aggregate rows
2024 → 12 aggregate rows
2025 → 12 aggregate rows
```

This is very different from retaining millions of raw historical transactions.

Raw history remains in R2.

---

# 45. Backup Architecture

Backups are distinct from archives.

```text
BACKUP
=
recover system state

ARCHIVE
=
long-term historical retention
```

R2 stores both, under separate namespaces.

---

# 46. Backup Policy

For each active tenant:

```text
SQLite
   ↓
consistent snapshot
   ↓
compress
   ↓
checksum
   ↓
R2 backup generation
   ↓
verify
```

Metadata includes:

```text
tenant
generation
schema version
checksum
event sequence
timestamp
```

---

# 47. Backup Restore

Restore:

```text
R2
 ↓
download latest valid generation
 ↓
verify checksum
 ↓
decompress
 ↓
SQLite integrity validation
 ↓
open operational database
```

DuckDB recovery happens separately afterward.

---

# 48. RPO Warning

R2 snapshots alone do not provide zero-data-loss failover.

Example:

```text
12:00 backup

12:03 payment

12:05 server failure
```

Restoring only the 12:00 backup loses later state.

Future architecture may add:

```text
snapshot
+
incremental WAL/log replication
```

to achieve lower RPO.

> **Amended 2026-10-06:** snapshot plus continuous WAL replication to Wasabi (2-minute batches, 90-day retention) is part of the MVP (see Amendments and ADR-0043 decision 26).

---

# 49. Server Failure Recovery

```text
Cell 07 fails
      ↓
identify affected tenants
      ↓
assign replacement Cell
      ↓
acquire ownership generation
      ↓
restore SQLite
      ↓
recover incremental state if available
      ↓
make operational API available
      ↓
create/rebuild DuckDB
      ↓
hot-data backfill
      ↓
event catch-up
      ↓
analytics READY
```

Operational service always comes first.

---

# 50. Recovery Priority

```text
1. Tenant ownership

2. SQLite

3. Operational APIs

4. Outbox/JetStream catch-up

5. DuckDB rebuild

6. Analytics

7. historical export capacity
```

---

# 51. PostgreSQL Outage

Global PostgreSQL should not sit directly inside every business operation.

Do not require:

```text
start session
   ↓
global PG license query
   ↓
SQLite
```

Use locally cached/signed tenant entitlement.

This provides a control-plane grace period.

---

# 52. Migration Policy

Use:

```text
EXPAND
  ↓
BACKFILL
  ↓
SWITCH
  ↓
CONTRACT
```

Large migrations are chunked.

Never apply expensive migrations to every tenant simultaneously.

Rollout:

```text
internal/canary
      ↓
1%
      ↓
10%
      ↓
25%
      ↓
100%
```

---

# 53. Archival and Migration Concurrency

Each Storage Cell maintains background concurrency limits.

Example:

```text
1 expensive migration

+

1 archival purge

+

normal analytics jobs
```

Exact limits must be benchmarked.

Foreground customer operations always have reserved capacity.

---

# 54. Background Job Priorities

Recommended ordering:

```text
P0  operational API

P1  SQLite/outbox

P2  critical recovery

P3  backup

P4  analytics ingestion

P5  DuckDB hot backfill

P6  schema migration backfill

P7  archive export

P8  archive purge

P9  historical export

P10 maintenance/compaction
```

Historical exports may receive higher priority when user initiated, but must remain resource constrained.

---

# 55. SQLITE_BUSY Policy

Customer operation:

```text
small bounded retry
+
jitter
```

Background operation:

```text
back off
```

Never increase `busy_timeout` indefinitely.

Track:

```text
busy count
wait duration
writer transaction duration
```

---

# 56. Large Transaction Policy

Administrative tasks must not perform massive transactions.

Applies to:

```text
archive deletion
migration
backfill
bulk correction
data import
data repair
```

Default:

```text
small batch
COMMIT
yield
```

---

# 57. Disk Management

Monitor:

```text
SQLite
SQLite WAL
DuckDB
temporary archive files
temporary export files
outbox
NVMe free space
```

Suggested operational zones:

```text
<70%      healthy
70–80%    warning
80–90%    corrective action
>90%      critical
```

At high pressure:

```text
pause low-priority exports
pause backfills
pause archive staging
move tenants
add capacity
```

---

# 58. Tenant Rebalancing

If Cell 07 becomes hot:

```text
Cell07
  ├ Tenant A
  ├ Tenant B
  ├ Tenant C
  └ Tenant D
```

move:

```text
Tenant C → Cell12
Tenant D → Cell14
```

No application architecture changes.

---

# 59. Hot Tenant Escape Hatch

Maintain:

```text
storage_engine
```

Possible future values:

```text
SQLITE
DEDICATED_SQLITE
POSTGRES
```

Most cafés:

```text
SQLITE
```

Exceptionally large tenant:

```text
DEDICATED_SQLITE
```

or eventually:

```text
POSTGRES
```

Routing hides the storage-engine choice from upper layers.

---

# 60. Observability

## SQLite

```text
read p50/p95/p99
write p50/p95/p99
writer transaction duration
SQLITE_BUSY
WAL bytes
DB bytes
freelist
checkpoint status
```

## JetStream

```text
backlog
consumer lag
redeliveries
ack latency
```

## DuckDB

```text
ingestion lag
query latency
rebuild status
backfill progress
DB size
```

## Archive

```text
oldest unarchived period
export progress
R2 upload failures
verification failures
purge progress
```

## Export

```text
queue depth
job duration
bytes scanned
result size
failure count
```

## Recovery

```text
tenant hydration duration
R2 download duration
restore failures
ownership conflicts
DuckDB reconstruction time
```

---

# 61. Important Alerts

Alert on:

```text
SQLite write p99 degradation
SQLITE_BUSY spike
WAL abnormal growth

disk > threshold

backup overdue
backup verification failure

R2 archive verification failure

outbox backlog
JetStream lag

DuckDB ingestion lag
DuckDB rebuild failure

tenant ownership conflict

migration failure

backfill failure

export queue saturation
```

---

# 62. Common Failure Mitigation Matrix

| Failure | Mitigation |
|---|---|
| Long SQLite transaction | Chunk + commit + yield |
| SQLITE_BUSY | Bounded foreground retry, background backoff |
| Huge archive DELETE | Adaptive batched deletion |
| VACUUM pause | Never automatic in production |
| NATS unavailable | SQLite outbox retains events |
| Duplicate NATS event | Event ID idempotency |
| DuckDB corrupt | Recreate + hot 18-month backfill |
| DuckDB behind | Replay JetStream |
| R2 unavailable | Never purge archive source |
| Storage Cell dies | Acquire ownership elsewhere + restore SQLite |
| Missing tenant file | Restore only after ownership validation |
| Two cells attempt same tenant | Ownership generation/lease |
| Migration fails | Per-tenant resumable migration |
| Backfill interrupted | Checkpoint and resume |
| Huge historical query | Async DuckDB export |
| Old analytics request | Read R2 only through export job |
| Hot tenant | Move/dedicate tenant |
| Disk pressure | Pause background work + move tenants |
| Global PG outage | Cached routing/license grace period |
| Backup corrupt | Automated restore/integrity testing |
| DuckDB missing after move | Rebuild after OLTP restoration |
| Archive schema old | Schema-version-aware DuckDB reader |

---

# 63. Explicit Anti-Patterns

Do not:

```text
synchronously dual-write SQLite + DuckDB
```

Do not:

```text
put NATS in the critical transaction path
```

Do not:

```text
run giant yearly DELETE
```

Do not:

```text
VACUUM after every archive
```

Do not:

```text
restore a missing SQLite file without ownership control
```

Do not:

```text
allow two Storage Cells to write one tenant
```

Do not:

```text
treat DuckDB as authoritative
```

Do not:

```text
rebuild all historical years into normal DuckDB
```

Do not:

```text
serve huge historical archive scans synchronously
```

Do not:

```text
use R2 snapshot alone and claim zero-RPO recovery
```

Do not:

```text
run every tenant migration simultaneously
```

---

# 64. Final Data Lifecycle

```text
                        BUSINESS WRITE
                              │
                              ▼
                           SQLite
                              │
                  ┌───────────┴──────────┐
                  │                      │
                  ▼                      ▼
               Outbox                 Backup
                  │                      │
                  ▼                      ▼
             JetStream                  R2
                  │
                  ▼
               DuckDB
                  │
                  │ HOT ≈ 18 months
                  │
                  ▼
            Interactive Analytics


                 AGE > 18 MONTHS
                        │
                        ▼
                  Archive Worker
                        │
                        ▼
                  R2 / Parquet
                        │
                        ▼
                Long-term Archive
                        │
                        ▼
                Historical Export
                        │
                        ▼
                      DuckDB
                 temporary worker
                        │
                        ▼
                CSV / Parquet / XLSX
                        │
                        ▼
                       R2
                        │
                        ▼
                    Download
```

---

# 65. Final Hot/Archive Policy

## Hot — approximately 18 months

Stored in:

```text
SQLite
+
DuckDB
```

Capabilities:

```text
operational APIs
interactive dashboards
interactive analytics
normal reporting
```

---

## Old — older than approximately 18 months

Stored primarily in:

```text
R2 Parquet
```

Capabilities:

```text
retention
audit
historical recovery
exports
```

No normal interactive dashboard queries against raw old data.

---

## Very Old

Remain in R2 according to long-term lifecycle policies.

Normal tenant infrastructure does not need to load these datasets.

---

# 66. Historical Export Principle

Historical analytics is intentionally:

```text
ARCHIVE
   ↓
DuckDB scan
   ↓
export file
   ↓
download
```

rather than:

```text
ARCHIVE
   ↓
restore everything into tenant DB
   ↓
interactive dashboard
```

This keeps the active system bounded regardless of whether a tenant has:

```text
2 years
5 years
10 years
20 years
```

of history.

---

# 67. DuckDB Recovery Principle

Tenant operational restoration:

```text
SQLite restored
   ↓
tenant online
```

Analytics restoration:

```text
new DuckDB
   ↓
hot 18-month backfill
   ↓
JetStream catch-up
   ↓
READY
```

Historical archive does not need to be reloaded.

---

# 68. Server Portability Principle

A Storage Cell should be considered replaceable infrastructure.

```text
old cheap server
       ↓
new larger server
       ↓
move tenants individually
```

Tenant state is portable because ownership and generation are externalized through PostgreSQL and R2.

The platform should eventually support:

```text
allocate server
      ↓
install Rust Storage Cell
      ↓
assign tenants
      ↓
hydrate SQLite
      ↓
rebuild DuckDB
      ↓
serve
```

without manual database reconstruction.

---

# 69. Architectural Invariants

These rules must always hold.

### Invariant 1

```text
One tenant has exactly one writable owner.
```

### Invariant 2

```text
SQLite is operational truth.
```

### Invariant 3

```text
DuckDB can always be rebuilt.
```

### Invariant 4

```text
R2 archive must be verified before SQLite purge.
```

### Invariant 5

```text
Historical archive never blocks normal OLTP.
```

### Invariant 6

```text
Operational recovery does not wait for analytics recovery.
```

### Invariant 7

```text
Normal analytics remains bounded to the hot window.
```

### Invariant 8

```text
Older raw analytics is delivered through asynchronous exports.
```

### Invariant 9

```text
Storage Cell hardware is replaceable.
```

### Invariant 10

```text
No background process gets priority over customer transactions.
```

---

# 70. Final Architecture Decision

Arena360 uses:

```text
GLOBAL

Rust
+
PostgreSQL
```

for:

```text
routing
subscriptions
licensing
tenant management
location management
storage placement
ownership
```

and:

```text
PER TENANT

SQLite
+
DuckDB
```

for:

```text
operational data
+
hot analytics
```

with:

```text
SQLite
   ↓
transactional outbox
   ↓
JetStream
   ↓
DuckDB
```

and:

```text
18-month hot window
       ↓
verified archival
       ↓
R2 Parquet
```

Historical analytics is:

```text
R2
 ↓
temporary DuckDB export worker
 ↓
downloadable export
```

Storage hardware remains replaceable because tenants can be moved, restored, hydrated, and rebuilt independently.

The core scaling question therefore remains:

> **How many weighted active tenants can a Storage Cell support while preserving the required OLTP latency and recovery guarantees?**

When that boundary is reached, add another Storage Cell rather than vertically scaling a single global database indefinitely.
