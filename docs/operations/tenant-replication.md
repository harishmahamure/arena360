# Tenant WAL replication

M8 implementation work is in progress. This guide covers the WAL worker; it is
not evidence that the snapshot/restore or staging launch gates have passed.

## Configuration

Every cell captures committed SQLite WAL to
`TENANT_DATA_DIR/tenant-{id}/replication/spool`. Capture runs once per second for
open tenants and on close. SQLite automatic and close-time checkpoints are
disabled. The worker durably writes the committed WAL prefix and metadata before
running a truncate checkpoint. Readers may delay truncation; repeated prefixes
are deduplicated by checksum. A capture failure withholds the checkpoint.

Enable remote uploads with both:

- `REPLICATION_BUCKET`: the Wasabi bucket.
- `REPLICATION_KEY_DIR`: a separate, durable secret mount. Each tenant has a raw
  32-byte AES key in `{tenant-id}.key`. Provision it before enabling replication
  for that tenant. Keep it recoverable independently of the cell's NVMe; exclude
  it from database snapshots, object manifests, container images and Git.

The S3 client reads the standard `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` and
`AWS_REGION`. `REPLICATION_ENDPOINT` sets the Wasabi S3 endpoint. Optional
`REPLICATION_ALLOW_HTTP=true` is for a local S3 stand-in only. Media asset
`STORAGE_*` settings are independent.

No tenant key is generated or substituted automatically. A missing or invalid
key preserves the spool and reports a failure. Tenant-key deletion must remove
the independently provisioned key and its secret-store recovery copies; the
provider exposes a durable deletion operation. Lifecycle wiring is still part
of subsequent platform work.

Uploads start when the oldest capture reaches 120 seconds or the pending source
bytes reach 8 MiB. Each upload batch combines a contiguous prefix of captures,
targeting 8 MiB (at most 4,096 captures); an individual large capture remains
bounded at 256 MiB. Batch descriptors and raw bytes are durable before upload,
and retries preserve the batch boundary even while more writes arrive.
Segments use tenant and generation prefixes with monotonically numbered,
immutable keys. Their zstd bytes are encrypted with AES-256-GCM, authenticating
the complete object key. Encoded retry bytes are persisted locally, so retries
use exactly the original nonce and ciphertext.

## Verification and failures

The worker reads back every upload and compares all bytes. It then publishes an
immutable generation manifest revision, updates `manifest.json` conditionally,
reads that back, and records verification in PostgreSQL while checking ownership
and lease expiry. Only then may it delete source spool files.

An existing immutable object with different bytes is an incident, not an
overwrite opportunity. Preserve local evidence and resolve the object collision.
Never manually checkpoint, delete the spool, reset the WAL, or use a backup from
another ownership generation to bypass an error.

Object storage or control-plane outages retain local spool. Operational writes
continue using their existing lease; normal ownership self-fencing still applies
if lease renewal fails. An object storage outage can exceed the normal recovery
point window if the entire cell disk is then lost.

## Monitoring

Load `infra/monitoring/replication-alerts.yml` into the monitoring installation.
The cell's `/metrics` endpoint exposes:

- `arena360_replication_spool_bytes`: spool files (including encoded retry artifacts) across the local root,
  including closed or fenced tenants.
- `arena360_replication_oldest_unshipped_milliseconds`: age of the oldest capture.
- `arena360_replication_failures_total`: capture, upload or manifest failures.

The rules alert on age above 150 seconds, spool above 1 GiB and recent failures.
Alert evaluation and a real Wasabi outage drill remain required staging evidence.
Before paying-venue onboarding, complete all M8 snapshot, restore, disk-pressure,
cell-loss and weekly-drill gates in the build plan. Publish measured recovery
time from those drills; no availability target is assumed here.

## Generation ownership

The control plane's `tenants.current_replication_generation` selects the lineage
used for recovery. Do not select a generation by object listing order or folder
timestamp. A lease change starts a new generation; restore and detected WAL gaps
also rotate it, even within the same ownership generation. Previous lineages are
retained with `SEALED`, `RESTORED` or `GAPPED` state for investigation and retention.

Restore orchestration persists a transition UUID before calling generation
rotation. A retry with that UUID returns the same current generation. Once a
later transition supersedes it, a delayed retry is rejected. Capture-number gaps
and backwards frame boundaries conservatively create a new lineage; they never
append to an apparently continuous history. A new lineage still needs its
verified snapshot baseline from API-0050 before it can be used for recovery.

Manifest reservation and verification lock the tenant and lease rows, then
recheck the actual database clock and the process's monotonic lease after lock
acquisition. A locally cached former owner's lease cannot authorize control-plane
manifest writes after reassignment.

## Snapshot protocol

Each generation receives a verified baseline before its WAL is published. Active
tenants are checked every minute for a daily snapshot; an already verified daily
snapshot within 24 hours suppresses another copy. `SnapshotHook` integrates with
the API-0021 migration orchestrator and verifies pre/post snapshots around the
actual schema migration. An unavailable backup prevents migration from proceeding.

A consistent `VACUUM INTO` copy is taken while the tenant writer is gated. The
gate is released before normal daily/baseline compression and network upload.
Migration hooks use their already gated writer and reject a changed ownership
generation without reopening it. Snapshot metadata records the schema version,
durable event sequence, capture position, UTC snapshot instant, source SHA-256,
encrypted SHA-256 and encrypted size.

Compression and encryption stream through 64-KiB authenticated chunks. Chunk
position and an authenticated final marker detect rearrangement and truncation.
Completed encrypted artifacts are read-only mapped for conditional upload;
verification hashes the downloaded stream. Source and encrypted temporary files
remain available until remote object, generation manifest and control-plane
verification succeed. Retries retain the same object key and ciphertext.

Snapshot and WAL manifests share a monotonically allocated revision namespace.
Each revision lists verified snapshots and segments, plus the artifact being
verified. Conditional pointer updates prevent a delayed request from replacing a
newer revision. Recovery must also use the control-plane verification records;
an uploaded object alone is not a completed backup.

Immutable revision objects contain the changed entry and the full manifest's
checksum, rather than duplicating its complete history on every upload. The
well-known manifest contains the current verified index. WAL batch metadata
records its first and last capture positions; each enclosed capture keeps its
own timestamp, WAL header and frame checksum chain for ordered/PIT replay.

Local acknowledgement is persisted before capture cleanup. An acknowledged WAL
prefix pinned by an old reader does not create a new upload when there are no
writes. A pending batch descriptor also resumes cleanup if the process stops
after source files were removed.

## Backup retention

The owning cell runs retention hourly at P10 maintenance priority, serialized
with that tenant's backup publication. Each pass handles at most 100 objects.
The cutoff uses PostgreSQL UTC time minus 90 days. The newest verified snapshot
at or before the cutoff remains the anchor; every WAL batch after its capture
position remains, including batches older than the cutoff. A batch expires only
when its last capture is old enough and the anchor covers its entire range.
Without an anchor, current-generation history is retained. An idle current
generation keeps its last baseline. A sealed noncurrent generation expires only
after its seal, snapshots and all verified captures have aged out.

PostgreSQL `retired_at` is a durable deletion intent. Restore must exclude these
rows even if an older object-store manifest still lists them. Metadata stays in
the ledger for numbering, audit and retries. Remote deletion treats absence as
success and verifies absence before recording `deleted_at`. An outage leaves
intents pending. Each deletion requires a fresh owning lease and holds the
control-plane tenant/lease locks across a bounded remote operation, preventing
reassignment from racing a former owner's delete. Unverified objects, local
spool and encryption keys are outside this retention job.
