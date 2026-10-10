# Weighted cell rebalancing

The command consumes measured tenant workloads and `cells.capacity_weights`.
It never derives capacity from a tenant count. Every included cell must have a
benchmark profile, and every assigned tenant must have a measurement in exactly
the same resource dimensions. Tenant measurements expire after five minutes.

## Capacity and workload inputs

Store benchmark results in each cell's `capacity_weights` JSON object:

```json
{
  "hardware_profile": "YOUR_MEASURED_HARDWARE_PROFILE",
  "benchmark_id": "YOUR_BENCHMARK_RESULT_ID",
  "measured_at": "2026-10-08T12:00:00Z",
  "headroom": 0.8,
  "limits": {
    "write_tps": 100,
    "disk_bytes": 100000000000
  }
}
```

These are format examples, not production capacity measurements. `headroom`
is the usable fraction of each measured limit; 0.8 reserves 20%. Use at least
two resource dimensions. Include the limiting resources for the hardware:
write TPS, read QPS, SQLite/DuckDB bytes, WAL bytes per second, resident memory,
analytics/background CPU demand, available disk and measured storage throughput.
Measure latency as a benchmark acceptance condition; do not sum tenant latency
percentiles as a resource demand.

The workload file contains additive demand in matching units:

```json
{
  "measured_at": "2026-10-08T12:00:00Z",
  "cells": [
    {
      "cell_id": "00000000-0000-0000-0000-000000000002",
      "resources": { "write_tps": 0, "disk_bytes": 1000000000 }
    }
  ],
  "tenants": [
    {
      "tenant_id": "00000000-0000-0000-0000-000000000001",
      "resources": { "write_tps": 20, "disk_bytes": 5000000000 }
    }
  ]
}
```

`cells` must cover every cell exactly once with measured demand outside its
currently assigned tenant workloads: retained old copies, temporary files,
other services, and baseline resource use. Include incoming staging if it is
already present; keeping the full incoming reservation is conservative.
Storage dimensions (`disk_bytes`, `sqlite_bytes`, `duckdb_bytes`, `wal_bytes`,
`spool_bytes`, and `temporary_bytes`) remain charged to the source during
planned placement because old copies are retained.

Missing/duplicate tenants, mismatched dimensions, negative/nonfinite demand,
invalid budgets, and future timestamps fail validation. The measurement file
must include assigned tenants in provisioning, fenced or moving states too;
those tenants consume capacity but are not eligible for a new planned move.

## Commands

```sh
cargo run -p arena360-tools --bin rebalance_cells -- \
  --measurements workloads.json --max-moves 10
cargo run -p arena360-tools --bin rebalance_cells -- \
  --measurements workloads.json --drain CELL_UUID --max-moves 10 --apply
cargo run -p arena360-tools --bin rebalance_cells -- \
  --decommission CELL_UUID
```

Commands use `CONTROL_DATABASE_URL`. Preview is the default. `--apply` reruns
placement against locked cell budgets and tenant ownership, then atomically
enqueues explicit moves. `--drain` marks that source `DRAINING` only on apply;
it stops new assignments while existing leases continue serving tenants.
Move execution follows [the tenant move runbook](tenant-moves.md).

The highest normalized resource utilization determines cell pressure. Targets
must remain inside every usable budget. Pre-handoff copies reserve target
capacity while their source still consumes resources. Draining cells receive
no new tenants. Overloaded cells move eligible tenants only when target
placement improves utilization. Stable UUID tie breaking makes previews
repeatable for unchanged inputs.

Inspect `remaining_pressure`, pending moves, and refreshed measurements before
running another batch. A reservation does not release source disk space:
source files remain for seven days. Empty draining cells may be marked
`OFFLINE`; owned tenants, live leases or pending incoming moves block this.
After that control transition, the operator can stop the cell service and
decommission its hardware.

## Evidence

Tests use synthetic resource budgets to validate multi-resource and heterogeneous
placement, missing/stale input rejection, preview behavior, concurrent apply
reservations, draining, rollback and empty-cell decommissioning. Actual hardware
capacity remains the separate TEST-0050 benchmark gate.
