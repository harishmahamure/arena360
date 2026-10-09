# Tenant management portal

The platform portal is a separate browser entry at `/portal.html`. It manages the new PostgreSQL control plane; tenant business operations continue through the existing admin panel and owning SQLite cell.

## Start for local testing

Use a dedicated local control database. Configure the backend with:

```sh
export CONTROL_DATABASE_URL=postgres://USER:PASSWORD@127.0.0.1:5432/arena_portal
export ARENA_CELL_ID=$(uuidgen)
export TENANT_DATA_DIR="$PWD/data/portal-tenants"
export JWT_SECRET=$(openssl rand -hex 32)
export PLATFORM_ADMIN_TOKEN=$(openssl rand -hex 32)
export PORT=3000
pnpm backend:dev
```

For business dashboards and reports, run the backend with `--features duckdb-analytics` using the project’s DuckDB SDK/extension setup, configure `NATS_URL` for a JetStream server, and initialize its stream with `pnpm analytics:stream:init`. Without these, the control portal and SQLite operations work, while analytical reports remain unavailable.

In another terminal:

```sh
pnpm portal:dev
```

Open `http://localhost:5174/portal.html` and enter the backend's `PLATFORM_ADMIN_TOKEN`. The Vite server proxies `/platform` to `http://127.0.0.1:3000`; set `PLATFORM_API_PROXY_TARGET` when the backend uses a different origin.

1. Open **Cells → Register cell**. Use the backend's exact `ARENA_CELL_ID`, a unique name and its HTTP origin. Registration does not start a cell process.
2. Open **Tenants → Create tenant**. Choose a unique lowercase slug, an IANA timezone and trial duration. Provisioning runs on the portal API's connected cell, migrates a real SQLite database, seeds defaults, acquires ownership and activates the tenant. The same slug/name/timezone can resume interrupted provisioning.
3. Open the tenant workspace and **Create administrator**. Choose a new global username and a password of 12–72 bytes. Existing usernames are rejected without modifying their credentials or memberships.
4. Use **Open business admin** to sign into the existing admin panel with that account and test tenant operations. The normal panel login/MFA flow applies. No platform token is passed to the business panel.
5. Inspect ownership generation, lease expiration, schema version, licenses and recent jobs. Change timezone to test asynchronous projection reconciliation.
6. With replication storage and at least two running cells configured, request a move. Only an ACTIVE target with a fresh hydration heartbeat is offered. Move and cold controls are disabled until the source cell has a fresh hydration heartbeat. The existing move agent performs the handoff. Pending moves can be cancelled before cutover.
7. With verified replication and a running hydration-capable cell, request cold storage, then wake the tenant. The existing cold agent snapshots, verifies, releases and cleans up. A wake response of 202 means hydration was requested, not completed. Poll the tenant state and jobs. Unreleased cold jobs can be cancelled.

Move/cold workers require the replication configuration described in `tenant-moves.md` and `cold-tenants.md`. Registration and plain provisioning work without object storage. Archive, backfill and export jobs are visible in the workspace; their existing command/API surfaces enqueue them. Cell capacity values are not fabricated by this portal. License status is displayed; billing and license editing are not implemented here.

## API and access

`/platform/*` is mounted independently of tenant routing, JWT authorization and the legacy REST flag. Every route requires an exact bearer match against `PLATFORM_ADMIN_TOKEN` (minimum 32 characters), using constant-time HMAC verification. Missing or short configuration disables the entire surface with 503. Tenant JWTs do not grant platform access. Tokens stay in browser memory, and requests omit cookies. Serve the portal and API through HTTPS outside localhost; provision the operator token through backend secrets, never through a `VITE_*` variable.

- `GET /platform/overview`, `/cells`, `/tenants` and `/tenants/{id}`
- `POST /platform/cells`, `/tenants` and `/tenants/{id}/admins`
- `PUT /platform/tenants/{id}/timezone`
- `POST /platform/tenants/{id}/move`, `/cold`, `/wake`
- `POST /platform/tenants/{id}/jobs/{job}/cancel` with `kind: "Move"` or `"Cold lifecycle"`

Tenant listing is paginated (30 by default, 100 maximum) and supports literal `search` and exact `state`. Job lists return the last ten records per job family. Responses omit password hashes, MFA secrets, object keys and export worker tokens. Cancel requests verify that the job belongs to the requested tenant before calling the existing fenced cancellation service.

## Build and verification

`pnpm portal:build` builds both admin and portal entries. Serve `portal.html` from the resulting admin `dist` directory and proxy `/platform` to the control API, or set `VITE_PLATFORM_API_BASE` to its origin at build time. The portal runs independently of the business panel's authentication/session bootstrap.

```sh
pnpm --filter @gaming-cafe/admin typecheck
pnpm --filter @gaming-cafe/admin exec vitest run src/portal/Portal.test.tsx
CONTROL_TEST_DATABASE_URL=postgres://... cargo test --manifest-path apps/backend/Cargo.toml --test platform_portal -- --ignored
```

The backend gate uses a real disposable control database and provisioned SQLite file to verify authentication, registration, provisioning/retry, hashed administrator creation, timezone validation, move/cold state admission and tenant-scoped cancellation. It is included in `pnpm backend:test:integration`.

### Verified on 2026-10-09

TypeScript checking, the production multi-entry build, all three portal UI tests, the operator-token unit check and the PostgreSQL/SQLite API integration gate pass. The native DuckDB API build also passes. A browser walkthrough registers a running local cell, provisions an ACTIVE tenant on schema 17 with a fresh lease, creates its administrator and signs into the business panel. With local JetStream initialized, the tenant's analytical overview loads from DuckDB. The local walkthrough uses dedicated test data; remote object storage and production staging drills remain separate prerequisites for full move/cold completion.
