# Setup guidelines

Configure a fresh development environment here, then use the [start guide](start.md) for daily work. Run commands from the repository root.

## Prerequisites

- Node.js 20–22; the workspace requires `>=20 <23`.
- pnpm 9.15.9, as specified in `package.json`.
- Rust through rustup; `rust-toolchain.toml` selects stable with rustfmt and Clippy.
- A Docker runtime and the Docker Compose plugin. Colima can supply the runtime on macOS.
- Python 3 for the hidden password prompt below.
- The native Tauri build prerequisites for your OS if you develop the kiosk.

Check your installation:

```sh
node --version
pnpm --version
rustup show
docker version
docker compose version
```

Docker Compose is a separate requirement from the Docker CLI. If `docker compose` is missing, install/configure its plugin before the fresh infrastructure setup. Rust protobuf builds use a vendored protoc binary.

## Dependencies and infrastructure

```sh
pnpm install --frozen-lockfile
```

Start your Docker runtime, then:

```sh
docker compose up -d postgres postgres-init redis
```

Compose exposes PostgreSQL at `localhost:5432` and Redis at `localhost:6379`. Its local PostgreSQL username/password are `arena360` / `arena360`. The `postgres-init` service creates `arena360_control` if it is missing and preserves other databases in the volume.

## Backend configuration

Create the root environment file once:

```sh
cp .env.example .env
```

Set these entries in `.env`; uncomment or add the split-service entries:

```dotenv
CONTROL_DATABASE_URL=postgres://arena360:arena360@localhost:5432/arena360_control
ARENA_CELL_ID=00000000-0000-4000-8000-000000000001
STORAGE_CELL_ID=00000000-0000-4000-8000-000000000001
STORAGE_SERVICE_URL=http://localhost:3001
TENANT_DATA_DIR=data/tenants
PORT=3000
REDIS_URL=redis://127.0.0.1:6379
LEGACY_REST_ENABLED=true
JWT_SECRET=replace-with-a-generated-secret
STORAGE_SERVICE_TOKEN=replace-with-another-generated-secret
```

Generate each secret separately and paste its value into the relevant entry:

```sh
openssl rand -hex 32
```

`JWT_SECRET` must contain at least 32 characters. `STORAGE_SERVICE_TOKEN` must contain at least 32 ASCII characters without surrounding whitespace. Both services must use the same values. Keep `.env` local; it is ignored by Git.

The two cell IDs must match the worker registered in the portal. Keep an existing cell's UUID when reusing its tenant files. Storage runs with `PORT=3001`; gateway runs with `PORT=3000`. Exported process variables override `.env`. Rust startup falls back to `apps/backend/.env` if no root file can be loaded.

The fixed service roles are gateway `control,router` and storage `cell`. `ARENA_ROLES` does not change those split-service roles. `LEGACY_REST_ENABLED=true` enables the REST business routes used alongside the public RPC APIs.

Leave `NATS_URL` unset for the default local setup. Redis can also be omitted; tenant operational reads have a fallback. Default reports use SQLite and need no DuckDB SDK.

## Frontend configuration

For the staff panel, create its environment file once:

```sh
cp apps/admin/.env.example apps/admin/.env
```

Its local values are:

```dotenv
VITE_API_URL=http://localhost:3000
VITE_GATEWAY_URL=ws://localhost:3000/realtime
```

The platform portal proxies `/platform` to `http://127.0.0.1:3000`. Set `PLATFORM_API_PROXY_TARGET` when starting the portal if your gateway uses another address.

## First portal operator

The operator command applies the embedded PostgreSQL control migrations and creates the account. Choose a username and a 12–72 byte password. This account manages tenants; it is separate from their business administrators.

```sh
python3 -c 'import getpass; print(getpass.getpass("Portal password: "))' | cargo run -p arena360-tools --bin platform_operator -- platform.admin
```

Replace `platform.admin` with your operator username. The command creates a new account; rerunning it for an existing username returns a duplicate error. An existing local operator can sign in without running this command again.

Build the Rust services:

```sh
pnpm services:build
```

Start the four processes in the [start guide](start.md). Open the portal, sign in, and enroll an authenticator. Register the database worker using the configured UUID and the storage origin `http://localhost:3001`. Confirm the default Trial plan is active before creating a tenant. The worker address must point to storage.

## Database and provisioning rules

Control migrations live in `crates/backend-core/migrations/control`; tenant migrations live in `crates/backend-core/migrations/tenant`. Service startup applies control migrations, and new tenant provisioning installs all tenant migrations. Existing tenant schema changes go through the owning cell's migration/rollout tooling and current lease.

Provisioning commits eleven canonical units, three roles, six templates, runtime timezone metadata, and validated initial setting overrides before lease acquisition and activation. Settings defaults remain in the shared catalog. Retries preserve customized values and restore missing seed rows; conflicting names/IDs or incompatible role/template types fail the seed transaction. Existing customized permissions are not automatically upgraded.

Generate a new migration with:

```sh
pnpm migration generate change_name --target control
pnpm migration generate change_name --target tenant
```

Do not edit existing applied migration versions. Optional manual control migration commands require the SQLx CLI and `CONTROL_DATABASE_URL`:

```sh
pnpm migration run --target control
pnpm migration info --target control
```

## Validation

```sh
pnpm rust:check
pnpm typecheck
pnpm --filter @gaming-cafe/tenant test
cargo test -p arena360-core --test tenant_provisioning --test access_control
```

For the broader backend suite, run `pnpm backend:test`. The isolated PostgreSQL/SQLite runner is `pnpm backend:test:integration`; it needs local PostgreSQL server tools (`pg_config`, `initdb`, `pg_ctl`) or an explicit `CONTROL_TEST_ADMIN_DATABASE_URL` that permits disposable database creation. Infrastructure-gated tests retain their explicit requirements.

After changing public contracts, run `pnpm lint:proto`, `pnpm gen:proto`, and `pnpm gen:api-types` as appropriate. Generated clients belong in their existing packages.

## Troubleshooting

| Symptom | Check |
| --- | --- |
| Database connection refused | Docker runtime, PostgreSQL container, port 5432, and `CONTROL_DATABASE_URL` |
| Address already in use | An existing process on 3000, 3001, 5173, or 5174; stop it or reuse it |
| Provisioning unavailable | Gateway storage URL/ID, matching private token, running storage service, active registered worker |
| Missing Trial plan | Control migrations completed and the Trial plan is active |
| Operator duplicate error | Use the existing account or choose another username |
| Login requests an authentication code | Complete first-login enrollment or use the enrolled authenticator |
| Default role conflict | Correct the conflicting local row and retry the same tenant request |
| Kiosk build fails | Native Tauri dependencies and the kiosk's independent Cargo workspace |

## Containers and deployment

Rust images build from the repository root:

```sh
pnpm rust:image --target gateway --tag arena360/tenant-gateway:local
pnpm rust:image --target storage --tag arena360/tenant-storage:local
```

`infra/helm/tenant-services` deploys gateway and storage; `infra/helm/platform-portal` deploys the portal separately. Shared service secrets contain `CONTROL_DATABASE_URL`, `JWT_SECRET`, and `STORAGE_SERVICE_TOKEN`. Gateway is stateless; storage uses a persistent tenant volume, stable cell UUID, and a single replica with Recreate updates. Default containers run as UID/GID 10001, so host-mounted tenant/key directories must be writable by that identity.

For recoverable backups, configure `REPLICATION_BUCKET` and `REPLICATION_KEY_DIR`, the S3 endpoint/credentials, and a separate durable backup-key volume. Remote replication is disabled when those required settings are absent. Moves and cold recovery depend on configured backup/hydration capability. Operator binaries for rollout, recovery, moves, rebalancing, and cold lifecycle live under `tools/backend-cli/src/bin`.
