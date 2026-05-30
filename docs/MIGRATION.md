# MIGRATION: game-zone-management-fe → gaming-cafe (+ Rust Axum backend rewrite)

> **Update statuses as moves complete: `pending` → `moved` → `verified`.**
>
> `moved` = files exist at the new path.
> `verified` = the target workspace builds (`pnpm --filter <pkg> build`)
> and any smoke test passes.
>
> Last updated: 2026-05-27.

This is the per-file execution checklist for Phase 2 of the
consolidation plan. The NestJS backend (`arena360-backend/`) is being
**replaced** with a fresh Rust (Axum + SQLx) backend rather than migrated
file-by-file, and the Tauri kiosk (`agent-kiosk/`) has been removed from
scope entirely. The frontend repo (`game-zone-management-fe/`) is still
migrated into the new monorepo root (`/Users/harishmahamure/gaming-cafe/`,
authored from `_new/` until cutover).

---

## 1. Rust backend at `apps/backend/`

The NestJS backend (`arena360-backend/`) is NOT being migrated file-by-file.
Instead, a fresh Rust (Axum + SQLx) backend is written from scratch at
`apps/backend/`, connecting to the same existing Postgres database.
See ADR-0009 for rationale.

| Target path | Purpose | Status |
|---|---|---|
| `apps/backend/Cargo.toml` | Rust package manifest with axum, sqlx, utoipa deps | verified |
| `apps/backend/src/main.rs` | Entry point: Axum server, routes, middleware | verified |
| `apps/backend/src/lib.rs` | Library root | verified |
| `apps/backend/src/config/` | Settings from env vars, SQLx pool setup | verified |
| `apps/backend/src/handlers/` | HTTP handlers (auth, users, devices, games, plans, sessions, etc.) | verified (partial: auth, stats, devices, sse) |
| `apps/backend/src/services/` | Business logic layer | verified (partial) |
| `apps/backend/src/repositories/` | Data access via SQLx queries | verified (partial) |
| `apps/backend/src/models/` | Domain models (SQLx FromRow) | verified (partial) |
| `apps/backend/src/dto/` | Request/Response DTOs (serde + utoipa) | verified |
| `apps/backend/src/middleware/` | Auth JWT extraction, logging, error handling | verified |
| `apps/backend/src/error/` | AppError enum, IntoResponse impl | verified |
| `apps/backend/src/sse/` | Server-Sent Events broadcaster + event types | verified |
| `apps/backend/Dockerfile` | Multi-stage Rust build | moved |
| `apps/backend/.env.example` | Environment variable template | moved |
| `apps/backend/migrations/README.md` | Note: using existing TypeORM schema | verified |

---

## 2. `game-zone-management-fe/` → `apps/admin/` + `packages/{ui,theme,providers,utils}/`

Drop the Nx + Next.js leftovers entirely. Rewrite each `package.json#name`
to the `@gaming-cafe/*` scope. Preserve `Chart.yaml#name: admin-panel`
in the relocated Helm chart.

### 2a. `apps/admin`

| Source path | Target path | Status |
|---|---|---|
| `game-zone-management-fe/apps/admin/src/` | `apps/admin/src/` (update imports from `@admin-panel/shared-*` → `@gaming-cafe/*`) | verified |
| `game-zone-management-fe/apps/admin/public/` | `apps/admin/public/` | verified |
| `game-zone-management-fe/apps/admin/index.html` | `apps/admin/index.html` | verified |
| `game-zone-management-fe/apps/admin/vite.config.ts` | `apps/admin/vite.config.ts` (drop `@nx/vite` plugin) | verified |
| `game-zone-management-fe/apps/admin/tsconfig.json` | `apps/admin/tsconfig.json` (extend `@gaming-cafe/tsconfig/react.json`) | verified |
| `game-zone-management-fe/apps/admin/tsconfig.app.json` | `apps/admin/tsconfig.app.json` | pending (not copied; single tsconfig used) |
| `game-zone-management-fe/apps/admin/tsconfig.spec.json` | `apps/admin/tsconfig.spec.json` | pending (not copied) |
| `game-zone-management-fe/apps/admin/Dockerfile` | `apps/admin/Dockerfile` | moved |
| `game-zone-management-fe/apps/admin/docker/` | `apps/admin/docker/` (nginx conf, entrypoint) | pending (not in source tree) |
| `game-zone-management-fe/apps/admin/helm/admin-panel/` | `infra/helm/admin/` (preserve `Chart.yaml#name: admin-panel`) | verified |
| `game-zone-management-fe/package.json` (root) | `apps/admin/package.json` (extract admin-specific deps; rename to `@gaming-cafe/admin`; drop `@nx/*`, `next`, `eslint-config-next`) | verified |
| `game-zone-management-fe/apps/admin/.github/workflows/deploy-admin-prod.yml` (or equivalent at repo root) | `.github/workflows/deploy-admin.yml` (rebased) | pending |

### 2b. `packages/{ui,theme,providers,utils}`

| Source path | Target path | Status |
|---|---|---|
| `game-zone-management-fe/libs/shared/ui/` | `packages/ui/` (rename `package.json#name` → `@gaming-cafe/ui`) | verified |
| `game-zone-management-fe/libs/shared/theme/` | `packages/theme/` (rename → `@gaming-cafe/theme`; add `tokens.css` emit per ADR 0007) | verified |
| `game-zone-management-fe/libs/shared/providers/` | `packages/providers/` (rename → `@gaming-cafe/providers`) | verified |
| `game-zone-management-fe/libs/shared/utils/` | `packages/utils/` (rename → `@gaming-cafe/utils`; absorb axios setup from admin + kiosk per ADR plan Phase 4) | verified |
| `game-zone-management-fe/libs/shared/*/project.json` | **DELETE** (Nx-specific) | verified |

### 2c. Files to DELETE outright

| Source path | Reason | Status |
|---|---|---|
| `game-zone-management-fe/nx.json` | Nx dropped (ADR 0001) | verified |
| `game-zone-management-fe/next-env.d.ts` | Next.js never actually used | verified |
| `game-zone-management-fe/apps/kiosk/**` | Electron kiosk dropped (ADR 0002) | verified |
| `game-zone-management-fe/eslint.config.mjs` | Replaced by Biome (ADR 0006) | verified |
| `game-zone-management-fe/.eslintrc*`, `.prettierrc*` (wherever) | Replaced by Biome | verified |
| `game-zone-management-fe/yarn.lock` | Replaced by `pnpm-lock.yaml` | verified |
| `game-zone-management-fe/.yarn/`, `.yarnrc.yml` | Yarn dropped | verified |
| `game-zone-management-fe/project.json` (root) | Nx-specific | verified |

**Note (2026-05-27):** `_new/apps/` and `_new/packages/` source no longer
contain `eslint-disable*` comments or ESLint/Prettier config files;
Biome is the sole lint/format tool (see PLANNER `fix-broken-eslint`).

---

## 4. New files to create at the monorepo root

These are not present in any source repo. Author them in Phase 1
before the moves in Phase 2.

| Target path | Purpose | Status |
|---|---|---|
| `package.json` (root) | `"packageManager": "pnpm@9.x"`, root scripts (`dev`, `build`, `lint`, `format`, `typecheck`, `test`, `gen:api-types`), devDeps (`turbo`, `typescript`, `@biomejs/biome@^2.4.15`, `@changesets/cli`, `openapi-typescript`) | pending |
| `pnpm-workspace.yaml` | `packages: ['apps/*', 'packages/*']` | pending |
| `turbo.json` | Pipelines for `build`, `dev` (persistent), `lint`, `format`, `typecheck`, `test`; `outputs` per task | pending |
| `biome.json` | Re-exports `@gaming-cafe/biome-config`; project-wide includes/excludes | done |
| `tsconfig.base.json` | Shared TS compiler options consumed via `extends` from per-package tsconfigs | done |
| `.gitignore` | `node_modules`, `dist`, `.turbo`, `.env*`, `target/`, `coverage/`, `.vscode/`, `.DS_Store`, `apps/kiosk/src-tauri/target/`, `apps/kiosk/src-tauri/gen/`, `apps/admin/dist/`, etc. | verified |
| `.npmrc` | `strict-peer-dependencies=true`, `auto-install-peers=true`, `engine-strict=true` | pending |
| `.nvmrc` | `20` (matches backend Dockerfile) | pending |
| `README.md` | Quickstart, dev loop, links to `docs/*`, deploy targets, support matrix | pending |
| `.github/workflows/ci.yml` | PR gates: `pnpm i --frozen-lockfile` → `turbo run lint typecheck test build --filter='...[origin/main]'` | pending |
| `.github/workflows/deploy-backend.yml` | Rebased from `arena360-backend/.github/workflows/deploy-prod.yml`; build context `apps/backend`, chart `infra/helm/backend` | pending |
| `.github/workflows/deploy-admin.yml` | Rebased from `game-zone-management-fe`'s admin deploy workflow; build context `apps/admin`, chart `infra/helm/admin` | pending |
| `.github/workflows/release-kiosk.yml` | New — Tauri Windows build on tag push; signed artefact attached to GitHub Release | pending |
| `packages/api-types/package.json` | `@gaming-cafe/api-types`, exports `schema.ts` | pending |
| `packages/api-types/src/schema.ts` | Generated by `pnpm gen:api-types`; tracked but regenerated in CI | pending |
| `packages/biome-config/package.json` | `@gaming-cafe/biome-config`, exports `biome.json` preset | done |
| `packages/biome-config/biome.json` | Shared lint + format rules (per ADR 0006) | done |
| `packages/tsconfig/package.json` | `@gaming-cafe/tsconfig` | done |
| `packages/tsconfig/base.json` | Shared base options | done |
| `packages/tsconfig/node.json` | Node-targeted options | done |
| `packages/tsconfig/react.json` | React + DOM lib options | done |
| `packages/tsconfig/nest.json` | Nest-specific options (decorators, emitDecoratorMetadata) | done |
| `packages/theme/scripts/emit-tokens-css.ts` | Generates `dist/tokens.css` from the TS token map (per ADR 0007) | pending |
| `packages/contracts/package.json` | `@gaming-cafe/contracts`, exports runtime enums and shared types (per ADR 0008) | pending |
| `packages/contracts/tsconfig.json` | Extends `@gaming-cafe/tsconfig/base.json` | done |
| `packages/contracts/src/index.ts` | Re-exports all contract modules | pending |
| `packages/contracts/src/errors.ts` | `ErrorCode` enum + `ErrorMessages` map | pending |
| `packages/contracts/src/pagination.ts` | `IPaginationParams`, `IPaginationResult<T>`, `SortOrder`, `IQueryOptions` | pending |
| `packages/contracts/src/roles.ts` | `UserRole` type + `UserStatus` enum | pending |
| `packages/contracts/README.md` | Package documentation | pending |
| `apps/kiosk/.npmrc` | `node-linker=hoisted` for Tauri compatibility (per Q4 resolution) | verified |
| `.changeset/config.json` | Default config; `access: "restricted"` (private packages) | pending |
| `docs/proposals/.gitkeep` | Keep the folder even before any proposal is added | pending |

---

## 5. Backend domain types → `packages/contracts/`

These types from `arena360-backend/src/types/` are migrated to
`packages/contracts/` so they can be shared across backend, admin, and
kiosk. See ADR-0008 for rationale.

| Source path | Target path | Status |
|---|---|---|
| `arena360-backend/src/types/error-codes.types.ts` | `packages/contracts/src/errors.ts` (content merged + `FILE_TOO_LARGE` added) | pending |
| `arena360-backend/src/types/common.types.ts` (pagination types only) | `packages/contracts/src/pagination.ts` | pending |
| `arena360-backend/src/types/user.types.ts` (`UserRole`, `UserStatus` only) | `packages/contracts/src/roles.ts` | pending |
| `arena360-backend/src/types/fastify-req-with-user.types.ts` | **N/A** — Rust backend uses `Claims` struct in `middleware/auth.rs` | n/a |
| `arena360-backend/src/types/plan.types.ts` | **N/A** — Rust backend uses typed models in `models/` | n/a |
| `arena360-backend/src/types/common.types.ts` (`IApiResponse`, `IErrorResponse`, `IBaseEntity`) | **N/A** — Rust backend uses `ApiResponse<T>` and `AppError` in `error/` | n/a |
| `game-zone-management-fe/apps/admin/src/services/**/types.ts` | **DELETE** (replace with aliases to `@gaming-cafe/api-types` in `apps/admin/src/services/aliases.ts`) | pending |

**Note**: After migration, backend imports `ErrorCode` from
`@gaming-cafe/contracts` instead of defining it locally. A temporary
re-export shim is used during transition, then deleted.

---

## Move sequence

1. **Phase 1 prerequisite**: source repos archived to backup branches/tags.
2. Scaffold root files (table 4) in `_new/`.
3. Scaffold Rust backend (table 1).
4. Move admin app (table 2a).
5. Move shared libs (table 2b).
6. Delete dropped frontend files (table 2c).
7. From the new root: `pnpm install` once; `turbo run build` for every
   workspace.
8. Update each row to `moved`; after `turbo run build` + smoke test
   passes for the affected workspace, update to `verified`.
9. Once every row in tables 1–2 is `verified`, the source repo
   directories under `/Users/harishmahamure/gaming-cafe/{arena360-backend,
   game-zone-management-fe}/` are deleted and the `_new/`
   contents are promoted to the repo root.

---

## Verification checklist (per workspace)

For each `apps/<x>` and `packages/<y>`, mark `verified` only after:

- [ ] `pnpm install` at the root succeeds with no peer-dep warnings.
- [ ] `pnpm --filter @gaming-cafe/<x> build` succeeds.
- [ ] `pnpm --filter @gaming-cafe/<x> typecheck` succeeds.
- [ ] `pnpm --filter @gaming-cafe/<x> lint` succeeds (Biome).
- [ ] The smoke test (one per workspace per ADR 0005) passes.
- [ ] For apps: a manual local boot (`pnpm dev`) renders the home
      screen / responds to `/health`.
- [ ] For `packages/contracts`: `pnpm --filter @gaming-cafe/contracts typecheck` succeeds.
- [ ] For `apps/backend` (Rust): `cargo build --release` succeeds and `cargo test` passes.
- [ ] For `apps/backend`: regenerated `docs/openapi.json` is identical
      to the version that produced the current
      `packages/api-types/src/schema.ts` (no drift).

---

## Rollback notes

- Source repos remain available on remote `archive/<date>` branches +
  tags. Any file can be retrieved from there.
- The Helm `Chart.yaml#name` fields (`fastify`, `admin-panel`) are
  preserved, so re-deploying the old layout against the same cluster
  would still target the same in-cluster releases.
- No database schema changes happen in this migration. The Rust backend
  connects to the same existing Postgres schema via SQLx.
