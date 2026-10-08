# Weekly restore drills and cell-loss runbook

## Automatic weekly checks

Every API cell with replication configured checks the control plane once a minute.
It restores all SQLite tenants assigned to it in `ACTIVE` state once every seven
days. The first check is one minute after startup. Failed runs retry no more than
once an hour. A control-plane advisory lock prevents concurrent runs for the
same cell, including across process restarts. Interrupted runs retain their
partial results and are marked failed when the next run starts.

Each tenant uses the existing ownership lease and the current replication
generation. The drill downloads authenticated backups, replays verified WAL and
runs SQLite `integrity_check` in private `recovery-staging/drills/` directories.
It neither transfers leases nor opens the staged image for operational requests.
Successful and failed staging is removed. Before a new run, the cell also removes
private restore directories left by a killed previous drill. A ten-minute per-tenant deadline
includes background admission waiting. P10 maintenance admission pauses drills
under corrective disk pressure and gives operational traffic priority.

`cell_restore_drills` stores the selected tenant count, per-tenant recovery
position/time, source generation, image bytes, errors, elapsed milliseconds and final status.
A passing restore proves that the selected uploaded chain can be read and
recovered; writes not yet uploaded are outside that chain.

## Published measurements

The cell's `/metrics` endpoint publishes the latest completed drill:

- `arena360_restore_drill_success`: 1 for pass, 0 for failure.
- `arena360_restore_drill_elapsed_milliseconds`: total restore-only elapsed time,
  including admission wait, downloads, replay, integrity checks and cleanup.
- `arena360_restore_drill_finished_timestamp_seconds`: completion UTC.

The values are reloaded from PostgreSQL after restart. Import the drill alert
rules in `infra/monitoring/replication-alerts.yml`; failure and eight-day absence
are actionable. Apply these rules to replication-enabled cell scrape targets.
Keep instance/cell labels on alerts so an individual missing cell is visible;
monitor scrape failure separately (`up == 0`).

Local fixture measurement (2026-10-08): one tenant, **109 ms** restore-only,
using a local in-memory object store and PostgreSQL. The real process-crash test
using the S3 client and a local HTTP fixture measured a **14.6-second recovery
point lag** after disk loss. These figures describe separate properties and do
not establish a production RTO.

For a durable report, use the control-plane database:

```sql
SELECT cell_id, started_at, finished_at, status, tenant_count,
       elapsed_milliseconds, results
FROM cell_restore_drills
ORDER BY started_at DESC;
```

**Restore-only duration is not the full cell recovery time.** Publish full RTO
from the staging procedure below alongside tenant count, source bytes, hardware,
backup endpoint, lease-expiry waiting time and analytics readiness. Local tests
and a local HTTP S3 fixture are not evidence of staging/Wasabi performance.

## Full staging cell-loss drill

1. Record the cell ID, assigned tenant list, last acknowledged operational writes,
   and latest backup verification. Save the source generation IDs and starting
   UTC. Use a staging cell with the normal backup endpoint and monitoring.
2. Stop the source API. Preserve its disk for comparison; perform disk deletion
   only on the designated disposable staging cell. Keep tenant encryption keys
   on their independent durable mount. Record the lease expiry plus skew.
3. Prepare a replacement cell with the correct binary/schema, control-plane and
   object-store access, independent key mount and empty `TENANT_DATA_DIR`.
4. Run the recovery command before starting its API against that directory:

   ```sh
   tenant_recover --lost-cell OLD_CELL_UUID --list
   tenant_recover --lost-cell OLD_CELL_UUID --target-cell NEW_CELL_UUID
   ```

   Set `CONTROL_DATABASE_URL`, `TENANT_DATA_DIR`, `NATS_URL`,
   `REPLICATION_BUCKET`, `REPLICATION_KEY_DIR`, `REPLICATION_ENDPOINT` and the
   normal S3 credentials. The binary requires the `duckdb-analytics` feature for
   native analytics. Use `--operations-only` if analytics is intentionally
   deferred; record that limitation in the report. Recovery waits for normal
   lease expiry/skew and resumes durable jobs on retry.
5. Verify every listed tenant reaches operational readiness, accepts an
   authenticated write and preserves uploaded outbox/business facts. Record
   `tenant_recovery_jobs.operations_ready_at` and `analytics_ready_at`; measure
   full-cell elapsed time from the declared outage start until the last tenant
   becomes operational, then until the last analytics rebuild finishes.
6. Compare recovered facts with the independent acknowledgement ledger. Report
   loss since each last verified capture; a backup outage may exceed the normal
   approximately two-minute recovery point. Run a point-in-time restore against
   a retained 30-day-old chain and compare expected dated source facts.
7. Publish the report with all tenants, failures/retries, loss window, operational
   and analytics recovery times. Keep the full-cell gate pending if any tenant
   fails or its operational recovery exceeds the published RTO.

## Simulated object-store outage

Block only the staging cell's backup endpoint or use a fault proxy, leaving
control-plane lease renewal running. Generate operational/outbox traffic. Verify
writes continue, source spool remains intact, and `/metrics` reports backlog age
and failures. Wait for monitoring to evaluate the actual rules (age over 150
seconds for another 30 seconds; failure increase for one minute). Save firing
alerts with cell labels. Restore connectivity, verify conditional uploads/readback
resume, and verify backlog drains without deleting unverified spool. A rule file
or unit test alone does not satisfy this staging alert-firing gate.

## Failure handling

For checksum, authentication, incomplete-chain or SQLite integrity errors,
preserve the drill record and source generation metadata. Inspect backup/key
configuration and replication alerts. Never reset capture counters, overwrite
immutable backup objects, remove unverified spool or force-steal an ownership
lease. Stale ownership is a failed drill for inspection, not authorization to
reassign a tenant. An interrupted full recovery is resumed through
`tenant_recover`; ordinary opens reject quarantined images until activation.
