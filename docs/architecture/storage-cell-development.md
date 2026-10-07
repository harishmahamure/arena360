# Storage-cell development (M5)

Operational APIs use one SQLite file per tenant. PostgreSQL stores control-plane
metadata and global staff credentials. The shared operational PostgreSQL schema,
its migrations, the old analytics writer, and direct PostgreSQL wallet imports are
retired. ClickHouse report definitions remain only for M7 parity; live reports return
`503 ANALYTICS_UNAVAILABLE` until tenant DuckDB reporting is ready.

## Local setup

Start PostgreSQL and optional Redis using Compose. A new Compose database is named
`arena360_control`. An existing PostgreSQL volume is not reset by changing Compose;
create a separate empty control database when reusing a local volume.

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
never a test target. Five optional external ClickHouse/Redis gates remain separate.

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
checksum-verified server runtime. Without a local runtime the two JetStream gates
are explicitly reported as skipped; the SQLite/control checks still run.

Run only the two disposable JetStream checks with `pnpm backend:test:integration --jetstream-only`; this mode requires a local server runtime and does not create a control database.
