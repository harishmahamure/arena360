# Arena360

Arena360 is an operations platform for a gaming cafe. It manages players, staff,
stations, timed play, plans, point-of-sale activity, inventory, cash controls,
and the station client used on gaming PCs.

This repository is a monorepo containing three runnable surfaces:

| Surface | Technology | Purpose |
|---|---|---|
| `apps/backend` | Rust, Axum, SQLx | REST API, authentication, business rules, realtime events, and persistence |
| `apps/admin` | React, Vite, MUI | Staff and administrator operations console |
| `apps/kiosk` | Tauri 2, React, Rust | Locked-down Windows station client and game launcher |

PostgreSQL is the system of record. Redis is an optional cache and invalidation
layer; the backend continues with a no-op cache if Redis is unavailable.

## Documentation

- [Current system and capabilities](docs/SYSTEM.md)
- [Product vision](docs/PRODUCT_VISION.md)
- [Product planning and milestones](docs/planning/README.md)
- [Local development](docs/DEVELOPMENT.md)
- [Deployment and operations](docs/DEPLOYMENT.md)

The documentation describes implemented behavior only. The generated OpenAPI
spec and the source code remain authoritative for endpoint-level details.

## Quick start

Prerequisites: Node 20, pnpm 9, a stable Rust toolchain, Docker, and
`sqlx-cli`.

```bash
corepack enable
pnpm install
docker compose up -d
cp apps/backend/.env.example apps/backend/.env
```

Set this local database URL in `apps/backend/.env`:

```dotenv
DATABASE_URL=postgres://arena360:arena360@localhost:5432/arena360
```

Then initialize the database and start the API and staff console in separate
terminals:

```bash
pnpm migration run
pnpm backend:dev
```

```bash
pnpm admin:dev
```

The API listens on `http://localhost:3000`. In non-production environments,
Swagger UI is available at `http://localhost:3000/api/docs`.

## Common checks

```bash
pnpm lint
pnpm typecheck
pnpm test
pnpm backend:test
pnpm backend:clippy
```

See [local development](docs/DEVELOPMENT.md) for the complete command matrix,
database workflow, and client-specific setup.
