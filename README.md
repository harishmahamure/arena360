# Arena360

Arena360 is an operations platform for a gaming cafe. It manages players, staff,
stations, timed play, plans, point-of-sale activity, inventory, cash controls,
and the station client used on gaming PCs.

The monorepo separates runnable apps, shared libraries, operator tools, and deployment assets:

| Surface | Technology | Purpose |
|---|---|---|
| `apps/tenant-gateway` | Rust, Axum, PostgreSQL | Stateless platform API and tenant routing |
| `apps/tenant-storage` | Rust, SQLite | Persistent tenant operations, reports, and migrations |
| `crates/sqlite-reporting` | Rust | Bounded read snapshots and ledger/calendar functions |
| `crates/backend-core` | Rust | Shared application and database implementation |
| `crates/tenant-protocol` | Protobuf, Rust | Public API and private service gRPC contracts |
| `tools/backend-cli` | Rust, Node | Operator commands, seeding, benchmarks, and integration runners |
| `apps/backend` | Rust | Combined backend compatibility launcher |
| `apps/admin` | React, Vite, MUI | Staff and administrator operations console |
| `apps/tenant` | React, Vite | Platform tenant management portal |
| `apps/kiosk` | Tauri 2, React, Rust | Locked-down Windows station client and game launcher |

Rust packages share the root Cargo workspace, dependency versions, and lockfile.
Frontend apps use the pnpm workspace. Container builds and Helm charts live in
`infra/`. Start with the [project structure and contribution guide](docs/architecture/project-structure.md).

Tenant SQLite files own operational records; PostgreSQL owns control-plane metadata
and global staff credentials. Tenant reports read the same SQLite files. Redis is optional;
the backend continues with a no-op cache if Redis is unavailable.

## Documentation

- [Project structure and commands](docs/architecture/project-structure.md)
- [Network database service architecture](docs/architecture/network-database-service.md)
- [Tenancy architecture](docs/architecture/tenancy.md)
- [Data platform implementation plan](docs/plans/data-platform-build-plan.md)
- [Storage-cell local development](docs/architecture/storage-cell-development.md)
- [Two-service deployment and operations](docs/operations/two-rust-services.md)
- [Tenant SQLite reporting](docs/architecture/analytics.md)

The documentation describes implemented behavior only. The generated OpenAPI
spec and the source code remain authoritative for endpoint-level details.

## Getting started

Prerequisites: Node 20, pnpm 9, a stable Rust toolchain, Docker, and `sqlx-cli`.

### One-time setup

```bash
corepack enable
pnpm install
docker compose up -d
cp .env.example .env
cp apps/admin/.env.example apps/admin/.env
```

Compose starts PostgreSQL, which every service needs. It also starts Redis, NATS, PgBouncer, and a local S3 stand-in. Redis is only a cache. NATS and object storage are for outbox publishing and backups. Leave them running; the apps start without them.

Put this in the repository root `.env`. `ARENA_CELL_ID` and `STORAGE_CELL_ID` are the same cell UUID:

```dotenv
CONTROL_DATABASE_URL=postgres://arena360:arena360@localhost:5432/arena360_control
JWT_SECRET=replace-with-a-local-secret-at-least-32-characters
STORAGE_SERVICE_TOKEN=replace-with-at-least-32-random-ascii-characters
TENANT_DATA_DIR=data/tenants
ARENA_CELL_ID=00000000-0000-4000-8000-000000000001
STORAGE_CELL_ID=00000000-0000-4000-8000-000000000001
STORAGE_SERVICE_URL=http://127.0.0.1:3001
```

Apply the control schema before the first request. Service startup also applies it.

```bash
pnpm migration run --target control
```

Staff admin reads `apps/admin/.env` (`VITE_API_URL=http://localhost:3000`). Copy `apps/kiosk/.env.example` to `apps/kiosk/.env` only if you run the station client.

### What to start

Run each long-running process in its own terminal. Start storage, then the gateway, then the UIs.

| Process | Command | Address | Start it for |
| --- | --- | --- | --- |
| Tenant storage | `PORT=3001 pnpm storage:dev` | `http://127.0.0.1:3001` | Tenant SQLite, reports, and provisioning. Required. |
| Tenant gateway | `PORT=3000 pnpm gateway:dev` | `http://localhost:3000` | Public API, auth, and routing to the cell. Required. |
| Staff admin | `pnpm admin:dev` | `http://localhost:5173` | Venue console. |
| Platform portal | `pnpm portal:dev` | `http://localhost:5174` | Cells, tenants, plans, and operators. |
| Kiosk | `pnpm kiosk:dev` | Vite on `http://localhost:1420`, then the Tauri window | Station client. Skip it for back-office work. |

The shell `PORT=` overrides the shared `.env`, so storage and the gateway do not both bind port 3000. The gateway ignores `ARENA_CELL_ID` and sends tenant work to `STORAGE_SERVICE_URL`. Swagger UI is at `http://localhost:3000/api/docs` outside production.

The portal proxies `/platform` to the gateway. Register the cell once, from **Cells**, using the same UUID and backend origin `http://127.0.0.1:3001`. That records the cell. It does not start the process. New tenants stay blocked until that cell is active.

Create the first portal operator before signing in. The password is 12–72 bytes and is read from stdin. First sign-in enrolls TOTP.

```bash
printf '%s\n' 'replace-with-local-password' | cargo run -p arena360-tools --bin platform_operator -- operator
```

Sign in at `http://localhost:5174/` as `operator`. Details are in the [portal guide](docs/operations/tenant-management-portal.md).

Reports read the owning tenant's SQLite file. Default builds need neither DuckDB nor NATS. See the [reporting guide](docs/architecture/analytics.md).

### One-process backend

`pnpm backend:dev` runs control, routing, and the cell together on port 3000. Use it instead of the gateway and storage pair, with those two processes stopped so port 3000 is free. Register the cell at `http://127.0.0.1:3000` and keep `ARENA_CELL_ID` set to that cell. The admin, portal, and kiosk commands stay the same.

### Demo data

After the cell is registered, seed 60 days of synthetic sales and activity:

```bash
pnpm demo:seed
```

Set `DEMO_PLATFORM_PASSWORD` when seeding to create the `demo.platform` portal operator. It must enroll TOTP on first login. Repeated completed runs return the original summary. See the [demo setup guide](docs/architecture/analytics.md#demo-data) for operator and player login options.

## Common checks

```bash
pnpm lint
pnpm typecheck
pnpm test
pnpm rust:check
pnpm backend:test
pnpm backend:test:integration
pnpm backend:clippy
```

`backend:test:integration` runs the SQLite suite and all control-plane integration tests.
It creates and removes a disposable control database and a temporary directory for tenant files. By default it starts a temporary
local PostgreSQL server (`initdb`/`pg_ctl` discovered via `pg_config`, or `PG_BINDIR`).
Alternatively, set `CONTROL_TEST_ADMIN_DATABASE_URL` to a server with database-creation
permission. Application database settings are not used as test targets. Use
`--control-only` for the gated control/ownership/staff/demo checks. Optional legacy analytics and Redis tests keep their explicit infrastructure gates.

See the [project guide](docs/architecture/project-structure.md#commands-and-configuration)
for commands and ownership, [storage-cell development](docs/architecture/storage-cell-development.md)
for the database workflow, and [two-service deployment](docs/operations/two-rust-services.md)
for images, Helm, and private gRPC settings.
