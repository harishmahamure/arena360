# Cold tenants

Cells require the control database, verified replication storage, and the tenant's existing encryption key. Routers wake cold tenants automatically. Only cells advertising a fresh hydration heartbeat are eligible; selection uses assigned tenant count. Capacity measurements remain a separate deployment gate.

## Request and inspect

```sh
cargo run --manifest-path apps/backend/Cargo.toml --bin tenant_cold -- --tenant UUID
cargo run --manifest-path apps/backend/Cargo.toml --bin tenant_cold -- --status JOB_UUID
cargo run --manifest-path apps/backend/Cargo.toml --bin tenant_cold -- --cancel JOB_UUID
```

These commands use `CONTROL_DATABASE_URL`. The default idle threshold is 24 hours; `--idle-seconds` accepts 0–31536000. Zero is useful for a disposable fixture. Cooling is operator initiated, requires the current tenant schema and a fresh source lease, and rejects pending moves or migrations. Open sessions, active shifts, and recent outbox activity prevent cooling. Without events, creation time is the idle boundary. Cancellation is allowed only before lease release.

## Durable transitions

1. `SNAPSHOTTING`: the source stays ACTIVE while preparing a verified snapshot. Under the final writer gate, it rechecks inactivity, durably captures and verifies remaining WAL, and records the snapshot and capture boundary.
2. `RELEASED`: source writers are fenced; one control transaction deletes the lease, sets COLD with no owner, and records the transition. A durable local marker identifies exactly which copy can be removed. Cleanup closes SQLite and analytics handles, atomically renames the directory, then removes it. Interrupted cleanup retries safely. The encryption key remains available.
3. `HYDRATING`: the first router request selects a ready ACTIVE cell after source cleanup, acquires a new ownership generation, and waits for that cell's recovery agent. Concurrent requests share the same assignment. The existing recovery workflow downloads and verifies snapshot/WAL, installs the private image, and produces a verified baseline before activation.
4. `ACTIVE`: operations become routable. Analytics rebuild separately through the recovery queue. `hydration_milliseconds` records assignment to operations readiness; it excludes time waiting for source cleanup and is a local measurement, not a production SLA.

A request that exceeds the router's 60-second wait receives `TENANT_HYDRATION_PENDING` (503). The durable job continues; retry the request. Failures retain the control state and error rather than exposing an unverified image. `last_error` and recovery job details identify storage, key, integrity, or lease failures. Restore the dependency and let the agent retry. Never manually remove the key or override ownership to bypass recovery.

The integration fixture exercises a real SQLite cold/warm round trip with an in-memory object store, missing-key failure, concurrent first requests, stale lease fencing, and a single background slot. Run through `pnpm backend:test:integration` or `cargo test --manifest-path apps/backend/Cargo.toml --test cold_tenants -- --include-ignored --nocapture` against an isolated control database.
