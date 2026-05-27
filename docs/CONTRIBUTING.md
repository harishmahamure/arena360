# CONTRIBUTING

Welcome to the Arena360 gaming-cafe monorepo. This document is the
contract for how we develop, test, review, and release.

Read `docs/ARCHITECTURE.md` for the why; this file is the how.

---

## Prerequisites

You need the following installed locally:

- **Node 20** (matches `.nvmrc` and the backend Dockerfile). Use
  `nvm install` / `nvm use` in this repo or any equivalent version
  manager.
- **Corepack** (ships with Node 20): `corepack enable` once, after
  installing Node. This pins `pnpm` to the version declared in the root
  `package.json` `packageManager` field.
- **pnpm 9.x** (auto-installed via Corepack on first run).
- **Rust toolchain** (for backend and kiosk work):
  - `rustup` with the stable channel (`apps/backend/rust-toolchain.toml`
    pins stable + rustfmt + clippy).
  - Platform Tauri prerequisites for kiosk — see
    <https://tauri.app/start/prerequisites/>.
  - On Windows: WebView2 runtime (ships with Windows 11; install
    manually on Windows 10).
- **Docker** (only for building backend / admin images locally).
- **Postgres 15+** (only for backend dev; or use `docker compose up db`).

Verify your setup:

```bash
node -v       # v20.x
corepack -v   # any
pnpm -v       # 9.x
rustc --version  # needed for backend and kiosk work
```

---

## IDE setup

Cursor and VS Code share the same config under `.vscode/`:

1. **Extensions** — accept the recommended-extensions prompt on first
   open (Biome, rust-analyzer, CodeLLDB, Even Better TOML, etc.).
2. **Settings** — copy the shared preset into your local (gitignored)
   file:
   ```bash
   cp .vscode/settings.recommended.json .vscode/settings.json
   ```
   The preset wires Biome for TS/JS and `rust-analyzer.linkedProjects`
   so the backend crate at `apps/backend/Cargo.toml` is discovered from
   the monorepo root.
3. **Rust tasks** — run **Tasks: Run Task** for `rust: cargo build`,
   `rust: cargo test`, or `rust: cargo clippy` (all run in
   `apps/backend/`).
4. **Backend debugging** — create `apps/backend/.env` from
   `.env.example`, install CodeLLDB, then use **Run and Debug → Debug
   backend (gaming-cafe-api)**.

---

## First-time setup

```bash
git clone <repo-url> gaming-cafe
cd gaming-cafe
corepack enable
pnpm install
cp apps/backend/env.template apps/backend/.env
# edit apps/backend/.env with local DB + JWT_SECRET (>= 32 chars)
pnpm gen:api-types   # ensures packages/api-types/src/schema.ts is fresh
```

`.env` files are gitignored everywhere (see
`docs/adr/0003-secrets-management.md`). The only tracked env file is
`apps/backend/env.template`.

---

## Day-to-day dev commands

All commands are run from the **repo root** unless noted.

### Run everything together

```bash
pnpm dev        # turbo run dev — starts backend, admin, and shared package watchers
```

### Run one workspace at a time

```bash
pnpm --filter @gaming-cafe/backend dev
pnpm --filter @gaming-cafe/admin   dev
pnpm --filter @gaming-cafe/kiosk   tauri:dev
```

`tauri:dev` boots the Vite dev server **and** the Rust shell with
hot-reload. It is the kiosk equivalent of `pnpm dev`.

### Targeted package work

```bash
pnpm --filter @gaming-cafe/ui    dev   # tsup watch
pnpm --filter @gaming-cafe/theme build # one-shot rebuild + emit tokens.css
```

`packages/api-types` is **generated**, not hand-edited. Regenerate with
`pnpm gen:api-types` (see below) whenever the backend OpenAPI spec
changes.

---

## Quality gates

Run before pushing. CI runs the same commands.

```bash
pnpm lint        # biome check across the workspace
pnpm format      # biome format --write across the workspace
pnpm typecheck   # tsc --noEmit per workspace (via turbo)
pnpm test        # vitest/jest/cargo across the workspace (via turbo)
pnpm build       # turbo run build — every workspace
```

Per-workspace variants follow the same pattern:

```bash
pnpm --filter @gaming-cafe/backend test         # jest
pnpm --filter @gaming-cafe/admin   test         # vitest
pnpm --filter @gaming-cafe/kiosk   test         # vitest (React)
pnpm --filter @gaming-cafe/kiosk   test:rust    # cargo test in src-tauri
pnpm --filter @gaming-cafe/ui      test         # vitest + RTL
```

See `docs/adr/0005-testing-strategy.md` for what each test runner is
responsible for and the coverage ramp.

---

## Generating shared API types

`packages/api-types` is the typed surface of the backend's REST API.
It is generated; never edit `src/schema.ts` by hand.

```bash
pnpm gen:api-types
```

What this does:

1. Runs `cargo run --bin openapi-gen` in `apps/backend` (via
   `pnpm run openapi:generate`) to emit `apps/backend/docs/openapi.json`
   from Rust handler `#[utoipa::path]` annotations.
2. Pipes that through `openapi-typescript` into
   `packages/api-types/src/schema.ts`.
3. Runs `pnpm --filter @gaming-cafe/api-types build`.

CI runs the same command and fails the PR if the generated file does
not match what is committed (drift check — see ADR 0004).

---

## Release flow (changesets)

We use [changesets](https://github.com/changesets/changesets) for
versioning. Releases are PR-driven, never manual `npm publish`.

1. Make your code change on a feature branch.
2. Run `pnpm changeset` and answer the prompts: which packages
   changed, whether it is a `patch`, `minor`, or `major`, and write a
   one-line summary. This creates a markdown file in `.changeset/`.
3. Commit the changeset file with the code change.
4. Open a PR into `main`. CI runs lint + typecheck + test + build.
5. Once merged, the changesets GitHub Action opens (or updates) a
   **Version Packages** PR that bumps versions in `package.json` files
   and updates each affected `CHANGELOG.md`.
6. Merge the Version Packages PR. CI tags the release and (for apps)
   triggers the corresponding deploy workflow.

Most internal packages stay `private: true`; their changesets exist
purely to keep an auditable CHANGELOG.

---

## Branching and commits

- **Trunk-based**: `main` is the only long-lived branch. Feature
  branches are short-lived and merge into `main` via PR.
- **No direct pushes to `main`**: enforced by branch protection.
- **CI must pass** before merge. Reviewers can approve but not bypass.
- **Squash-merge by default.** The PR title becomes the squash commit;
  write it in conventional-commit form.

### Conventional commits

Format: `type(scope): summary`

Types we use:

| Type | Meaning |
|---|---|
| `feat` | New user-visible behaviour |
| `fix` | Bug fix |
| `refactor` | No behaviour change |
| `perf` | Performance-only change |
| `docs` | Documentation only |
| `test` | Tests only |
| `chore` | Build, deps, scaffolding |
| `ci` | CI/CD config only |

Scope is the workspace short name: `backend`, `admin`, `kiosk`, `ui`,
`theme`, `providers`, `utils`, `api-types`, `infra`, or `root`.

Example: `feat(backend): validate JWT_SECRET at boot (US-AUTH-004)`.

Reference the user-story ID in the body when it applies.

---

## Adding a new package

1. Decide: app or library?
   - **App** → `apps/<name>/` (consumes packages; never consumed by them).
   - **Library** → `packages/<name>/` (consumable by any app; no app
     imports).
2. `mkdir packages/<name>` and create `package.json` with:
   - `"name": "@gaming-cafe/<name>"`
   - `"private": true` unless we intend to publish externally
   - `"type": "module"`
   - `"main"`, `"types"`, `"exports"` pointing at the build output
3. Add a `tsconfig.json` extending the right preset from
   `@gaming-cafe/tsconfig` (`base`, `node`, `react`, or `nest`).
4. Add a `vitest.config.ts` (or `jest.config.ts` for Nest) and one
   smoke test in `src/__tests__/smoke.test.ts`.
5. Wire scripts: `build`, `dev`, `lint`, `typecheck`, `test`,
   `clean`. Turborepo discovers them automatically through the root
   pipeline.
6. Update `docs/ARCHITECTURE.md` "Package responsibilities" table.
7. Run `pnpm install` at the root so pnpm links the workspace.
8. Add a changeset (`pnpm changeset`) describing the new package.

---

## Adding a new app

Same as a package, with these extras:

1. The folder lives under `apps/<name>/`, not `packages/`.
2. The app may import any `@gaming-cafe/<pkg>` but **must not** import
   from another app.
3. If the app is deployable, add an `infra/helm/<name>/` chart and a
   `.github/workflows/deploy-<name>.yml` workflow.
4. Update `docs/ARCHITECTURE.md` component diagram.

---

## Code style

- Formatting and linting are enforced by **Biome ^2.4.15** (one tool,
  no ESLint + Prettier split — see `docs/adr/0006-tooling-biome.md`).
- Run `pnpm format` before pushing; `pnpm lint` will block the PR
  otherwise.
- Follow the standards in
  `/Users/harishmahamure/.cursor/rules/production-grade.mdc` — KISS,
  DRY, SOLID — and the API design rules in
  `/Users/harishmahamure/.cursor/rules/04-api-design.mdc` for any HTTP
  surface.

---

## Working with the agent

Project-local Cursor rules live in `.cursor/rules/`:

| Rule | Scope |
|---|---|
| `00-standards.mdc` | Always — KISS/DRY/SOLID + project constraints |
| `10-typescript.mdc` | `apps/admin`, `packages/*`, `scripts/` |
| `11-rust.mdc` | `apps/backend/**/*.rs` |
| `20-adr-discipline.mdc` | Always — ADR workflow |

When the agent detects an **architectural change** (stack swap, new
dependency, API envelope change, infra edit, etc.), it must:

1. Stop before writing code.
2. Draft `docs/adr/DRAFT-NNNN-kebab-title.md` with `Status: Proposed`.
3. Append an event to `docs/adr/.events.jsonl` (gitignored, local only).
4. Ask for your approval before implementing.

A Cursor hook (`.cursor/hooks.json`) also logs edits to
architecture-sensitive paths independently. Restart Cursor after cloning
if hooks do not load.

---

## Reporting bugs and proposing features

- **Bug**: open an issue with reproduction steps, expected vs actual,
  and the relevant `requestId` (visible in error toasts and backend
  logs).
- **Feature**: open a PR against `docs/REQUIREMENTS.md` adding a
  `US-<AREA>-<NNN>` story with acceptance criteria. Once merged, that
  story can be picked up for implementation.
- **Architectural change**: open a PR adding a new ADR under
  `docs/adr/`. Use the template at the top of any existing ADR.

---

## Quick reference

| Task | Command |
|---|---|
| Install deps | `pnpm install` |
| Run everything | `pnpm dev` |
| Run one workspace | `pnpm --filter @gaming-cafe/<name> dev` |
| Kiosk dev (Tauri + Vite) | `pnpm --filter @gaming-cafe/kiosk tauri:dev` |
| Format | `pnpm format` |
| Lint | `pnpm lint` |
| Typecheck | `pnpm typecheck` |
| Test | `pnpm test` |
| Build | `pnpm build` |
| Regenerate API types | `pnpm gen:api-types` |
| Create changeset | `pnpm changeset` |
