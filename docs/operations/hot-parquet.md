# Monthly hot Parquet

Native analytics cells with replication and `NATS_URL` run a separate durable JetStream consumer named `hot_{tenant UUID without hyphens}`. It ACKs events only after the SQLite source watermark is covered by verified monthly manifests. Existing SQLite data is backfilled on first use, and changed ownership, restored watermarks, timezone changes, or a daily maintenance deadline trigger a rebuild without requiring an event.

## Dataset

`hot_month_manifests` stores the current verified object list for each tenant calendar month. The rolling window includes the month 18 months before the current month through the current month. Objects live under `tenants/{tenant}/hot/{yyyy}/{mm}/` with immutable UUID filenames. They are encrypted envelopes containing ZSTD Parquet, using the tenant's durable data key. Downloads verify encrypted and plaintext sizes/checksums and authenticated envelope completion before reading Parquet.

Rows use the explicit, secret-free analytics projection shared by M6 ingestion and rebuilds. Money remains DECIMAL(19,4), timestamps remain UTC, and month boundaries use the tenant timezone. Child facts follow their parent's month. Current dimension rows are copied into each month; they do not represent historical dimension versions. Export readers must use the manifest object lists, rather than unioning every file found under a prefix.

## Publication

The writer takes a consistent SQLite snapshot using a background reader, converts one table at a time to Parquet, and uploads and reads back new objects. DuckDB uses one thread and a 128 MB memory limit, with spill files in private staging. A cell-wide background scheduler admits hot backfill; pull buffers have a separate four-slot limit and an 8 MiB batch ceiling. Network operations do not hold SQLite's writer.

Canonical source row checksums allow unchanged tables to reuse previously verified objects after checking their remote presence and size; missing objects are rebuilt. Background admission is released between tables so queued backups can run. Monthly manifests publish under the current control-plane ownership lock. The all-month source cursor advances only after every month succeeds. A verified archive month prevents later hot publication for that month. Failed or interrupted work retries from SQLite; partial uploads never become an authoritative archive or authorize a purge.

Superseded files can remain under the prefix until archive handoff cleanup. Only current manifest objects are eligible for exports. The live SQLite database remains authoritative; `hot/` is rebuildable.

## Diagnose

Inspect `hot_tenant_state` for source watermark, ownership generation, timezone, and maintenance deadline; inspect `hot_month_manifests` for verified objects. The `Hot copy remains retryable` log records failures. Missing keys, object-store failure, a changed lease, or an incompatible broker consumer prevent publication/ACK rather than exposing unchecked files. Restore the dependency and the cell restarts its consumer.

The `hot_parquet` integration gate uses a disposable control database and JetStream server. It checks tenant-calendar boundaries, exact values, retry reuse, authenticated downloads, publication-before-ACK, and missing-key failure. Run with the native DuckDB library enabled through `pnpm backend:test:integration`.

## Metrics and alerts

`arena360_historical_job_completed_total`, `arena360_historical_job_failures_total`, `arena360_historical_job_elapsed_milliseconds_total`, and `arena360_historical_job_last_success_timestamp_seconds` use the `task` label. Publication uses `hot_parquet`; consumer setup/delivery failures use `hot_consumer`. `HotParquetPublicationFailing` in `infra/monitoring/replication-alerts.yml` alerts on repeated failures. Load the rule into the monitoring environment; actual alert firing remains part of the staging launch gate.
