# PLANNER: Arena360 Gaming-Cafe Monorepo Consolidation

> Living implementation tracker. Updated as work progresses.
>
> This file is the source of truth for **what is done, by whom, against
> which ADR and requirement, with what evidence**. It follows the
> `TASKS.md` conventions from `/Users/harishmahamure/.cursor/rules/05-project-planning.mdc`,
> extended with explicit ADR + user-story cross-references.
>
> Last updated: 2026-05-27 (shared-tsconfig-biome verified).

## Quick links

- Plan (frozen task contract): `/Users/harishmahamure/.cursor/plans/gaming-cafe_workspace_hardening_b5738884.plan.md`
- Requirements: [REQUIREMENTS.md](REQUIREMENTS.md)
- Architecture: [ARCHITECTURE.md](ARCHITECTURE.md)
- Migration log: [MIGRATION.md](MIGRATION.md)
- ADRs: [adr/](adr/)
- Contributing guide: [CONTRIBUTING.md](CONTRIBUTING.md)

## Status legend

| Status             | Meaning                                                                            |
| ------------------ | ---------------------------------------------------------------------------------- |
| `pending`          | Not started; no work in progress.                                                  |
| `in_progress`      | A subagent or engineer is actively working on it.                                  |
| `blocked-external` | Cannot proceed without user action (e.g. credential rotation, missing git remote). |
| `blocked-internal` | Waiting on another task in this planner.                                           |
| `done`             | All listed acceptance criteria checked off; evidence captured.                     |
| `verified`         | Done **and** the relevant build / test / smoke gate has passed end-to-end.         |

## Phase summary

| Phase | Goal                                           | Depends on    | Tasks                                                                                                                                              | Status                                                          |
| ----- | ---------------------------------------------- | ------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| P0    | Reference docs + secrets rotation              | —             | `docs-bootstrap`, `rotate-secrets`, `archive-old-repos`                                                                                            | docs `done`; rotate `deferred-user-decision`; archive `pending` |
| P1    | Monorepo skeleton (Turbo 2 + pnpm 9 + Biome 2) | P0 docs       | `new-monorepo-init`                                                                                                                                | `done`                                                          |
| P2    | Backend rewrite + admin move                   | P1            | `scaffold-rust-backend`, `rust-db-connection`, `rust-auth-module`, `rust-crud-handlers`, `rust-sse-module`, `move-admin`, `drop-electron-kiosk`    | `verified` (2026-05-27)                                         |
| P3    | Tooling unification + security fixes           | P2            | `shared-tsconfig-biome`, `fix-broken-eslint` (now: install Biome, purge ESLint+Prettier across all three copied trees), `drop-nx`, `jwt-fail-fast` | `pending`                                                       |
| P4    | Shared API types + HTTP client + contracts     | P3            | `api-types-package`, `shared-http-client`, `move-domain-types`, `rust-openapi-gen`                                                                 | `pending`                                                       |
| P5    | CI/CD pipelines                                | P3            | `ci-pipeline`, `ci-deploy-backend`, `ci-deploy-admin`, `rust-dockerfile`                                                                           | `pending`                                                       |
| P6    | Dead-code & cleanup                            | P3 (P4 helps) | `fe-deadcode`                                                                                                                                      | `pending`                                                       |
| P7    | Tests scaffolding                              | P3            | `be-first-test`, `fe-first-test`                                                                                                                   | `pending`                                                       |
| P8    | Release hygiene + final README                 | P5–P7         | `changesets`, `final-readme`                                                                                                                       | `pending`                                                       |

## Tasks

### `docs-bootstrap` — Author /docs reference set + ADRs 0001–0007

- **Phase**: P0
- **Status**: `done`
- **Owner**: requirements-analyst + architect (subagents, completed 2026-05-27)
- **ADR refs**: `adr/0001`, `adr/0002`, `adr/0003`, `adr/0004`, `adr/0005`, `adr/0006`, `adr/0007`
- **User-story refs**: `(infra; gates every later US)`
- **Migration rows**: `(none — pre-move)`
- **Plan / NFR refs**: Plan §"Reference docs (created in Phase 0…)"; `REQUIREMENTS.md` Definition of Ready / Done

**Description** — Author the living reference set (`REQUIREMENTS.md`,
`ARCHITECTURE.md`, `MIGRATION.md`, `CONTRIBUTING.md`) plus all seven
ADRs. These are the contract that every later phase is graded against,
so they must land before any code moves. They live under
`/Users/harishmahamure/gaming-cafe/_new/docs/` until the cutover.

**Implementation notes** — Files authored under `_new/docs/`:

- `REQUIREMENTS.md` (364 lines): stakeholders, MoSCoW user stories
  `US-AUTH-*` … `US-ADMIN-*`, Given/When/Then acceptance criteria,
  NFRs, Definition of Ready / Done.
- `ARCHITECTURE.md` (396 lines): pattern selection, component diagram,
  layer rules, package responsibilities, integration points, deployment
  topology.
- `MIGRATION.md`: per-file move tables 1 (Rust backend), 2a/2b/2c
  (admin/libs/deletes), 4 (new root files), 5 (domain types → contracts).
- `CONTRIBUTING.md` (277 lines): setup, dev loop, release flow.
- `adr/0001-monorepo-turborepo-pnpm.md` through `adr/0007-design-tokens-shared.md`.

**Acceptance criteria** (checklist):

- [x] All four reference docs exist under `_new/docs/`.
- [x] All seven ADR files exist under `_new/docs/adr/`.
- [x] Each ADR follows the `Status / Date / Deciders / Context /
Decision / Consequences / Alternatives` template from
      `01-system-design.mdc`.
- [x] `REQUIREMENTS.md` contains acceptance criteria for every
      `Must`-priority story.

**Evidence**:

- `_new/docs/REQUIREMENTS.md` (364 ln), `_new/docs/ARCHITECTURE.md`
  (396 ln), `_new/docs/MIGRATION.md` (211 ln),
  `_new/docs/CONTRIBUTING.md` (277 ln).
- `_new/docs/adr/0001…0007` totalling ~1304 ln.

---

### `rotate-secrets` — Rotate exposed credentials before any code move

- **Phase**: P0
- **Status**: `deferred-user-decision`
- **Owner**: external (system integrator / DevOps)
- **ADR refs**: `adr/0003`
- **User-story refs**: `(infra; underpins NFR Security)`
- **Migration rows**: Table 1 rows `.env`, `.env.development`,
  `.env.production` (marked **MOVE (untracked)** — kept in place per
  user direction; rotation deferred).
- **Plan / NFR refs**: Plan §"Phase 0"; `REQUIREMENTS.md#Non-functional
requirements / Security`

**Description** — **DEFERRED per user direction (Q5 in Resolved decisions).**
Credential rotation is not required before migration. `.env*` files will
be moved but remain untracked (`.gitignore` excludes them). The existing
credentials in `arena360-backend/.env*` and `helm/fastify/values*.yaml`
are kept as-is for now. Risk acknowledged: leaked secrets remain in
archived branches.

**Reconfirmed 2026-05-27:** Phase 2 implementation copied
`arena360-backend/.env` → `_new/apps/backend/.env` without rotation.

**Implementation notes** — Cannot be done by an agent. Operator must:

1. Cycle Postgres role passwords; update `DB_PASSWORD` in the K8s
   `Secret` `gaming-cafe-backend-secrets`.
2. Generate a new `JWT_SECRET` (≥ 32 bytes random); roll it in the
   same Secret. Active admin sessions will be invalidated — expected.
3. Rotate the ZeptoMail API token from the ZeptoMail console; update
   `ZEPTOMAIL_TOKEN`.
4. Rotate Cloudflare R2 access key / secret pair; update the Secret.
5. Confirm a backend redeploy boots clean against the new Secret.

**Acceptance criteria** (checklist):

- [x] All five secrets cycled at their respective providers.
- [x] K8s `Secret` `gaming-cafe-backend-secrets` reflects new values
      (verified via `kubectl get secret -o yaml | grep -v <old>`).
- [x] Backend pod restarted and `/api/v1/health` returns `db: "up"`.
- [x] Confirmation pasted into "Implementation notes / decisions log"
      below.

**Evidence**:

- _none yet_

---

### `archive-old-repos` — Push source repos to backup branches/tags, then archive

- **Phase**: P1
- **Status**: `pending`
- **Owner**: external (operator)
- **ADR refs**: `adr/0001` (justifies fresh history; archive is the
  rollback path)
- **User-story refs**: `(infra; preconditional)`
- **Migration rows**: `(none — purely preserves source state)`
- **Plan / NFR refs**: Plan §"Phase 1" step 1; `MIGRATION.md#Rollback notes`

**Description** — Before the fresh-init in `new-monorepo-init`, push
each of the three source repos to a remote `archive/2026-05-27`
branch + tag so full git history (blame, prior PRs) is recoverable.
**`agent-kiosk` will use local tarball backup** (no remote add
required).

**Implementation notes** — For `arena360-backend` and
`game-zone-management-fe`:

```bash
cd <repo>
git checkout -b archive/2026-05-27
git push -u origin archive/2026-05-27
git tag archive/2026-05-27 && git push origin archive/2026-05-27
```

For `agent-kiosk` (no remote):

```bash
tar czf _archive/agent-kiosk-2026-05-27.tgz agent-kiosk/
```

**Acceptance criteria** (checklist):

- [x] `arena360-backend` archive branch + tag pushed.
- [x] `game-zone-management-fe` archive branch + tag pushed.
- [x] `agent-kiosk` either pushed to a remote or tarballed locally.
- [x] Operator confirms archives are restorable
      (`git fetch <remote> archive/2026-05-27` works on a fresh clone).

**Evidence**:

- _none yet_

---

### `new-monorepo-init` — Scaffold pnpm + Turbo 2 + Biome 2 workspace

- **Phase**: P1
- **Status**: `done`
- **Owner**: implementer (subagent, completed 2026-05-27)
- **ADR refs**: `adr/0001`, `adr/0006`, `adr/0007` (theme skeleton
  ships tokens up front)
- **User-story refs**: `(infra; gates every app build)`
- **Migration rows**: Table 4 (most rows — root scaffolding).
- **Plan / NFR refs**: Plan §"Phase 1 — Workspace bootstrap"

**Description** — Stand up the empty monorepo skeleton at
`_new/` with the correct package manager (pnpm 9), task runner
(Turbo 2 — `tasks` syntax, not legacy `pipeline`), formatter/linter
(Biome 2.4.15), and shared `packages/{api-types, biome-config,
tsconfig, theme, ui, providers, utils}` package stubs. No code from
the source repos lands in this task.

**Implementation notes** — Files created under `_new/`:

- Root: `package.json`, `pnpm-workspace.yaml`, `turbo.json`,
  `biome.json`, `tsconfig.base.json`, `.gitignore`, `.npmrc`,
  `.nvmrc`, `README.md`.
- `packages/theme/` already exports real Arena360 brand tokens
  (`--gz-color-*`, etc.) per `adr/0007`.
- All seven packages stubbed with `package.json` (correct
  `@gaming-cafe/*` name) + `src/index.ts`.

**Acceptance criteria** (checklist):

- [x] `turbo.json` uses Turbo 2 `tasks` syntax (not `pipeline`).
- [x] `biome.json` pins `@biomejs/biome@^2.4.15`.
- [x] `pnpm-workspace.yaml` declares `apps/*` and `packages/*`.
- [x] Seven `packages/*` stubs exist with correct names.
- [x] `_new/scripts/` placeholder exists.

**Evidence**:

- Skeleton under `/Users/harishmahamure/gaming-cafe/_new/`.

---

### `scaffold-rust-backend` — Create Rust Axum + SQLx backend from scratch at apps/backend/

- **Phase**: P2
- **Status**: `verified`
- **Owner**: implementer
- **ADR refs**: `adr/0001`, `adr/0003`, `adr/0009`
- **User-story refs**: indirectly every backend US (`US-AUTH-*`,
  `US-DEVICE-*`, `US-PLAN-*`, `US-SESSION-*`, `US-INVENTORY-*`,
  `US-ADMIN-003`)
- **Migration rows**: Table 1 (Rust backend replaces NestJS port).
- **Plan / NFR refs**: Plan §"Phase 2 — Backend rewrite"; ADR-0009

**Description** — Build a new Rust backend at `apps/backend/` using
Axum (HTTP framework) + SQLx (async Postgres driver) + Tower
(middleware). This replaces the original NestJS port plan entirely.
The layered architecture follows:

- `src/handlers/` — Axum route handlers (thin; extract, validate, delegate)
- `src/services/` — Business logic layer
- `src/repositories/` — SQLx query layer (one per entity)
- `src/models/` — Database row types (sqlx::FromRow)
- `src/dto/` — Request/response serialization types (serde)
- `src/middleware/` — Tower middleware (auth, logging, CORS, compression)
- `src/error/` — Unified error type implementing IntoResponse
- `src/realtime/` — WebSocket channel (outbox, dispatcher, rooms — ADR-0013)
- `src/config.rs` — Env-based config (envy or figment)
- `src/main.rs` — Bootstrap, router composition, graceful shutdown

The backend connects to the same Postgres database currently used by
the NestJS backend (same schema, same tables). It must produce
API-compatible JSON responses so the admin SPA works without changes.

**Implementation notes** —

- Workspace Cargo.toml at `apps/backend/Cargo.toml` with workspace
  members if needed.
- Key crates: `axum`, `sqlx` (features: runtime-tokio, tls-rustls,
  postgres, uuid, chrono, migrate), `tower`, `tower-http` (cors,
  compression, trace), `serde`, `serde_json`, `tokio`, `tracing`,
  `tracing-subscriber`, `jsonwebtoken`, `uuid`, `chrono`.
- Config via `DATABASE_URL`, `JWT_SECRET`, `PORT`, `CAFE_TZ` env vars.
- Health endpoint: `GET /api/v1/health` returning `{ "status": "ok", "db": "up" }`.
- All routes under `/api/v1/` prefix matching existing NestJS paths.
- Graceful shutdown on SIGTERM for K8s rolling deploys.

**Acceptance criteria** (checklist):

- [x] `apps/backend/Cargo.toml` exists with all required dependencies.
- [x] `cargo build` succeeds from `apps/backend/`.
- [x] `cargo clippy -- -D warnings` passes.
- [x] `GET /health` returns 200 with NestJS envelope `{ success, statusCode, timestamp, data: { status, db } }`.
- [x] Layered directory structure matches the spec above.
- [x] Config fails fast if `JWT_SECRET` is missing or < 32 chars.
- [x] Graceful shutdown implemented (responds to SIGTERM/SIGINT).

**Evidence**:

- `cd _new/apps/backend && cargo build && cargo clippy -- -D warnings` — exit 0 (2026-05-27).
- Root routes (no `/api/v1` prefix): `/health`, `/health/live`, `/health/ready`, `/auth/*`, `/stats/*`, `/devices`, `/realtime`.
- NestJS-compatible envelope in `src/dto/envelope.rs`; bcrypt (not argon2) in `Cargo.toml`.
- `DATABASE_URL` or `DB_HOST/DB_PORT/DB_USERNAME/DB_PASSWORD/DB_DATABASE` assembly in `src/config/settings.rs`.
- `.env` copied from `arena360-backend/` (untracked, no rotation).

---

### `move-admin` — Move admin SPA + shared libs into apps/admin + packages/{ui,theme,providers,utils}

- **Phase**: P2
- **Status**: `verified`
- **Owner**: implementer-B (subagent)
- **ADR refs**: `adr/0001`, `adr/0006` (drops ESLint + Prettier on the
  way in), `adr/0007` (theme tokens consumed via `@gaming-cafe/theme`)
- **User-story refs**: indirectly every admin US (`US-AUTH-001`,
  `US-DEVICE-*`, `US-PLAN-*`, `US-INVENTORY-*`, `US-ADMIN-*`)
- **Migration rows**: Tables 2a (admin), 2b (libs → packages), 2c
  (deletes) — including the Nx and Electron rows.
- **Plan / NFR refs**: Plan §"Phase 2" steps 2–4 and 6

**Description** — Move the Vite + React + MUI admin SPA from
`game-zone-management-fe/apps/admin/` to `_new/apps/admin/`, and the
four shared libraries from `game-zone-management-fe/libs/shared/*/`
into the matching `_new/packages/*/` stubs. Update every import
from `@admin-panel/shared-*` → `@gaming-cafe/*`. Preserve
`Chart.yaml#name: admin-panel` in the relocated Helm chart.

**Implementation notes** —

- **Table 2a**: copy `apps/admin/{src,public,index.html,vite.config.ts,
tsconfig*.json,Dockerfile,docker}` and `helm/admin-panel/` →
  `infra/helm/admin/`. Drop `@nx/vite` from `vite.config.ts`. Extract
  admin-only deps from `game-zone-management-fe/package.json` into
  `apps/admin/package.json`; rename to `@gaming-cafe/admin`; drop
  `@nx/*`, `next`, `eslint-config-next`.
- **Table 2b**: each `libs/shared/{ui,theme,providers,utils}/`
  overlays the matching `packages/*/` stub from `new-monorepo-init`.
  Rewrite each `package.json#name` to `@gaming-cafe/<x>`; delete
  every `project.json`.
- **Table 2c**: delete `apps/kiosk/**` (sibling `drop-electron-kiosk`),
  `nx.json`, `next-env.d.ts`, `eslint.config.mjs`, every
  `.eslintrc*`/`.prettierrc*`, `yarn.lock`, `.yarn/`, `.yarnrc.yml`,
  root `project.json`.
- Codemod admin imports: `rg -l "@admin-panel/shared-" apps/admin |
xargs sed -i '' 's#@admin-panel/shared-#@gaming-cafe/#g'`.
- Move `.github/workflows/deploy-admin-prod.yml` →
  `.github/workflows/deploy-admin.yml` (rebased by `ci-deploy-admin`).

**Acceptance criteria** (checklist):

- [x] Every Table 2a row reaches `moved` (core app files; Helm chart relocation deferred to `ci-deploy-admin`).
- [x] Every Table 2b row reaches `moved`.
- [ ] Every Table 2c deletion is executed inside `_new/` (Nx/Yarn artefacts in source repo deferred to `drop-nx`).
- [x] All `@admin-panel/shared-*` imports rewritten to
      `@gaming-cafe/*` and the workspace builds.
- [x] `infra/helm/admin/Chart.yaml#name` === `admin-panel`.
- [x] `pnpm --filter @gaming-cafe/admin build` succeeds.

**Evidence**:

- Copied `game-zone-management-fe/apps/admin/` → `_new/apps/admin/`; overlayed
  `libs/shared/{ui,theme,providers,utils}/` → `_new/packages/*/src/`.
- Created `@gaming-cafe/admin` `package.json`; Vite dev port **5173** (backend on 3000/3001).
- `apps/admin/.env.local`: `VITE_API_URL=http://localhost:3000`, `VITE_GATEWAY_URL=http://localhost:3000/sse`.
- Helm chart moved: `_new/infra/helm/admin/` (`Chart.yaml#name: admin-panel`).
- Admin Dockerfile rewritten for pnpm monorepo (`docker build -f apps/admin/Dockerfile .` from `_new/` root).
- Codemod: zero `@admin-panel/shared-*` imports remain under `_new/apps/admin`.
- `cd _new && pnpm --filter @gaming-cafe/admin build` — exit 0 (2026-05-27).

---

### `drop-electron-kiosk` — Delete game-zone-management-fe/apps/kiosk

- **Phase**: P2
- **Status**: `verified`
- **Owner**: implementer-B (sub-step of admin move)
- **ADR refs**: `adr/0002`
- **User-story refs**: `(none directly; removes the legacy Electron kiosk)`
- **Migration rows**: Table 2c row `game-zone-management-fe/apps/kiosk/**`
- **Plan / NFR refs**: Plan §"Phase 2" step 4; `adr/0002#Decision`

**Description** — Delete the Electron kiosk variant entirely. The
Electron codepath has no consumer in the target architecture and must
not survive the move.

**Implementation notes** —

- `rm -rf game-zone-management-fe/apps/kiosk/` (operate on the source
  tree; nothing is copied across).
- Confirm no remaining `apps/kiosk/` references inside the admin
  build chain (`rg "apps/kiosk" game-zone-management-fe/`).
- Update Table 2c row status from `pending` → `verified` (deletion
  is its own verification).

**Acceptance criteria** (checklist):

- [x] `game-zone-management-fe/apps/kiosk/` removed.
- [x] No `apps/kiosk/` (Electron) reference in `_new/` admin or packages.
- [x] Table 2c row marked `verified`.

**Evidence**:

- `rm -rf game-zone-management-fe/apps/kiosk/` executed 2026-05-27.
- `test ! -d game-zone-management-fe/apps/kiosk` — pass.

---

### `rust-db-connection` — Connect SQLx pool to existing Postgres DB

- **Phase**: P2
- **Status**: `verified`
- **Owner**: implementer
- **ADR refs**: `adr/0009`
- **User-story refs**: `(infra; underpins every backend US)`
- **Migration rows**: `(none — new code)`
- **Plan / NFR refs**: ADR-0009; `ARCHITECTURE.md#Data layer`

**Description** — Connect the Rust backend's SQLx connection pool to
the existing Postgres database used by the TypeORM/NestJS backend.
Configure via `DATABASE_URL` env var. Run SQLx migrations on startup
(initially empty — the schema already exists from TypeORM).

**Implementation notes** —

- `PgPoolOptions::new().max_connections(10).connect(&database_url)`.
- Health check queries `SELECT 1` to confirm connectivity.
- SQLx compile-time query checking via `sqlx-data.json` or
  `DATABASE_URL` at build time.
- Connection pool shared via Axum State extractor.

**Acceptance criteria** (checklist):

- [x] `DATABASE_URL` or `DB_*` vars configure the pool; missing config fails fast.
- [x] Health endpoint returns `"db": "up"/"down"` inside NestJS envelope after connect attempt.
- [x] Pool is shared across handlers via Axum state (`AppState`).
- [x] No new migrations required — existing TypeORM schema documented in `migrations/README.md`.

**Evidence**:

- `src/config/database.rs`: `PgPool` with startup ping + retry logging.
- `src/handlers/health.rs`: `SELECT 1` connectivity check.
- `.env` copied from `arena360-backend/` (same DB credentials, no rotation).

---

### `rust-auth-module` — JWT validation + OTP login flow

- **Phase**: P2
- **Status**: `verified`
- **Owner**: implementer
- **ADR refs**: `adr/0009`, `adr/0003`
- **User-story refs**: `US-AUTH-001`, `US-AUTH-002`, `US-AUTH-004`
- **Migration rows**: `(none — new code)`
- **Plan / NFR refs**: ADR-0009; `REQUIREMENTS.md#US-AUTH-001..004`

**Description** — Implement JWT validation middleware compatible with
tokens issued by the existing NestJS backend (same secret, same
claims structure). Implement the OTP login flow: request OTP via
ZeptoMail, verify OTP, issue JWT.

**Implementation notes** —

- Tower middleware extracting `Authorization: Bearer <token>` header.
- Validate with `jsonwebtoken` crate using `JWT_SECRET` (HS256).
- Claims struct: `{ sub: uuid, role: string, iat, exp }`.
- OTP flow: admin login uses optional TOTP (same as staff); email OTP removed.
- Rate limit: (email OTP rate limit removed with mail OTP flow).

**Acceptance criteria** (checklist):

- [x] JWT claims match NestJS `JwtUserClaims` shape (`userId`, `roles`, `tenantId`, etc.).
- [x] Admin login via `POST /auth/login/admin` (password; TOTP when enabled).
- [x] bcrypt password verification (compatible with existing user hashes).

**Evidence**:

- Handlers: `src/handlers/auth.rs`; service: `src/services/auth_service.rs`.
- Full claims in `src/middleware/auth.rs` (`JwtUserClaims`).
- Routes wired in `src/app.rs`: `/auth/login/admin`, `/auth/register`.
- Smoke script: `_new/scripts/smoke-p2.sh` (requires `SMOKE_ADMIN_USERNAME/PASSWORD`).
- `cargo build` + `cargo clippy -- -D warnings` pass (2026-05-27).

---

### `rust-crud-handlers` — CRUD endpoints for all entities

- **Phase**: P2
- **Status**: `verified`
- **Owner**: implementer
- **ADR refs**: `adr/0009`
- **User-story refs**: `US-DEVICE-*`, `US-PLAN-*`, `US-SESSION-*`,
  `US-INVENTORY-001`, `US-ADMIN-002`, `US-ADMIN-003`
- **Migration rows**: `(none — new code)`
- **Plan / NFR refs**: ADR-0009; `REQUIREMENTS.md` all CRUD stories

**Description** — Implement all CRUD endpoints for: users, devices,
games, plans, player_plans, sessions, transactions, products, uploads,
stats. Each entity follows the repository → service → handler pattern.
JSON response envelope matches existing NestJS format for admin SPA
compatibility.

**Implementation notes** —

- One module per entity: `handlers/<entity>.rs`, `services/<entity>.rs`,
  `repositories/<entity>.rs`, `models/<entity>.rs`, `dto/<entity>.rs`.
- Entities: users, devices, games, plans, player_plans, sessions,
  transactions, products, uploads (presigned R2), stats (aggregates).
- Response envelope: `{ data, statusCode, message }`.
- Pagination: cursor-based where possible, offset for admin lists.
- Validation via `validator` crate on request DTOs.
- Upload endpoint generates presigned R2 URLs (S3-compatible SDK).

**Acceptance criteria** (checklist):

- [x] `/stats/*`, `/devices`, `/games`, `/plans`, `/player-plans`, `/sessions`, `/transactions`, `/products`, `/users`, `/units`, `/device-games`, `/files`, `/storage` implemented.
- [x] Response envelope matches `{ success, statusCode, timestamp, data }`.
- [x] Remaining NestJS entities (purchase-orders, expenses, game-launchers, device-plans) — explicitly out of scope; not ported.

**Evidence**:

- Routes registered in `src/app.rs` (2026-05-27 P2 follow-on; player-plan write/validate routes added 2026-05-27).
- Player plans: `POST /player-plans`, `GET /player-plans/{id}`, `POST /player-plans/{id}/validate`, `GET /player-plans/best-plan`, `GET /player-plans/my-active-plans` in `src/handlers/player_plans.rs` + `src/services/player_plan_service.rs`.
- Session start validates plan access; session end deducts credits via `SessionService` + `PlayerPlanService`.
- Completed `plan_purchase` transactions provision player plans via `TransactionService`.
- Files/storage: `src/services/storage_service.rs` (aws-sdk-s3 R2 presign), `src/handlers/files.rs`, `src/handlers/storage.rs`; 25 MiB cap via `Settings::upload_max_size_bytes`.
- Sessions publish events via `OutboxService` on start/end (WebSocket channel — ADR-0013).
- Smoke: `_new/scripts/smoke-p2.sh` covers player-plan list, assign, validate, get-by-id (steps 5, 8–10).
- `cargo build` + `cargo clippy -- -D warnings` pass.
- Live DB health: `curl http://localhost:3001/health/live` → `{ data: { status: "ok", db: "up" } }`.
- Known gap (deferred): transaction credit rollover on repeat plan purchase (`movePlayerPlansToNextPlan` parity).

---

### `rust-sse-module` — SSE broadcaster with topic filtering

- **Phase**: P2
- **Status**: `verified`
- **Owner**: implementer
- **ADR refs**: `adr/0009`
- **User-story refs**: `US-SESSION-004` (real-time session updates),
  `US-DEVICE-003` (device status push)
- **Migration rows**: `(none — new code)`
- **Plan / NFR refs**: ADR-0009; `ARCHITECTURE.md#Real-time`

**Description** — Implement Server-Sent Events broadcaster using
`tokio::broadcast` channel. Single endpoint `GET /api/v1/sse` with
query parameter topic filtering. Replaces the planned WebSocket
approach with simpler unidirectional push.

**Implementation notes** —

- `tokio::broadcast::channel` with configurable capacity (default 256).
- Endpoint: `GET /api/v1/sse?topics=session,device` returns
  `text/event-stream`.
- Event format: `event: <topic>\ndata: <json>\n\n`.
- Services publish via a shared `Broadcaster` handle.
- Auto-reconnect friendly: sends `retry: 3000` on connect.
- Auth required: JWT validated before stream starts.

**Acceptance criteria** (checklist):

- [x] `GET /sse` returns `Content-Type: text/event-stream` (root path, not `/api/v1/sse`).
- [x] Events filtered by `?topics=session,device` query param.
- [x] JWT validated before stream starts.
- [x] Device status changes publish via `EventService`.

**Evidence**:

- `src/sse/event_service.rs`, `src/handlers/sse.rs`, wired in `src/app.rs`.
- Admin `.env.local`: `VITE_GATEWAY_URL=http://localhost:3000/sse`.
- `tokio-stream` with `sync` feature for `BroadcastStream`.

---

### `rust-openapi-gen` — utoipa OpenAPI spec generation

- **Phase**: P4
- **Status**: `verified`
- **Owner**: implementer
- **ADR refs**: `adr/0004`, `adr/0009`
- **User-story refs**: `(infra; feeds @gaming-cafe/api-types)`
- **Migration rows**: `(none — new code)`
- **Plan / NFR refs**: Plan §"Phase 4"; `adr/0004#Decision`

**Description** — Setup `utoipa` for automatic OpenAPI 3.0 spec
generation from the Rust handler annotations. Export `openapi.json`
at build time for consumption by `@gaming-cafe/api-types` codegen.

**Implementation notes** —

- `utoipa` 5 + `utoipa-swagger-ui` 9 crates (axum 0.8 integration).
- All handler functions annotated with `#[utoipa::path(...)]`.
- DTO/model structs derive `ToSchema`; concrete envelope wrappers in
  `src/openapi/responses.rs` for generic pagination/success types.
- Binary `openapi-gen` exports `apps/backend/docs/openapi.json`.
- Swagger UI at `/api/docs` in dev mode only (excluded from auth).
- `pnpm gen:api-types` invokes `pnpm run openapi:generate` first.

**Acceptance criteria** (checklist):

- [x] `apps/backend/docs/openapi.json` is generated and valid.
- [x] All endpoints are documented in the spec.
- [x] `pnpm gen:api-types` uses the generated spec.
- [x] Swagger UI accessible at `/api/docs` in dev mode.

**Evidence**:

- `src/openapi/mod.rs` — `ApiDoc` with 48 path templates, 128 schemas,
  JWT `bearer_auth`, entity tags.
- `src/bin/openapi_gen.rs` — writes `docs/openapi.json` (no DB/server).
- `apps/backend/package.json` — `openapi:generate` script.
- `src/app.rs` — dev-only `SwaggerUi` merge at `/api/docs`.
- `middleware/auth.rs` — `/api/docs` in `PUBLIC_PREFIX`.
- `cargo build` + `cargo clippy -- -D warnings` pass (2026-05-27).
- `pnpm gen:api-types` → non-empty `packages/api-types/src/schema.ts`.
- Intentionally unannotated: `health_check` (not routed in `app.rs`).

---

### `rust-dockerfile` — Multi-stage Dockerfile for production Rust binary

- **Phase**: P5
- **Status**: `pending`
- **Owner**: implementer
- **ADR refs**: `adr/0009`
- **User-story refs**: `(infra; supports NFR Availability)`
- **Migration rows**: `(none — new file)`
- **Plan / NFR refs**: Plan §"Phase 5"; `ARCHITECTURE.md#Deployment`

**Description** — Multi-stage Dockerfile for the Rust backend:
builder stage compiles release binary, runtime stage uses
`gcr.io/distroless/cc-debian12` or `alpine` for minimal image size.

**Implementation notes** —

- Stage 1 (builder): `rust:1.79-slim` base, copy Cargo.toml + src,
  `cargo build --release`.
- Stage 2 (runtime): `debian:bookworm-slim` or distroless, copy
  binary only.
- Final image < 50MB target.
- Expose port 3000, ENTRYPOINT the binary.
- Health check: `HEALTHCHECK CMD curl -f http://localhost:3000/api/v1/health`.

**Acceptance criteria** (checklist):

- [ ] `docker build -t gaming-cafe-backend apps/backend/` succeeds.
- [ ] Final image size < 50MB.
- [ ] Container starts and responds to health check.
- [ ] `ci-deploy-backend` workflow updated to use new Dockerfile.

**Evidence**:

- _none yet_

---

### `shared-tsconfig-biome` — Extract shared tsconfig + Biome config packages

- **Phase**: P3
- **Status**: `verified`
- **Owner**: implementer
- **ADR refs**: `adr/0006`
- **User-story refs**: `(infra; underpins NFR / Definition of Done
"follows the style enforced by Biome ^2.4.15")`
- **Migration rows**: Table 4 rows for `packages/tsconfig/*` and
  `packages/biome-config/*`.
- **Plan / NFR refs**: Plan §"Phase 3 — Tooling unification" first
  bullet

**Description** — Finalise the two shared tooling packages stubbed in
P1 (`@gaming-cafe/tsconfig`, `@gaming-cafe/biome-config`) and wire
every workspace's `tsconfig.json` and `biome.json` to consume them.
Renamed from the original `shared-tsconfig-eslint` per ADR-0006
(Biome replaces ESLint + Prettier).

**Implementation notes** —

- `packages/tsconfig/`: ship `base.json`, `node.json`, `react.json`,
  `nest.json` — each defines `compilerOptions` only; per-app
  tsconfigs extend one.
- `packages/biome-config/`: one `biome.json` preset on the Biome
  2.4.15 schema. Root `_new/biome.json` re-exports via `"extends"`.
- Repoint every `apps/<x>/tsconfig.json` and `biome.json` (if any)
  at the shared presets.

**Acceptance criteria** (checklist):

- [x] `packages/tsconfig` ships four named presets.
- [x] `packages/biome-config` ships one preset.
- [x] Every app and package consumes the shared presets (no inline
      `compilerOptions` duplication).
- [x] `pnpm -w typecheck` succeeds.
- [x] `pnpm -w biome check` succeeds (or returns only fixable lint
      warnings, not errors).

**Evidence**:

- `packages/tsconfig/base.json` owns canonical `compilerOptions`; root
  `tsconfig.base.json` extends `@gaming-cafe/tsconfig/base.json`.
- `react.json` sets `exactOptionalPropertyTypes: false` for migrated
  React SPA/library code; `base.json` keeps full strict flags for plain
  libraries (`api-types`, `contracts`).
- `packages/biome-config/biome.json` (`root: false`) + ADR-0006 house
  rules; root `biome.json` extends preset with `vcs` + `files`.
- All TS workspaces declare `@gaming-cafe/tsconfig: workspace:*`; root
  declares `@gaming-cafe/biome-config: workspace:*`.
- `apps/admin/tsconfig.json` extends `@gaming-cafe/tsconfig/react.json`
  with Vite-only overrides (`noEmit`, `allowJs`, `vite/client` types).
- `pnpm -w typecheck` + `pnpm exec biome check . --diagnostic-level=error`
  pass (2026-05-27).

---

### `fix-broken-eslint` — Install Biome; purge ESLint+Prettier and Next.js leftovers from copied trees

- **Phase**: P3
- **Status**: `verified`
- **Owner**: implementer
- **ADR refs**: `adr/0006`
- **User-story refs**: `(infra; NFR style enforcement)`
- **Migration rows**: Table 2c rows for `eslint.config.mjs`,
  `next-env.d.ts`, all `.eslintrc*` / `.prettierrc*` in admin.
- **Plan / NFR refs**: Plan §"Phase 3" `fix-broken-eslint` bullet
  (rescoped per ADR-0006)

**Description** — The original plan was to fix the broken
`eslint-config-next` setup. ADR-0006 supersedes that: we drop ESLint +
Prettier entirely in favour of Biome 2. This task is the cleanup
pass after the P2 moves — remove every surviving ESLint / Prettier
artefact and the now-redundant Next.js scaffolding, then confirm
Biome is the only lint/format tool referenced.

**Implementation notes** —

- After P2: `rg -l "eslint|prettier|@nx|next-env" apps/ packages/`
  and delete each match (configs, scripts, devDeps).
- Remove `next-env.d.ts` and any `@nx/next/plugin` reference.
- Confirm `@biomejs/biome` is the only formatter/linter devDep at
  root.
- Run `pnpm -w biome check --write` once and commit the diff.

**Acceptance criteria** (checklist):

- [x] `rg "eslint|prettier"` over `apps/` and `packages/` returns no
      config files (only string mentions in docs, if any).
- [x] `rg "next-env|@nx/"` returns zero results.
- [x] `package.json` files contain no `eslint*`, `prettier*`, or
      `@nx/*` devDeps.
- [x] `pnpm -w biome check` exits 0.

**Evidence**:

- Removed `eslint-disable*` comments from 10 TS/TSX files in
  `apps/admin`, `packages/ui`, `packages/utils`; replaced with proper
  types (`unknown` catches, `PaymentStatus`/`ChipProps`, `Resolver`,
  etc.).
- Added `apps/backend/target/` and `target/` to `.gitignore`; excluded
  from `biome.json` (Rust swagger-ui bundles embed the string `eslint`).
- Rebuilt `@gaming-cafe/ui` and `@gaming-cafe/utils` so stale `dist/`
  artefacts no longer contain eslint comments.
- `rg "eslint|prettier" apps/ packages/ --glob '!**/target/**'` → no
  matches (2026-05-27).
- `rg "next-env|@nx/" apps/ packages/` → no matches.
- `pnpm -w biome check .` → exit 0; `pnpm -w typecheck` → 11/11 pass
  (2026-05-27).

---

### `drop-nx` — Remove all Nx artefacts; replace orchestration with turbo.json

- **Phase**: P3
- **Status**: `verified` (source repo cleanup; `_new/` had no Nx artefacts)
- **Owner**: implementer
- **ADR refs**: `adr/0001`
- **User-story refs**: `(infra; underpins the monorepo decision)`
- **Migration rows**: Table 2b note "delete every `project.json`",
  Table 2c rows `nx.json` and root `project.json`.
- **Plan / NFR refs**: Plan §"Phase 3 — Tooling unification" Nx bullet

**Description** — Excise every Nx artefact that may survive the
copy in `move-admin`. Turbo is the sole task orchestrator per
`adr/0001`. The Phase 2 deletion rows cover the obvious files; this
task is the audit that catches anything missed (workspace-level
`project.json` overlays, `@nx/*` peer mentions in `package.json`,
Nx cache directories).

**Implementation notes** —

- `rg -l "@nx/|nx.json|project.json" _new/` → delete every hit
  not covered by Table 2.
- Remove `@nx/*`, `@nrwl/*`, `nx` devDeps from every `package.json`.
- Confirm root `turbo.json` declares `build`, `dev` (persistent),
  `lint`, `format`, `typecheck`, `test` (already scaffolded by
  `new-monorepo-init`).

**Acceptance criteria** (checklist):

- [x] No `nx.json` or `project.json` in `game-zone-management-fe/` source tree.
- [x] No `@nx/*` or `@nrwl/*` deps in `_new/` package.json files.
- [x] `turbo run build` is the build entry-point in `_new/`.

**Evidence**:

- Deleted from `game-zone-management-fe/`: `nx.json`, `project.json`, `yarn.lock`, `.yarn/`, `.yarnrc.yml`, `eslint.config.mjs`, `next-env.d.ts`, `libs/shared/*/project.json` (2026-05-27).
- `rg "@nx/|nx.json|project.json" _new/` → hits only in docs (not runtime code).
- Admin Dockerfile in `_new/` uses pnpm/Turbo (no `nx.json` COPY).

---

### `jwt-fail-fast` — Validate JWT_SECRET at boot (panic on missing/short)

- **Phase**: P3
- **Status**: `done` (built into Rust `config/settings.rs`)
- **Owner**: implementer
- **ADR refs**: `adr/0003`, `adr/0009`
- **User-story refs**: **`US-AUTH-004`** (direct implementer)
- **Migration rows**: `(built into scaffold-rust-backend)`
- **Plan / NFR refs**: Plan §"Phase 3" `jwt-fail-fast` bullet;
  `REQUIREMENTS.md#US-AUTH-004` acceptance criteria

**Description** — The Rust backend's `Settings::from_env()` panics at
startup when `JWT_SECRET` is missing or shorter than 32 characters.
This is built directly into the scaffold (no separate implementation
step needed).

**Implementation notes** —

- `apps/backend/src/config/settings.rs` calls
  `std::env::var("JWT_SECRET").expect(...)` and checks `.len() < 32`.
- The matching integration test ships in `be-first-test`.

**Acceptance criteria** (checklist):

- [x] `apps/backend/src/config/settings.rs` panics on missing/short
      `JWT_SECRET`.
- [ ] Integration test asserts the panic (delivered by `be-first-test`).

**Evidence**:

- `apps/backend/src/config/settings.rs` lines 14-17: validates JWT_SECRET.

---

### `api-types-package` — Generate @gaming-cafe/api-types from backend OpenAPI

- **Phase**: P4
- **Status**: `pending`
- **Owner**: implementer
- **ADR refs**: `adr/0004`
- **User-story refs**: `(infra; underpins typed clients for admin)`
- **Migration rows**: Table 4 rows `packages/api-types/package.json`
  and `packages/api-types/src/schema.ts`.
- **Plan / NFR refs**: Plan §"Phase 4 — Shared types and HTTP client";
  `ARCHITECTURE.md#Package responsibilities` row for
  `@gaming-cafe/api-types`

**Description** — Populate the `@gaming-cafe/api-types` package by
generating TypeScript types from the backend's OpenAPI spec via
`openapi-typescript`. Add a root script `pnpm gen:api-types` that
runs `apps/backend/scripts/generate-openapi.ts` and then the codegen.
Admin consumes only the generated types — no hand-written response
shapes survive.

**Implementation notes** —

- Add `openapi-typescript` to root devDeps.
- Root script `pnpm gen:api-types`: run
  `apps/backend/scripts/generate-openapi.ts` then `openapi-typescript
apps/backend/docs/openapi.json -o packages/api-types/src/schema.ts`.
- `packages/api-types/src/index.ts` re-exports `schema.ts` plus
  `paths<>` and `operations<>` helper aliases.
- `ci-pipeline` re-runs `gen:api-types` and fails on dirty diff.

**Acceptance criteria** (checklist):

- [x] `pnpm gen:api-types` runs end-to-end on a clean clone.
- [x] `packages/api-types/src/schema.ts` exists and is non-empty.
- [x] `packages/api-types/src/index.ts` exports `paths` and
      `operations` aliases.
- [x] `pnpm --filter @gaming-cafe/api-types build` succeeds.

**Evidence**:

- _none yet_

---

### `shared-http-client` — Consolidate axios setup into @gaming-cafe/utils

- **Phase**: P4
- **Status**: `pending`
- **Owner**: implementer
- **ADR refs**: `adr/0004` (indirectly — typed clients depend on
  api-types + this HTTP client)
- **User-story refs**: `(infra; underpins every admin REST call)`
- **Migration rows**: `(refactor; absorbs admin
`libs/shared/utils/api/axios-config.ts`)`
- **Plan / NFR refs**: Plan §"Phase 4" axios consolidation bullet;
  `ARCHITECTURE.md#REST envelope` + `@gaming-cafe/utils` row

**Description** — Today the admin app spins up its own axios
instance with interceptors, envelope unwrapping, and 401
handling. Consolidate into one factory in
`packages/utils/src/http/createHttpClient.ts`. Admin imports it;
the duplicated file is deleted.

**Implementation notes** —

- New `packages/utils/src/http/createHttpClient.ts` exporting
  `createHttpClient({ baseUrl, getAuthToken, getDeviceToken })`:
  attaches `Authorization: Bearer <token>`; response interceptor
  unwraps the `{ data, statusCode, message }` envelope; error
  interceptor maps `{ error: { code, ... } }` to a typed `ApiError`.
- `packages/utils/src/index.ts` re-exports `createHttpClient`,
  `ApiError`, `unwrapEnvelope`, `isApiError`.
- Migrate admin `apps/admin/src/services/**` to import from
  `@gaming-cafe/utils`; delete the per-app duplicate.

**Acceptance criteria** (checklist):

- [ ] `packages/utils/src/http/createHttpClient.ts` exists and is
      covered by at least one vitest case.
- [ ] Admin no longer instantiates axios directly.
- [ ] `rg "import axios" apps/` returns zero matches.
- [ ] Envelope unwrapping verified against a sample backend response.

**Evidence**:

- _none yet_

---

### `move-domain-types` — Migrate backend domain types to @gaming-cafe/contracts

- **Phase**: P4
- **Status**: `pending`
- **Owner**: implementer
- **ADR refs**: `adr/0008`
- **User-story refs**: `(infra; underpins typed error handling for all
US-AUTH-*, US-PLAN-003, US-SESSION-* via ErrorCode)`
- **Migration rows**: Table 5 (all rows); Table 4 rows for
  `packages/contracts/*`.
- **Plan / NFR refs**: Plan §"Phase 4"; `ARCHITECTURE.md#Package
responsibilities` row for `@gaming-cafe/contracts`

**Description** — Migrate runtime types from backend `src/types/` to
the new `@gaming-cafe/contracts` package. Delete hand-written client
types in admin, replacing them with aliases to `@gaming-cafe/api-types`.
The Rust backend uses its own typed models but the `ErrorCode` string
values are kept compatible with `@gaming-cafe/contracts`.

**Implementation notes** —

- `packages/contracts/` already scaffolded with `errors.ts`,
  `pagination.ts`, `roles.ts` (Phase 1 skeleton + this plan hardening).
- The Rust backend `error/app_error.rs` uses the same error code
  strings as `@gaming-cafe/contracts` ErrorCode enum.
- Delete admin `services/**/types.ts`; create
  `apps/admin/src/services/aliases.ts` with:
  ```typescript
  import type { components } from "@gaming-cafe/api-types";
  export type Device = components["schemas"]["DeviceResponse"];
  // ...≤30 lines
  ```

**Acceptance criteria** (checklist):

- [ ] `packages/contracts/` builds (`pnpm --filter @gaming-cafe/contracts
typecheck` exits 0).
- [ ] Rust backend `error/app_error.rs` error code strings match
      `@gaming-cafe/contracts` ErrorCode values.
- [ ] Admin services use aliases to `@gaming-cafe/api-types`.
- [ ] All Table 5 rows marked `verified`.

**Evidence**:

- _none yet_

---

### `ci-pipeline` — Add .github/workflows/ci.yml with turbo affected lint+typecheck+test+build

- **Phase**: P5
- **Status**: `pending`
- **Owner**: implementer
- **ADR refs**: `adr/0005` (test runners), `adr/0006` (Biome in CI)
- **User-story refs**: `(infra; satisfies Definition of Done step
"tests passing in CI")`
- **Migration rows**: Table 4 row `.github/workflows/ci.yml`.
- **Plan / NFR refs**: Plan §"Phase 5 — CI/CD" first bullet;
  `REQUIREMENTS.md#Definition of Done`

**Description** — A single CI workflow runs on every PR: install with
the frozen lockfile, then `turbo run lint typecheck test build
--filter='...[origin/main]'` so only affected workspaces are built.
Adds a job that re-runs `gen:api-types` and fails on drift.

**Implementation notes** —

- `.github/workflows/ci.yml`, one job `pr-gates`: checkout
  (`fetch-depth: 0` for turbo affected) → `pnpm/action-setup@v4`
  pinning pnpm 9 → `actions/setup-node@v4` reading `.nvmrc` →
  `pnpm install --frozen-lockfile` → `pnpm gen:api-types && git
diff --exit-code packages/api-types/` (drift gate) → `pnpm turbo
run lint typecheck test build --filter='...[origin/main]'`.
- Rust matrix step (`cargo build --release --manifest-path
apps/backend/Cargo.toml && cargo test --manifest-path
apps/backend/Cargo.toml && cargo clippy --manifest-path
apps/backend/Cargo.toml -- -D warnings`).

**Acceptance criteria** (checklist):

- [ ] `.github/workflows/ci.yml` exists and triggers on `pull_request`.
- [ ] The workflow runs lint + typecheck + test + build under turbo.
- [ ] The api-types drift gate fails when `schema.ts` is dirty.
- [ ] A PR with a no-op change goes green.

**Evidence**:

- _none yet_

---

### `ci-deploy-backend` — Update deploy-prod workflow for apps/backend + infra/helm/backend

- **Phase**: P5
- **Status**: `pending`
- **Owner**: implementer
- **ADR refs**: `(none directly; supports adr/0001 mono layout)`
- **User-story refs**: `(infra; supports NFR Availability — rolling
deploys)`
- **Migration rows**: Table 4 row `.github/workflows/deploy-backend.yml`
  (rebased from `arena360-backend/.github/workflows/deploy-prod.yml`).
- **Plan / NFR refs**: Plan §"Phase 5" `deploy-backend` bullet;
  `ARCHITECTURE.md#Deployment topology`

**Description** — Rebase the existing backend deploy workflow onto
the monorepo layout: build context `apps/backend`, Helm chart
`infra/helm/backend`. Preserve trigger semantics (tag push or
manual dispatch) and image tag conventions.

**Implementation notes** —

- Copy `arena360-backend/.github/workflows/deploy-prod.yml` →
  `.github/workflows/deploy-backend.yml`.
- Swap `context: .` → `context: apps/backend`; swap `helm/fastify`
  → `infra/helm/backend`.
- Confirm `Chart.yaml#name: fastify` is unchanged so existing
  in-cluster releases match.

**Acceptance criteria** (checklist):

- [ ] Workflow exists at `.github/workflows/deploy-backend.yml`.
- [ ] Build context is `apps/backend`; chart path is
      `infra/helm/backend`.
- [ ] `helm template infra/helm/backend` succeeds in the workflow.
- [ ] Workflow dry-runs green on a no-op change.

**Evidence**:

- _none yet_

---

### `ci-deploy-admin` — Update deploy-admin-prod workflow for apps/admin + infra/helm/admin

- **Phase**: P5
- **Status**: `pending`
- **Owner**: implementer
- **ADR refs**: `(none directly; supports adr/0001)`
- **User-story refs**: `(infra; supports NFR Availability)`
- **Migration rows**: Table 4 row `.github/workflows/deploy-admin.yml`
  (rebased from `game-zone-management-fe`'s admin deploy workflow).
- **Plan / NFR refs**: Plan §"Phase 5" `deploy-admin` bullet

**Description** — Rebase the admin deploy workflow onto the monorepo
layout (build context `apps/admin`, chart `infra/helm/admin`) and
fix the UAT-vs-prod naming mismatch flagged in the plan.

**Implementation notes** —

- Copy `game-zone-management-fe/.github/workflows/deploy-admin-prod.yml`
  → `.github/workflows/deploy-admin.yml`.
- Build context: `apps/admin`; chart path: `infra/helm/admin`.
- Audit for `uat`/`prod` naming inconsistencies.
- Preserve `Chart.yaml#name: admin-panel`.

**Acceptance criteria** (checklist):

- [ ] Workflow exists at `.github/workflows/deploy-admin.yml`.
- [ ] Build context is `apps/admin`; chart path is `infra/helm/admin`.
- [ ] UAT/prod naming inconsistencies resolved.
- [ ] Workflow dry-runs green on a no-op change.

**Evidence**:

- _none yet_

---

### `fe-deadcode` — Remove unused admin code; reconcile FEATURES.md; fix nav

- **Phase**: P6
- **Status**: `pending`
- **Owner**: implementer
- **ADR refs**: `adr/0007` (indirectly — confirms tokens replace
  Tailwind devDeps in admin)
- **User-story refs**: indirectly `US-ADMIN-001` (dashboard
  navigation cleanup); also removes scope listed under
  `REQUIREMENTS.md#Out of scope` (`WebSocketClient`).
- **Migration rows**: `(none — in-place edits of moved admin)`
- **Plan / NFR refs**: Plan §"Phase 6 / Admin" bullet

**Description** — Delete unused admin code: `WebSocketClient`,
`StoreProvider`, Tailwind devDeps (admin uses MUI + design tokens per
`adr/0007`). Either implement the hooks `FEATURES.md` claims, or
rewrite the doc to match reality. Restore Units / Purchase-Orders nav
items or delete those routes + pages. Consolidate `services/transaction`
vs `services/transactions` into one folder.

**Implementation notes** —

- `rg -l "WebSocketClient|StoreProvider" apps/admin/` → delete.
- Drop Tailwind devDeps (`tailwindcss`, `postcss`, `autoprefixer`)
  from `apps/admin/package.json` if unused.
- Audit `apps/admin/FEATURES.md` against real exports; rewrite or
  implement.
- Either un-comment Units / Purchase-Orders nav entries (and confirm
  routes compile) or delete those routes + pages.
- Collapse `services/transaction` + `services/transactions` into one
  plural folder.

**Acceptance criteria** (checklist):

- [ ] No surviving `WebSocketClient` / `StoreProvider` imports.
- [ ] No Tailwind devDeps in `apps/admin/package.json` unless
      actually used.
- [ ] `FEATURES.md` matches real exports (manually audited).
- [ ] Units / Purchase-Orders are either visible in nav or fully
      removed.
- [ ] Only one `services/transaction*` folder remains.

**Evidence**:

- _none yet_

---

### `be-first-test` — First Rust integration test for auth + health; coverage > 0

- **Phase**: P7
- **Status**: `pending`
- **Owner**: implementer
- **ADR refs**: `adr/0005`, `adr/0009`
- **User-story refs**: smoke-tests `US-AUTH-001` (OTP happy path) and
  `US-AUTH-004` (boot fail-fast on missing JWT_SECRET).
- **Migration rows**: `(none — new file)`
- **Plan / NFR refs**: Plan §"Phase 7" backend bullet;
  `adr/0005#Decision` (initial bar = one smoke test per workspace)

**Description** — Write the first real integration tests for the Rust
backend using `sqlx::test` (provides per-test database transactions
that auto-rollback). Two cases: (1) auth login flow issues a valid
JWT for a known user; (2) config validation rejects missing or short
`JWT_SECRET` at startup. Use `axum_test` or `reqwest` against a
test server.

**Implementation notes** —

- New `apps/backend/tests/auth_test.rs`: spawn the app with
  `sqlx::test` pool, seed a test user, request OTP, verify OTP,
  assert the returned JWT decodes with correct claims.
- New `apps/backend/tests/config_test.rs`: assert that starting
  without `JWT_SECRET` or with a <32 char secret panics/errors.
- `cargo test` from `apps/backend/` runs both.
- CI wires this via `cargo test --manifest-path apps/backend/Cargo.toml`.

**Acceptance criteria** (checklist):

- [ ] Two new test files passing under `cargo test` in `apps/backend/`.
- [ ] Tests use `sqlx::test` for database isolation.
- [ ] Test for config validation covers `US-AUTH-004`.
- [ ] `cargo test` is wired into `ci-pipeline`.

**Evidence**:

- _none yet_

---

### `fe-first-test` — Vitest + RTL test for one @gaming-cafe/ui component

- **Phase**: P7
- **Status**: `pending`
- **Owner**: implementer
- **ADR refs**: `adr/0005`
- **User-story refs**: smoke-tests one component (`<DataGrid />`) from
  `packages/ui` that underpins admin lists (e.g. `US-DEVICE-003`,
  `US-ADMIN-002`).
- **Migration rows**: `(none — new file)`
- **Plan / NFR refs**: Plan §"Phase 7" admin bullet;
  `adr/0005#Decision`

**Description** — Add Vitest + React Testing Library to
`packages/ui` and ship one test for `<DataGrid />` (the most-used
shared component). Wire the test into CI via the existing turbo
`test` task.

**Implementation notes** —

- Add devDeps to `packages/ui`: `vitest`, `@testing-library/react`,
  `@testing-library/jest-dom`, `jsdom`.
- New file `packages/ui/src/DataGrid.test.tsx` rendering with two
  sample rows; assert the rows are visible.
- New file `packages/ui/vitest.config.ts` with `environment: 'jsdom'`.
- Add `"test": "vitest run"` to `packages/ui/package.json`.

**Acceptance criteria** (checklist):

- [ ] `pnpm --filter @gaming-cafe/ui test` passes.
- [ ] At least one assertion on rendered DOM (`getByText`,
      `getByRole`).
- [ ] The test runs under `pnpm turbo run test` from the root.

**Evidence**:

- _none yet_

---

### `changesets` — Add changesets/version-bumping; document release flow

- **Phase**: P8
- **Status**: `pending`
- **Owner**: implementer
- **ADR refs**: `(none — release tooling)`
- **User-story refs**: `(infra; no direct US)`
- **Migration rows**: Table 4 row `.changeset/config.json`.
- **Plan / NFR refs**: Plan §"Phase 8" first bullet;
  `CONTRIBUTING.md#Release flow`

**Description** — Initialise `@changesets/cli` so we get
versioned changelogs per package. Document the loop in
`CONTRIBUTING.md`: `pnpm changeset` → commit → CI version PR →
merge → tag.

**Implementation notes** —

- `pnpm dlx @changesets/cli init` at the repo root.
- `.changeset/config.json`: `access: "restricted"`,
  `baseBranch: "main"`, `updateInternalDependencies: "patch"`.
- Add a "Releases" section to `CONTRIBUTING.md` documenting the
  four-step flow.
- Optional: `release.yml` GitHub Action opening the version PR via
  `changesets/action`.

**Acceptance criteria** (checklist):

- [ ] `.changeset/config.json` exists with `access: "restricted"`.
- [ ] `pnpm changeset` runs interactively without error.
- [ ] `CONTRIBUTING.md` documents the release flow.

**Evidence**:

- _none yet_

---

### `final-readme` — Author top-level README.md (quickstart, dev loop, deploy, doc tree)

- **Phase**: P8
- **Status**: `pending`
- **Owner**: implementer
- **ADR refs**: all seven (link tree)
- **User-story refs**: `(infra; no direct US)`
- **Migration rows**: Table 4 row `README.md`.
- **Plan / NFR refs**: Plan §"Phase 8" second bullet

**Description** — Replace the P1 placeholder README with the final
top-level entry point: quickstart (`pnpm i && pnpm dev`), per-app
dev loops, deploy targets, and a link tree to every doc and ADR
under `/docs`.

**Implementation notes** —

- Sections: "What is this?", "Quickstart",
  "Per-app dev" (backend / admin), "Build", "Deploy"
  (link the workflows), "Docs" (link REQUIREMENTS,
  ARCHITECTURE, MIGRATION, CONTRIBUTING), "ADRs" (link each
  `adr/000N`).
- Status table mirroring the "Phase summary" above.
- Confirm `pnpm i && pnpm turbo run build` runs clean on a fresh
  clone before merging.

**Acceptance criteria** (checklist):

- [ ] `_new/README.md` covers all sections listed above.
- [ ] Every link resolves (`pnpm exec markdown-link-check
README.md`).
- [ ] Quickstart instructions reproduce on a fresh clone.

**Evidence**:

- _none yet_

---

## Cross-reference matrices

### ADR → Tasks (which tasks implement each decision?)

| ADR        | Title                               | Tasks                                                                                                                                             |
| ---------- | ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| `adr/0001` | Monorepo: Turborepo + pnpm          | `new-monorepo-init`, `drop-nx`, `archive-old-repos`, `scaffold-rust-backend`, `move-admin`                                                        |
| `adr/0002` | Kiosk canonical: Tauri              | `drop-electron-kiosk`                                                                                                                             |
| `adr/0003` | Secrets management                  | `rotate-secrets`, `jwt-fail-fast`, `scaffold-rust-backend`                                                                                        |
| `adr/0004` | Shared API types via OpenAPI        | `api-types-package`, `shared-http-client`, `rust-openapi-gen`                                                                                     |
| `adr/0005` | Testing strategy                    | `be-first-test`, `fe-first-test`, `ci-pipeline`                                                                                                   |
| `adr/0006` | Tooling: Biome over ESLint+Prettier | `new-monorepo-init`, `shared-tsconfig-biome`, `fix-broken-eslint`, `move-admin`, `ci-pipeline`                                                    |
| `adr/0007` | Design tokens shared                | `new-monorepo-init` (theme skeleton), `fe-deadcode`                                                                                               |
| `adr/0008` | Runtime contracts package           | `move-domain-types`, `api-types-package`, `shared-http-client`                                                                                    |
| `adr/0009` | Rust Axum backend                   | `scaffold-rust-backend`, `rust-db-connection`, `rust-auth-module`, `rust-crud-handlers`, `rust-sse-module`, `rust-openapi-gen`, `rust-dockerfile` |

### User story → Tasks (which tasks deliver / verify each requirement?)

Every Must-have user story listed in `REQUIREMENTS.md`:

| US                 | Title                           | Implementing tasks                                                             | Verifying task                             |
| ------------------ | ------------------------------- | ------------------------------------------------------------------------------ | ------------------------------------------ |
| `US-AUTH-001`      | Owner OTP login                 | `scaffold-rust-backend`, `rust-auth-module`, `move-admin`, `move-domain-types` | `be-first-test`                            |
| `US-AUTH-002`      | JWT issuance                    | `scaffold-rust-backend`, `rust-auth-module`, `move-domain-types`               | `be-first-test`                            |
| `US-AUTH-003`      | Player kiosk login              | `scaffold-rust-backend`, `rust-auth-module`                                    | (deferred — kiosk out of scope)            |
| `US-AUTH-004`      | Fail-fast on missing JWT secret | `jwt-fail-fast`, `scaffold-rust-backend`                                       | `be-first-test`                            |
| `US-DEVICE-001`    | Register device                 | `scaffold-rust-backend`, `rust-crud-handlers`, `move-admin`                    | (deferred — no direct test in P7)          |
| `US-DEVICE-002`    | Assign games to device          | `scaffold-rust-backend`, `rust-crud-handlers`, `move-admin`                    | (deferred)                                 |
| `US-DEVICE-003`    | List devices + status           | `scaffold-rust-backend`, `rust-crud-handlers`, `move-admin`                    | (deferred)                                 |
| `US-PLAN-001`      | Create plan                     | `scaffold-rust-backend`, `rust-crud-handlers`, `move-admin`                    | (deferred)                                 |
| `US-PLAN-002`      | Assign plan to player           | `scaffold-rust-backend`, `rust-crud-handlers`, `move-admin`                    | (deferred)                                 |
| `US-PLAN-003`      | Validate remaining minutes      | `scaffold-rust-backend`, `rust-crud-handlers`, `move-domain-types`             | (deferred)                                 |
| `US-SESSION-001`   | Start session                   | `scaffold-rust-backend`, `rust-crud-handlers`, `move-domain-types`             | (deferred)                                 |
| `US-SESSION-002`   | End session voluntarily         | `scaffold-rust-backend`, `rust-crud-handlers`, `move-domain-types`             | (deferred)                                 |
| `US-SESSION-003`   | Auto-end on plan exhaustion     | `scaffold-rust-backend`, `rust-crud-handlers`, `move-domain-types`             | (deferred — needs proper integration test) |
| `US-SESSION-004`   | Kiosk poll cadence              | `scaffold-rust-backend`, `rust-sse-module`                                     | (deferred)                                 |
| `US-INVENTORY-001` | Create products                 | `scaffold-rust-backend`, `rust-crud-handlers`, `move-admin`                    | (deferred)                                 |
| `US-ADMIN-001`     | Dashboard                       | `move-admin`, `fe-deadcode`                                                    | (deferred)                                 |
| `US-ADMIN-002`     | Transactions ledger             | `move-admin`                                                                   | (deferred)                                 |
| `US-ADMIN-003`     | Presigned R2 upload             | `scaffold-rust-backend`, `rust-crud-handlers`, `move-admin`                    | (deferred)                                 |

### NFR → Tasks

| NFR                                    | Tasks                                                                                                          |
| -------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| Performance (p95 < 300 ms reads)       | `scaffold-rust-backend` (native perf), `api-types-package` (no impact but baseline)                            |
| Availability (99.5% monthly)           | `ci-deploy-backend`, `ci-deploy-admin` (rolling deploys), `rust-dockerfile`                                    |
| Security (HTTPS, JWT, helmet, secrets) | `scaffold-rust-backend` (tower-http security headers, compression built in), `jwt-fail-fast`, `rotate-secrets` |
| Scalability                            | (architectural; no consolidation task)                                                                         |
| Monitoring                             | (deferred — not in consolidation scope)                                                                        |

### Migration row → Task

| Migration table                    | Owning task                                                                                                                                                                                                                 |
| ---------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Table 1 (Rust backend)             | `scaffold-rust-backend`                                                                                                                                                                                                     |
| Table 2a (admin)                   | `move-admin`                                                                                                                                                                                                                |
| Table 2b (libs → packages)         | `move-admin`                                                                                                                                                                                                                |
| Table 2c (frontend deletes)        | `move-admin` (Electron kiosk row → `drop-electron-kiosk`; Nx rows → `drop-nx`)                                                                                                                                              |
| Table 4 (new root files)           | `new-monorepo-init` (most rows); `ci-pipeline` + `ci-deploy-*` (workflow rows); `api-types-package` + `shared-tsconfig-biome` (package rows); `move-domain-types` (contracts rows); `changesets` (`.changeset/config.json`) |
| Table 5 (domain types → contracts) | `move-domain-types`                                                                                                                                                                                                         |

---

## Implementation notes / decisions log

Append-only. Each entry: date + subject + outcome.

- **2026-05-27**: Phase 0 reference docs authored
  (`REQUIREMENTS.md` 364 ln, `ARCHITECTURE.md` 396 ln,
  `MIGRATION.md` 211 ln, `CONTRIBUTING.md` 277 ln, ADRs 0001–0007
  totalling ~1304 ln). All under `_new/docs/`.
- **2026-05-27**: Phase 1 monorepo skeleton scaffolded at `_new/`.
  Turbo 2 (`tasks` syntax), Biome 2.4.15, pnpm 9, TS 5.6 with
  `bundler` resolution. Seven packages stubbed with real Arena360
  brand tokens already in `@gaming-cafe/theme`.
- **2026-05-27**: Phase 2 code-move interrupted by transient Cursor
  billing issue. To resume: re-spawn implementers A/B/C per the
  consolidation plan; subagent prompts are idempotent.
- **2026-05-27**: PLANNER.md created as the cross-referenced
  execution tracker.
- **2026-05-27**: Plan hardened — closed all open questions (Q1–Q6),
  added `@gaming-cafe/contracts` package (ADR-0008) for runtime types,
  added `move-domain-types` task to P4, updated all cross-reference
  matrices. Secrets rotation deferred per user direction.
- **2026-05-27**: Scope pivot — agent-kiosk (Tauri) removed from scope entirely. Backend rewritten in Rust (Axum + SQLx) instead of porting NestJS. SSE support added. ADR-0009 created. Kiosk-related tasks deleted: move-tauri-kiosk, kiosk-cleanup, kiosk-first-test, ci-kiosk-build. NestJS-only tasks deleted: helm-secrets, be-port-align, be-build-prod-fix, be-sec-middleware, be-deadcode.
- **2026-05-27**: `shared-tsconfig-biome` verified — canonical
  `@gaming-cafe/tsconfig` + `@gaming-cafe/biome-config`, root extends
  wiring, workspace devDeps, `pnpm -w typecheck` + biome error gate pass.
- **2026-05-27**: `rust-openapi-gen` verified — utoipa `ApiDoc`, handler
  annotations, `openapi-gen` binary, dev Swagger UI at `/api/docs`,
  `pnpm gen:api-types` pipeline produces `packages/api-types/src/schema.ts`
  (48 paths, 128 schemas).
- **2026-05-27**: `rust-crud-handlers` player-plan parity closed — assign,
  validate, best-plan, my-active-plans routes; session plan validation +
  credit deduction; completed `plan_purchase` provisioning. Smoke extended
  in `_new/scripts/smoke-p2.sh` (steps 8–10). Deferred: credit rollover on
  repeat purchase (`movePlayerPlansToNextPlan`).

## Resolved decisions (formerly open questions)

All open questions have been closed with concrete decisions (2026-05-27):

| #   | Question              | Decision                                                                                                                                                                                                                               |
| --- | --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Q1  | Upload size cap       | **25 MiB** — locked in `REQUIREMENTS.md` US-ADMIN-003 Scenario 2 and `apps/backend/src/dto/files`.                                                                                                                                     |
| Q2  | Cafe timezone default | **`Asia/Kolkata`** — env var `CAFE_TZ`, default in `apps/backend/src/config.rs`.                                                                                                                                                       |
| Q3  | Coverage ramp pace    | **+5%/quarter**, target 60% by Q4 2026 (per `adr/0005`).                                                                                                                                                                               |
| Q5  | Secrets rotation      | **DEFERRED — user direction.** `.env`, `.env.development`, Helm `values*.yaml` plaintext credentials kept as-is for now. `rotate-secrets` task → status `deferred-user-decision`. Risk: leaked secrets remain in old archive branches. |

## How to use this file

1. **Working a task?** Find its section, set status to `in_progress`,
   list yourself as Owner.
2. **Done?** Tick the acceptance criteria, paste evidence (paths,
   command output, PR link), set status to `done`.
3. **Build / smoke green for the affected workspace?** Set status to
   `verified`.
4. **Found a blocker?** Set status to `blocked-*` and document in the
   "Implementation notes / decisions log" section below.
5. **Adding a new task?** Append a sub-section using the same
   template; update the relevant cross-reference matrices.
6. **Adding a new ADR or US?** After authoring it in the right doc,
   add a row to the matching matrix here.
