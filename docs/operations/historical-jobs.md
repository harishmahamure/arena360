# Historical job manifests

Migration `0018_historical_jobs.sql` stores archive, backfill, and export jobs in the PostgreSQL control plane. These tables record durable progress; creating a row alone does not run a worker. Subsequent M11 workers own execution, object verification, ownership checks, and admission limits.

## Archive

`archive_manifests` identifies one tenant calendar month, source cell and ownership generation, source schema, event watermark, row count, object keys/checksums/sizes, and an aggregate checksum.

The phases are `PLANNED → EXPORTING → UPLOADED → VERIFIED → PURGING → COMPLETE`. Workers persist errors while remaining in their retryable current phase. The database rejects skipped/reversed phases. Uploaded objects must reside under `tenants/{tenant}/archive/{yyyy}/{mm}/`, and their row totals must match the manifest. Verification time is required before purge. Verified source evidence cannot change. Purge counters cannot decrease or exceed the source count; completion requires all manifest rows to have been accounted for.

Verification is performed by the archive worker against SQLite and the uploaded objects. A database state or checksum string alone does not prove object contents. Do not set VERIFIED manually to bypass the worker.

## Backfill

`historical_backfills` references an archive using a composite tenant/archive foreign key. New jobs require a VERIFIED, PURGING, or COMPLETE source, preventing cross-tenant source references and unverified imports.

Phases are `PLANNED → DOWNLOADING → TRANSFORMING → VALIDATING → BACKFILLING → VERIFYING → COMPLETE`. Store per-table checkpoints, processed/failed counts, staging objects, the target schema, validation checksum, and timestamps. Validation evidence freezes before operational batches begin. Counters cannot regress. Completion requires no failed rows.

## Export

`historical_exports` records the requester, exact half-open UTC date range, filters, format (`CSV`, `CSV_GZ`, or `PARQUET`), source objects, worker reservation token/expiry, result metadata, and download expiry.

Phases are `QUEUED → PREPARING → SCANNING_ARCHIVE → GENERATING → UPLOADING → READY`. Active phases can become FAILED or CANCELLED; READY can become EXPIRED. Terminal jobs cannot be restarted or have their result evidence changed. Retry a failed export by creating a new job. READY requires a checksummed result under `tenants/{tenant}/exports/{export}/`.

The export worker must validate authorization and filters, reserve concurrency across cell/tenant/platform, verify downloaded inputs, and only offer downloads before expiry. Manifests do not store a permanent signed URL.

## Verification

`historical_manifests` is an integration test against an isolated control database. It checks complete phase sequences, rejected skips and reversals, missing verification, immutable evidence, monotonic checkpoints, cross-tenant references/object paths, and terminal export protection.
