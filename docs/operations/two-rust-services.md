# Two Rust service deployments

This is the implemented SQLite-only Phase 1 deployment described in the
[network database architecture](../architecture/network-database-service.md).
Reports and operational APIs share each tenant SQLite file.

The root Cargo workspace separates deployable services, shared code, and operator tools:

| Package | Responsibility | Deployment storage |
| --- | --- | --- |
| `tenant-gateway` | PostgreSQL tenant management, platform APIs, control authentication, owner routing, and a wrapper for every tenant API | Stateless; no tenant volume |
| `tenant-storage` | Existing tenant repositories and business handlers, SQLite migrations and provisioning, SQLite reporting, WAL replication, recovery, and ownership fencing | Persistent tenant volume; stable cell identity |
| `sqlite-reporting` | Bounded read-only SQLite snapshots and calendar/ledger functions | Library |
| `arena360-core` | Shared application, repository, and control-plane implementation | Library used by the services |
| `arena360-tools` | Operator binaries, demo seed, schema rollout, recovery, and archive workers | Run explicitly when needed |
| `tenant-protocol` | Public API and private storage gRPC contracts | No runtime state |
| `gaming-cafe-api` | Compatibility launcher for the combined backend | Existing single-process deployment |

The gateway uses fixed `control,router` roles. Storage uses fixed `cell` roles. `ARENA_ROLES` cannot accidentally turn either service into an all-in-one process. The gateway clears `ARENA_CELL_ID` even when it is present in a shared environment file, and never creates a local tenant database manager or provisioner.

Business API requests and owner-side authentication completion use the private `TenantStorageService.Invoke` RPC. The owning storage cell checks the original JWT and current tenant grants, then runs the same handlers, repositories and SQLite transactions as before. Platform tenant creation uses `TenantStorageService.Provision`; migration and seeding execute on storage. SQL and database file paths are not exposed through RPC. The private service token is independent of user JWTs. Channels are reused, requests have deadlines and bounded payloads, and failed writes are not automatically retried.

The gateway keeps the existing public REST/gRPC-Web API behavior (`LEGACY_REST_ENABLED` remains opt-in). WebSocket realtime traffic continues through the existing streaming proxy. Default reports execute in storage against SQLite. Optional legacy historical exports require an explicitly enabled analytics build. This is a deployment split: existing tenant business logic stays with the repository implementation in storage, while both binaries reuse the existing library.

Storage still connects to control PostgreSQL for writer leases, migration state, identity projections and the recovery ledger. Removing that connection would require a separate lease/control RPC migration. PostgreSQL remains an external dependency, not a third Rust service deployment.

## Build and run

```sh
cargo check --workspace --all-targets
cargo build -p tenant-gateway
cargo build -p tenant-storage
cargo test -p arena360-core --test storage_rpc
```

Both services build and serve reports with default features, without a DuckDB SDK
or NATS. `duckdb-analytics` and `duckdb-bundled` remain explicit legacy historical
compatibility features and are excluded from the production images. The root
workspace shares dependency versions, `Cargo.lock` and `target/`; the kiosk Tauri
workspace remains independent.

Both entry points load the repository root `.env`, with `apps/backend/.env` as a compatibility fallback; exported variables take precedence. Both need:

- `CONTROL_DATABASE_URL`
- `JWT_SECRET` (the same value for both services)
- `STORAGE_SERVICE_TOKEN` (the same private value, at least 32 ASCII characters)

Storage additionally needs `ARENA_CELL_ID` and `TENANT_DATA_DIR`. Configure replication and recoverable backup keys using the existing settings. Redis and integration NATS are optional. Without NATS, `OUTBOX_RETENTION_DAYS` defaults to seven and only projected realtime events are expired. Gateway needs `STORAGE_SERVICE_URL` and `STORAGE_CELL_ID` for provisioning on the default storage cell. For normal tenant requests it resolves the current owner and address from PostgreSQL, preserving moves and cold hydration.

Example process commands, after exporting the shared database and auth settings:

```sh
PORT=3001 ARENA_CELL_ID=00000000-0000-4000-8000-000000000001 \
  TENANT_DATA_DIR=/var/lib/arena360/tenants cargo run -p tenant-storage

PORT=3000 STORAGE_SERVICE_URL=http://127.0.0.1:3001 \
  STORAGE_CELL_ID=00000000-0000-4000-8000-000000000001 cargo run -p tenant-gateway
```

Register the storage cell through the gateway's `/platform/cells` API using the same UUID and an address reachable from the gateway. The address serves native HTTP/2 gRPC and realtime HTTP/WebSocket on the same port; do not put an HTTP/1-only proxy between the services. Tenant provisioning requires a registered cell and active `trial` plan, as in the existing platform flow. Keep storage reachable only inside the deployment network. Use HTTPS or your cluster's encrypted service transport when traffic crosses an untrusted network; the gRPC client supports HTTPS with native trust roots.

## Container images and Kubernetes

Run builds from the repository root:

```sh
pnpm rust:image --target gateway --tag arena360/tenant-gateway:VERSION
pnpm rust:image --target storage --tag arena360/tenant-storage:VERSION
```

The helper sends an allowlisted Rust build context and excludes tenant data, build outputs and secrets; it also supports Docker installations without Buildx. Both images run as UID/GID 10001. The `gateway` image contains the stateless API. The `storage` image includes SQLite, the seeding, schema-rollout, recovery, tenant-move, rebalancing, cold-tenant and operator utilities, plus optional event stream setup, with no DuckDB SDK, extensions or bundled broker. The default `backend` target preserves the previous single-process topology using SQLite reporting. Its existing build workflow now uses repository-root context because dependencies live in the workspace.

Create a Kubernetes Secret named `arena360-tenant-services` containing the shared settings, then install the chart with your published image names:

```sh
helm upgrade --install arena360 infra/helm/tenant-services \
  --set gateway.image=YOUR_REGISTRY/tenant-gateway:VERSION \
  --set storage.image=YOUR_REGISTRY/tenant-storage:VERSION \
  --set storage.cellId=YOUR_STABLE_CELL_UUID
```

The chart creates exactly two Deployments, two internal Services and a retained tenant PVC. Gateway replicas can scale independently. Storage uses one replica and `Recreate` updates to avoid concurrent processes opening the same files. Point your ingress and platform portal API upstream at `arena360-gateway:3000`. Register the cell address as `http://arena360-storage:3001` for this release name.

Use `storage.persistence.existingClaim` to reuse existing tenant storage. Use `storage.replicationKeyClaim` for a separate durable backup-key PVC and supply the existing replication settings before production onboarding. The chart does not delete the tenant PVC on uninstall. Additional cells use worker-only releases with distinct identities and volumes:

```sh
helm upgrade --install arena360-cell-b infra/helm/tenant-services \
  --set gateway.enabled=false \
  --set storage.image=YOUR_REGISTRY/tenant-storage:VERSION \
  --set storage.cellId=YOUR_SECOND_CELL_UUID
```

Register `http://arena360-cell-b-storage:3001` in PostgreSQL through the platform
cell API. For a separate API release, set `storage.enabled=false`,
`gateway.storageUrl` to the default provisioning worker and `storage.cellId` to
that worker's UUID. Request routing follows each tenant's registered current owner.
Scaling the same storage Deployment to multiple replicas is unsupported.

## Validation

`storage_rpc` tests use a real TCP gRPC connection and migrated SQLite files. They check service authentication, tenant JWT enforcement, cross-tenant rejection, persisted session writes, remote migration/seeding, owner mismatch errors, and unavailable-storage responses. Reporting runs over that same network boundary without DuckDB or NATS.
`report_parity` checks all 31 captured report outputs with the corrected station
occupancy rule. `sqlite_reporting` checks exact money, snapshot consistency under
writes, tenant/venue isolation, ownership loss, index plans and outbox retention.
Existing provisioning, schema, ownership, startup and proxy tests also remain.
The combined-router regression checks public health and private gRPC on the same
worker listener. Local container smoke checks exercise dashboard, business,
finance and inventory reports through the gateway before and after a worker
restart using the same persistent tenant volume.

Before running a container with a host bind mount, make its data/key directories
writable by UID/GID 10001; the Helm chart supplies this through `fsGroup`. Keep
backup keys on durable separate storage and register the reachable cell identity
before tenant creation.

## Existing tenants and schema upgrades

New provisioning installs migration 0018 automatically. Existing tenant files
must pass the existing staged schema rollout before Phase 1 reports become
available; older schemas return explicit report unavailability. Enroll canaries
with `schema_rollout` on replication-enabled cells, then advance through the
persisted rollout gates. See [schema rollouts](schema-rollouts.md). Do not apply
DDL through unowned connections or bypass the backup hooks.

For repeated local builds, a `builder` image can retain Cargo's release cache:

```sh
pnpm rust:image --target builder --tag arena360/rust-build-cache:local
pnpm rust:image --target storage --tag arena360/tenant-storage:VERSION \
  --build-base arena360/rust-build-cache:local
```

Use a builder produced by this Dockerfile as the cache base. Runtime images copy
only release executables and do not contain the build toolchain or Cargo cache.
