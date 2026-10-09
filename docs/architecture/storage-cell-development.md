# Storage-cell development (M7)

Operational APIs use one SQLite file per tenant. PostgreSQL stores control-plane
metadata and global staff credentials. The shared operational PostgreSQL schema,
its migrations, the old analytics writer, and direct PostgreSQL wallet imports are
retired. Reports read the owning tenant’s DuckDB file; unready or disabled native
analytics returns `503 ANALYTICS_UNAVAILABLE` with `report temporarily rebuilding`.

## Local setup

Start PostgreSQL and optional Redis using Compose. The `postgres-init` service creates
`arena360_control` if it is missing, including when reusing an existing PostgreSQL
volume. It leaves other databases in that volume untouched.

Configure `apps/backend/.env`:

```dotenv
CONTROL_DATABASE_URL=postgres://arena360:arena360@localhost:5432/arena360_control
ARENA_ROLES=control,cell,router
TENANT_DATA_DIR=data/tenants
JWT_SECRET=replace-with-a-local-secret-at-least-32-characters
LEGACY_REST_ENABLED=true
```

Apply the control schema with `pnpm migration run --target control` (SQLx CLI), or
start the backend once; startup embeds and applies control migrations. Register a
cell in that control database, then set `ARENA_CELL_ID` to its returned UUID:

```sql
INSERT INTO cells(name,address)
VALUES('local-cell','http://127.0.0.1:3000') RETURNING id;
```

Restart the backend after selecting its cell ID. Tenant ownership is acquired only
by provisioning/recovery orchestration; normal requests never acquire or steal leases.
Use `pnpm demo:seed` to provision and populate the tenant. See the
[demo guide](analytics.md#demo-data) for dates, retry behavior and player login.

For a fresh local control database, an operator can be bootstrapped with its installed
`pgcrypto` extension; replace the local password placeholder before running:

```sql
INSERT INTO users(username,password_hash,first_name)
VALUES('local.operator',crypt('replace-with-local-demo-password',gen_salt('bf',10)),'Local')
RETURNING id;
```

Pass that ID via `DEMO_OWNER_USER_ID` when seeding to create administrator membership
in the demo tenant. Existing operators keep their credentials. Default generated demo
owner/counter accounts remain login-disabled. `DEMO_PLAYER_PASSWORD` enables local
player login for interactive kiosk checks.

## Schema changes and verification

```bash
pnpm migration generate add_control_metadata --target control
pnpm migration generate add_tenant_metadata --target tenant
pnpm backend:test
pnpm backend:test:integration
```

Migration generation uses the next sequential version in the chosen directory.
Control execution requires `CONTROL_DATABASE_URL`. Tenant migrations run through the
owning-cell migration orchestrator with lease, snapshot and resumability gates;
PostgreSQL CLI commands cannot target tenant files.

The integration runner creates a disposable control database and temporary tenant
root, enables all control-backed integration tests, and removes its resources on
completion or failure. It can start local PostgreSQL using `pg_config`/`PG_BINDIR`, or
use an explicit `CONTROL_TEST_ADMIN_DATABASE_URL`. The application database URL is
never a test target. Native report parity runs in the normal suite. Control and JetStream gates use disposable infrastructure.

The kiosk HTTP smoke script accepts `KIOSK_VENUE_ID` for explicit device provisioning;
use a demo player whose seeded floor session is not already active.

## Single-cell deployment

The backend Helm chart defaults to one instance, a retained ReadWriteOnce tenant
volume, and a `Recreate` deployment strategy. Cell roles reject multiple replicas,
autoscaling, ephemeral storage and a tenant path that differs from the mounted path.
Stateless router/control roles can use separate replicated deployments.

The deployment workflow requires `CONTROL_DATABASE_URL` and `ARENA_CELL_ID` secrets.
Its optional migration step uses `CONTROL_MIGRATION_DATABASE_URL` and applies only
`migrations/control`. The cell ID must already be registered with its reachable
address in that control database. The chart mounts tenant files at
`/var/lib/arena360/tenants`; an existing volume can be selected with
`persistence.existingClaim`. Uninstalling the chart retains a chart-created tenant PVC.

Replication, cell-loss recovery and backup drills are M8 work; distinct-cell
orchestration and rebalancing are M9 work. Docker builds embed both active migration
families and include the API and demo seed binaries.
## Tenant outbox publishing

Owning cells publish canonical event envelopes in sequence order every 250 ms when
`NATS_URL` is configured. Startup and operational commits do not wait for NATS.
The publisher expects stream `ARENA_TENANT_EVENTS` on
`arena.tenant.*.events.v1`; stream provisioning is OPS-0020. Missing or unavailable
streams retain SQLite rows. Message IDs combine tenant ID and event ID for server
deduplication; consumers must also deduplicate by per-tenant sequence.

Tenant migration 0014 persists the acknowledged sequence. Acknowledged source rows
remain until the durable realtime projection cursor covers them. SQLite cleanup
uses the current ownership lease and an immediate transaction. Network requests
never hold the SQLite writer. Lost acknowledgements and checkpoint failures replay
stable events; a sequence gap fails closed.

`/metrics` exposes retained event count, estimated envelope/payload bytes, oldest
retained event age, publication acknowledgement count and publication failures.
Counts aggregate locally owned tenants; they include acknowledged rows waiting for
realtime. Reopening a pending tenant requires an existing valid lease. Empty polls
allow idle eviction. Publishers do not provision tenants or acquire ownership.

### Provision the replay stream

Start the Compose NATS service, then run:

```sh
NATS_URL=nats://127.0.0.1:4222 pnpm analytics:stream:init
NATS_URL=nats://127.0.0.1:4222 pnpm analytics:stream:init -- --check
```

The native `tenant_events_setup` command creates `ARENA_TENANT_EVENTS` on
`arena.tenant.*.events.v1` with file storage, limits retention of seven days,
unlimited message/byte counts and a two-minute message-ID deduplication window.
Consumer acknowledgements retain replay history. New streams prohibit individual
message deletion and purging. Repeating setup verifies the existing stream without
changing its configuration or messages; incompatible retention, storage, subjects,
count/byte limits, replica settings, sealing or disabled acknowledgements fail.
Reconcile incompatible streams explicitly before publication.

`NATS_STREAM_REPLICAS` defaults to 1 for the standalone development server; use
3 or 5 only with a suitably sized NATS cluster. Production deployment requires
`NATS_URL`; provision the stream with the native command in the backend image from
a network that can reach NATS. Setup is separate from API startup, so a NATS outage
does not prevent operational startup. The publisher retains failed publications.

The integration runner starts and removes a fresh, empty JetStream server per gate
when `nats-server` is installed or `NATS_SERVER_BIN` points to it. CI uses a pinned,
checksum-verified server runtime. Without a local runtime the JetStream gates
are explicitly reported as skipped; the SQLite/control checks still run.

Run only the disposable JetStream checks with `pnpm backend:test:integration --jetstream-only`; this mode requires a local server runtime and does not create a control database.

### DuckDB development builds

DuckDB's Rust dependency is pinned to the native 1.5.6 release family. Build with
`--features duckdb-bundled` to compile its bundled source. For faster local builds,
use the official matching shared library and `--features duckdb-analytics`:

```sh
DUCKDB_LIB_DIR=/path/to/duckdb-1.5.6 pnpm backend:test:integration
```

The runner enables the feature and assigns the native loader path after Cargo
launches each test. For direct Cargo runs, use a target runner that assigns
`DYLD_LIBRARY_PATH` on macOS or `LD_LIBRARY_PATH` on Linux; shell launchers and
Cargo can filter externally supplied loader paths. Supply a matching
native library, not a different DuckDB version. No system library is installed by
the repository commands. The two feature choices use the same Rust implementation
and schema; bundled builds require a cached target directory for practical rebuilds.


### Analytics ingestion and rebuilds

Run owning cells with `duckdb-analytics` (matching native SDK) or `duckdb-bundled`
and explicit `NATS_URL`. Production images compile analytics in, download the
checksum-pinned native 1.5.6 SDK for their architecture, and install the matching
signature-verified SQLite extension during image construction. Runtime rebuilds
use the cached extension at `DUCKDB_EXTENSION_DIR`; unsigned extensions stay disabled.
Without that setting, the extension cache is inside the tenant directory and the
first rebuild requires HTTPS access to the official DuckDB extension repository.

Fresh analytics start REBUILDING. The tenant worker takes a private `VACUUM INTO`
snapshot through a read-only SQLite connection, records its persistent outbox
watermark, and attaches that snapshot read-only. It builds a shadow DuckDB file,
projects explicit columns with exact money, derives tenant calendar labels in
Rust, builds closed session hours and monthly aggregates, and replays retained
events after T0 to a finite post-backfill source watermark. Its replay consumer
starts after a broker position recorded before the SQLite snapshot, so earlier
stream history is covered by the snapshot rather than scanned again. Schema v2
persists that broker position; live consumers skip and acknowledge earlier
deliveries even after a restart, including writes lost from restored SQLite. Operational writes
continue while this happens. Live ingestion resumes after the canonical file
switch; reports remain unavailable until M7.

Failures leave the canonical state REBUILDING and preserve its facts. Rebuild
consumers have independent cursors and do not remove stream history. Temporary
snapshot/shadow files are removed when the job ends. Corrupt or outdated derived
files are quarantined for inspection before an initial rebuild; a newer schema
requires upgrading the binary. A DuckDB failure closes the worker for recovery;
failed opens and background polling do not keep an idle tenant open. Existing older monthly aggregates survive a normal
rebuild in the same timezone. A lost/corrupt analytics file reconstructs the hot
window from SQLite; historical raw facts are not pulled into the hot database.

`pnpm backend:test:integration --jetstream-only` includes the live consumer and
rebuild gates when `DUCKDB_LIB_DIR` is supplied. The runner creates isolated
JetStream storage and removes it after each target. Optional extension cache:
`DUCKDB_EXTENSION_DIR=/path/to/matching/signed/extensions`.

The native demo-seed control gate also rebuilds DuckDB when the SDK and disposable
NATS runtime are available. It compares all 27 projection row counts, every
scale-4 money-column total and closed session occupied seconds with SQLite.
