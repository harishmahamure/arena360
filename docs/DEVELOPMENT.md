# Local development

## Prerequisites

- Node.js 20
- Corepack and pnpm 9.15.9
- Stable Rust with `rustfmt` and `clippy`
- Docker for local PostgreSQL, PgBouncer, and Redis
- `sqlx-cli` for migrations
- Tauri 2 platform prerequisites for `apps/kiosk`

## Repository map

```text
apps/
  admin/       staff web console
  backend/     Rust API and SQL migrations
  kiosk/       Windows Tauri station client
packages/
  api-types/   generated TypeScript OpenAPI types
  contracts/   shared runtime enums, permissions, and domain constants
  providers/   shared React providers
  theme/       design tokens and MUI theme
  ui/          shared React components
  utils/       HTTP, validation, session, and UI utilities
  biome-config/, tsconfig/  shared tooling configuration
infra/helm/    Kubernetes charts
scripts/       code generation, migrations, and repository checks
```

The pnpm workspace contains the admin, kiosk, and shared packages. The Rust
backend is intentionally run through Cargo rather than as a pnpm workspace.

## First-time setup

```bash
corepack enable
pnpm install
docker compose up -d
cp apps/backend/.env.example apps/backend/.env
cp apps/admin/.env.example apps/admin/.env
cp apps/kiosk/.env.example apps/kiosk/.env
```

The local Compose database uses:

```dotenv
DATABASE_URL=postgres://arena360:arena360@localhost:5432/arena360
REDIS_URL=redis://127.0.0.1:6379
```

Set a unique development `JWT_SECRET` of at least 32 characters, then apply
the schema:

```bash
pnpm migration run
```

## Run the system

Use separate terminals:

```bash
pnpm backend:dev
```

```bash
pnpm admin:dev
```

For the Windows station client:

```bash
pnpm kiosk:dev
```

`pnpm dev` runs the JavaScript workspace development tasks, but it does not
start the Rust API.

## Environment

### Backend

| Variable | Required | Purpose |
|---|---:|---|
| `DATABASE_URL` | yes | PostgreSQL or PgBouncer connection URL |
| `JWT_SECRET` | yes | HMAC signing secret, minimum 32 characters |
| `DATABASE_MAX_CONNECTIONS` | no | SQLx pool size; defaults to 10 |
| `REDIS_URL` | no | Redis cache and invalidation endpoint |
| `PORT` | no | HTTP port; defaults to 3000 |
| `CAFE_TZ` | no | Venue timezone; defaults to `Asia/Kolkata` |
| `JWT_ACCESS_EXPIRATION` | no | Admin/staff token lifetime |
| `JWT_PLAYER_EXPIRATION` | no | Player token lifetime |
| `JWT_DEVICE_EXPIRATION` | no | Device token lifetime |
| `STORAGE_*` | no | S3-compatible upload signing configuration |
| `ZEPTOMAIL_TOKEN` | no | Credential used by the available mail service |

### Browser and station clients

| Variable | Used by | Purpose |
|---|---|---|
| `VITE_API_URL` | admin and kiosk builds | REST API base URL |
| `VITE_GATEWAY_URL` / `VITE_API_URL_WS` | admin and station builds | WebSocket endpoint |
| `VITE_GALLERY_URL` | kiosk | CDN gallery manifest |
| `VITE_LOGIN_BACKGROUND_VIDEO_URL` | kiosk | Login background media |
| `VITE_KIOSK_LOGO_URL` | kiosk | Optional venue logo |
| `VITE_OFFLINE_GRACE_MINUTES` | kiosk | Local countdown grace after API loss |

## Database migrations

```bash
pnpm migration generate <description>
pnpm migration run
pnpm migration info
pnpm migration revert
pnpm migration prepare
```

Migration filenames are chronological SQL files in
`apps/backend/migrations`. Review both forward and down migrations where a
pair exists. Production migrations are opt-in in the backend deployment
workflow.

## API schema and generated types

The Rust `ApiDoc` definition is the source for the OpenAPI document.

```bash
pnpm gen:api-types
```

This runs the backend OpenAPI generator and refreshes
`packages/api-types/src/schema.ts`. Do not edit the generated schema by hand.
Swagger UI is mounted at `/api/docs` when the backend is not running in a
production environment.

## Quality checks

```bash
# TypeScript workspaces
pnpm lint
pnpm typecheck
pnpm test
pnpm build

# Rust backend
pnpm backend:fmt
pnpm backend:clippy
pnpm backend:test
pnpm backend:build

# Windows kiosk
pnpm --filter @gaming-cafe/kiosk test
pnpm --filter @gaming-cafe/kiosk test:rust
```

The root TypeScript test command does not include backend Cargo tests.

## Working rules

- Treat migrations, backend handlers/services, admin routes, and station state
  machines as the source of truth for implemented behavior.
- Keep shared runtime constants in `@gaming-cafe/contracts`; generated HTTP
  shapes belong in `@gaming-cafe/api-types`.
- Never commit `.env` files, signing keys, keystores, or updater private keys.
- Update [the current system document](SYSTEM.md) when a user-visible
  capability or system boundary changes. Avoid adding roadmap statements to
  current-state documentation.
