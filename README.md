# Arena360

Arena360 manages gaming venues, stations, sessions, players, sales, inventory, and finance. The platform portal creates tenants and their administrators; the staff panel runs each venue; the kiosk runs on gaming stations.

## Guides

- [Start](docs/start.md): run an already configured workspace and test tenant creation.
- [Setup guidelines](docs/setup-guidelines.md): install prerequisites, configure services, and create the first portal operator.
- [Agent instructions](AGENTS.md): repository rules for coding agents.

## Applications and services

| Component | Package / path | Local URL |
| --- | --- | --- |
| Platform portal | `apps/tenant` | http://localhost:5174 |
| Staff panel | `apps/admin` | http://localhost:5173 |
| Station kiosk | `apps/kiosk` | Tauri desktop application |
| Tenant gateway | `apps/tenant-gateway` | http://localhost:3000 |
| Tenant storage | `apps/tenant-storage` | http://localhost:3001 |

The gateway handles platform APIs, staff authentication, and routing. Storage owns tenant SQLite files and business operations. PostgreSQL stores control-plane metadata and global staff identity. Reports use tenant SQLite snapshots by default. Redis is optional; NATS and DuckDB are not required for the default deployment.

## Workspace

```text
apps/                    Runnable services and frontend applications
crates/backend-core/     Shared application code, migrations, integration tests
crates/sqlite-reporting/ SQLite report snapshots and functions
crates/tenant-protocol/   Public and private protobuf definitions
packages/                Shared frontend packages and generated clients
tools/backend-cli/       Operator binaries and maintenance scripts
infra/docker/            Rust container builds
infra/helm/              Deployable Kubernetes charts
scripts/                 Workspace automation and client generators
docs/                    Start and setup guides
```

Shared Rust dependencies belong in the root Cargo workspace and lockfile. Application entry points stay thin; shared crates must not depend on application or operator packages. The kiosk's Tauri package retains its own Rust workspace and lockfile.

## Common commands

Run commands from the repository root.

| Task | Command |
| --- | --- |
| Install frontend dependencies | `pnpm install --frozen-lockfile` |
| Build both Rust services | `pnpm services:build` |
| Check Rust packages and tests | `pnpm rust:check` |
| Run core and operator tests | `pnpm backend:test` |
| Run isolated database integration tests | `pnpm backend:test:integration` |
| Check frontend types | `pnpm typecheck` |
| Run frontend tests | `pnpm test` |
| Build portal / staff panel | `pnpm portal:build` / `pnpm admin:build` |
| Generate protobuf clients | `pnpm gen:proto` |
| Generate API types | `pnpm gen:api-types` |
| Build operator tools | `pnpm tools:build` |

New tenants receive current migrations, three default roles, six role templates, and eleven units before activation. Platform settings stay in the code-owned catalog; tenant overrides are explicit. Provisioning retries preserve customizations and reject conflicts that would leave required defaults missing.
