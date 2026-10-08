# Planned tenant moves

Both cells must run the backend with replication enabled, access to the same
control database and backup bucket, and the tenant encryption key available.
The target must run the same tenant schema version as the source. Keep target
staging on the tenant filesystem: installation uses a hard link.

## Commands

```sh
cargo run --manifest-path apps/backend/Cargo.toml --bin tenant_move -- \
  --tenant TENANT_UUID --target-cell CELL_UUID
cargo run --manifest-path apps/backend/Cargo.toml --bin tenant_move -- \
  --status MOVE_UUID
cargo run --manifest-path apps/backend/Cargo.toml --bin tenant_move -- \
  --cancel MOVE_UUID
```

Commands use `CONTROL_DATABASE_URL`. Cancellation is allowed in
`PREPARING_MOVE` and `COPYING`; a cutover must finish or time out first.

## Execution and failures

The source takes a verified physical snapshot and publishes pending captures.
The target restores privately and advances that image as captures arrive.
Foreground writes continue during this pre-copy. Readiness is refreshed without
repeatedly scanning an unchanged image; final cutover rechecks its hash and
SQLite integrity.

The source then holds the writer gate, uploads the final committed capture,
and waits for target installation. A five-second asynchronous deadline aborts
an incomplete cutover and restores source writes. Synchronous file and SQLite
work cannot be preempted; inspect `write_gate_milliseconds` and validate with
representative tenant sizes before draining a production cell.

Ownership generation, lease, routing notification, and move phase switch in one
PostgreSQL transaction. The old writer is fenced. The target image stays
quarantined until integrity verification and completion. Target restart retries
activation, including a committed completion whose local marker remains.
After ownership has transferred, recovery uses the target generation; never
manually transfer ownership back or delete the target quarantine marker.

Source restart before handoff resumes only its exact existing lease generation.
It resets an interrupted cutover to pre-copy. Cell-loss recovery supersedes
unfinished moves. An unavailable target or stale receipt cannot transfer
ownership. Existing WebSocket sessions recheck ownership and reconnect through
the current route; requests and writes remain lease fenced.

Completed moves retain the source copy for seven days. The cell reconciler
then removes it under the tenant control lock, after checking that ownership
has moved away and no move back to the cell is pending. A newer departure
supersedes an older cleanup deadline. Copies displaced by a move back use
`move-retained-TENANT_UUID-MOVE_UUID` and expire with that move. Cancelled
pre-copy staging and its matching unpublished quarantine are also removed.

## Local verification

The isolated PostgreSQL integration test uses separate cell directories and
lease clients, encrypted object-store backups, and one background slot per
cell. It verifies writes after pre-copy, final catch-up, atomic routing, source
fencing, target quarantine, activation retry, and timeout without a target
acknowledgement. It exercises a fresh manager after a committed completion
with a leftover marker, schema mismatch, stale readiness, and retention cleanup.
This is local functional evidence; staging traffic, production
hardware, and production-size write-gate measurements remain launch checks.
