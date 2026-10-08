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
bytes reach 8 MiB. Each upload batch handles one capture, bounded at 256 MiB.
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
