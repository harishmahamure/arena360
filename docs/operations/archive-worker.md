# Monthly archive worker

## Request and inspect

Build the owning cell with `duckdb-analytics`, configure its replication object store and existing tenant keys, and apply control and tenant migrations. The native cell automatically processes queued jobs. Use the control database credentials for the operator command:

```sh
cargo run --manifest-path apps/backend/Cargo.toml --features duckdb-analytics --bin archive_month -- --tenant TENANT_UUID --month 2024-01-01 --p99-ms 25 --batch-rows 100
cargo run --manifest-path apps/backend/Cargo.toml --features duckdb-analytics --bin archive_month -- --status JOB_UUID
cargo run --manifest-path apps/backend/Cargo.toml --features duckdb-analytics --bin archive_month -- --replan JOB_UUID
```

Choose the p99 budget from the venue's operational target. Only complete tenant-calendar months before the current month minus 18 months are eligible. A frozen UTC cutoff keeps sessions and shifts ending inside the hot window live, even when they started in the requested old month. Open or unfinished operational work and dimensions stay live.

## Verification and purge

The worker captures a private, consistent SQLite copy using `VACUUM INTO`, exports lossless fact rows to encrypted Parquet, reads each upload back, and compares its rows with a fresh SQLite snapshot before marking the archive VERIFIED. It never runs `VACUUM` on the live database. The manifest records columns, source watermark, tenant calendar, schema, row counts, plaintext/encrypted checksums and immutable object keys.

Purge runs at background priority in bounded transactions, comparing every current row with its archived payload and checking incoming foreign keys before deletion. A correction or reference blocks deletion. If foreground writer p99 exceeds the job budget, the worker halves the batch (minimum one row), pauses for 30 seconds, and retries. The last 60 seconds of foreground samples (at most 1,024 writes) measure writer admission and transaction time; this is not an HTTP latency benchmark.

Each delete and local checkpoint commit together in SQLite. Control-plane progress is mirrored afterward. If that update fails, retry resumes from the replicated local checkpoint. Ownership and manifest locks fence each batch. Verified archives can continue after a tenant moves; a moved unverified source requires replan.

Replan creates a new revision and preserves older verified objects, including rows already purged under those revisions. Historical readers must retain those revisions. Never remove archive objects because a revision is superseded. Missing keys and corrupt or missing objects fail closed.

## Monitoring and validation

Inspect `archive_manifests.state`, `rows_purged`, `batch_rows`, `retry_after`, and `last_error`. Historical metrics expose `archive_export`, `archive_verify`, and `archive_purge`; `ArchiveWorkerFailing` alerts on repeated failures. Purge latency pauses are recorded in the job error.

The `archive_worker` integration gate needs an isolated control database and native DuckDB. It covers exact monetary payloads, verification-before-purge, absent keys, live corrections, incoming references, latency backoff, interrupted control updates, checkpoint recovery, overlapping sessions/shifts and live writes during purge. Local timings do not establish production capacity or staging recovery targets.
