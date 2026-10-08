# Historical exports

## Request and download

Organization administrators with a current control-panel session can create jobs with `POST /historical/exports`. These routes also work when legacy REST is disabled. The request uses UTC timestamps ending in `Z`, an exclusive end, format `CSV`, `CSV_GZ`, or `PARQUET`, and filters containing a canonical fact `table`, optional supported `locationId`, and optional `dailyRevenue` for transactions. Date ranges are limited to twenty years.

Inspect `GET /historical/exports/{job}`, cancel with `DELETE /historical/exports/{job}`, and request `GET /historical/exports/{job}/download` after READY. The link expires within ten minutes and the result within seven days. Download signatures bind tenant, job, expiry and plaintext checksum. Revoking the requesting administrator invalidates previously issued links. COLD tenants can export without hydration.

The worker reads all verified archive revisions and available hot months. It deduplicates corrected records, applies archived tombstones and retains rows purged under older revisions. Missing coverage, keys, checksums or canonical projections fail closed. Legacy raw-only archives require backfill followed by a new archive revision before typed exports are available. Monetary columns remain exact decimal values; CSV compression does not change the dataset.

## Isolated worker processes

Build `export_worker` with `duckdb-analytics`. Configure `CONTROL_DATABASE_URL`, `CELL_ID`, an absolute `EXPORT_STAGING_DIR`, the replication object store credentials, and `REPLICATION_KEY_DIR`. Run `export_worker` continuously, or use `--once` for one admission pass. Each admitted job runs in a separate low-priority child process with its own DuckDB connection, 128 MiB memory budget and one thread. Do not start jobs in the API process.

`historical_export_limits` controls platform, tenant and cell concurrency; defaults are 4, 1 and 2. Admission is serialized in PostgreSQL. Worker tokens expire after two minutes and renew every twenty seconds with a fifteen-second timeout. Losing the token stops the process. Cancelled or failed workers retain their reserved slots until their leases expire. Failed processing records an error and retries after thirty seconds; completed results cannot be replaced.

For Helm, enable `exportWorkers.enabled`, configure persistent tenant storage, the owning cell and config map, and a separate `replicationKeys.existingClaim`. `REPLICATION_KEY_DIR` must match `replicationKeys.mountPath`. The export sidecar mounts keys read-only and has independent CPU/memory limits. Its staging path must be under the persistent tenant volume. The backend image includes the operator binaries. Helm lint and enabled/default rendering are verified; the image was not built as part of the local export gate.

## Storage and monitoring

Results are encrypted before upload and read back before READY publication. Large objects use bounded multipart uploads. Configure the bucket lifecycle to abort abandoned multipart uploads after an appropriate operational interval; a terminated process cannot guarantee an abort request. Never remove retained archive revisions as superseded data may still be authoritative for purged rows.

Downloads verify and decrypt into private staging before streaming with bounded buffers. Two downloads per API process are admitted at once; disk pressure can refuse new downloads or worker jobs. Worker scratch directories have restrictive permissions and an ownership marker. Cleanup removes only marked, stale directories after checking the current token and lease; active jobs are protected.

Inspect `historical_exports.state`, `worker_cell`, `worker_expires_at`, `retry_after`, `last_error`, `row_count` and `result_size_bytes`. Monitor the separate worker process logs and queue age; API-process in-memory counters do not aggregate child process failures. Seven-day-expired unfinished jobs become FAILED, and expired READY results become EXPIRED. Object retention and orphan cleanup should follow the deployment's bucket policy.

The native `historical_exports` gate runs real child processes against PostgreSQL and a local object store. It checks all three output formats, exact totals spanning archived/hot data, correction/tombstone handling, COLD access, concurrency, token fencing, staging cleanup, signed HTTP downloads and administrator revocation. The multipart transport test uses the object-store memory implementation. These local results do not establish Wasabi availability or production throughput.
