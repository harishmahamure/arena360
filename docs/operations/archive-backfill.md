# SQLite archive backfill

## Request and inspect

Build the owning backend with `duckdb-analytics`, apply control/tenant migrations, and configure the replication object store and tenant keys. Native cells process queued backfills at P10. Use the operator command:

```sh
archive_backfill --archive ARCHIVE_UUID --p99-ms 25 --batch-rows 100
archive_backfill --status BACKFILL_UUID
```

Wake COLD tenants first. The target must be ACTIVE on the current schema with a fresh ownership lease. Finish outstanding archive/backfill jobs before requesting another one. New archiving is blocked while a backfill is pending. Choose the latency budget from the venue's operational target.

The request freezes all verified raw archive revisions for the selected month through the requested revision, including superseded revisions. For each table and ID the newest archived row wins; rows purged under an earlier revision remain available. Soft-deleted rows are restored with their deletion state. Archive objects are retained.

## Staging and validation

The worker decrypts and checks every source object and row before constructing a private disk-backed staging index. It copies current SQLite into a private validation database and inserts historical rows in foreign-key order. The live database is unchanged during this phase. INTEGER/TEXT values remain lossless. Additive nullable/defaulted fields use SQLite's target defaults. Removed or changed columns require an explicit conversion and stop the job; no fields are silently discarded.

Foreign keys and SQLite integrity checks must pass. The worker publishes encrypted, read-back-verified normalized staging Parquet, freezes its checksum and expected row count, then enters BACKFILLING. Remote staging permits recovery after local scratch loss or an ownership move. Old staging objects may be orphaned by an interrupted validation attempt; retain published objects while their job is active and manage unpublished or terminal objects with the deployment's retention policy.

## Bounded application and recovery

Each transaction admits the current owner and schema, compares existing IDs for exact equality, inserts missing rows, emits canonical analytics outbox events, and records a local checkpoint in the same SQLite transaction. A differing live row stops the job without overwriting it. Control-plane progress is mirrored after the SQLite commit. If that mirror fails, the next attempt uses the replicated local checkpoint and does not emit duplicate events for the committed batch. A local checkpoint behind the control plane requires WAL recovery before proceeding.

Batches are limited to 1–1,000 rows. Foreground writer p99 over the job budget halves the batch size, pauses for thirty seconds and retries. P10 admission also pauses under disk pressure. This measurement covers writer admission and transaction time, not HTTP latency. Backfill does not recalculate wallet or inventory balances and does not VACUUM the live database.

After all batches, bounded reads compare every restored row with validated staging. A retained SQLite connection detects concurrent commits before final writer admission; a changed dataset retries verification. A local completion marker precedes control-plane COMPLETE. Successful completion removes private scratch.

Inspect `historical_backfills.state`, `rows_processed`, `expected_rows`, `checkpoint`, `batch_rows`, `retry_after` and `last_error`. `archive_backfill` historical metrics track execution success and duration; `ArchiveWorkerFailing` also covers repeated backfill failures. Schema/data conflicts remain retryable with their error recorded; correct the underlying incompatibility before allowing retry. The manifest phases cannot be reset backward.

## Verified local gate

The native `archive_backfill` gate restores exact scaled money across two archive revisions, handles an additive nullable field, rejects missing keys and stale leases, queues under disk pressure, pauses on foreground p99, recreates lost scratch from remote staging, and recovers a failed control-plane mirror without duplicate events. It also refuses changed live rows. Archive purge, manifest state guards and UTC regressions pass. Local fixtures do not establish production throughput.
