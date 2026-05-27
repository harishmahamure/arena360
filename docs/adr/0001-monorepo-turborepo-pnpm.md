# ADR-0001: Monorepo on Turborepo + pnpm

**Status**: Accepted
**Date**: 2026-05-27
**Deciders**: Platform team (gaming-cafe consolidation working group)

## Context

We are consolidating three sibling repositories —
`arena360-backend` (NestJS + Fastify),
`game-zone-management-fe` (Nx 20 + Yarn workspace housing the admin
SPA, Electron kiosk, and shared libraries), and
`agent-kiosk` (Tauri 2 + React 19 desktop kiosk) — into a single
monorepo at `/Users/harishmahamure/gaming-cafe/`.

Each repo currently uses a different toolchain:

| Repo | Workspace tool | Package manager | Task runner |
|---|---|---|---|
| `arena360-backend` | none (single project) | npm + bun lockfiles present | npm scripts |
| `game-zone-management-fe` | Nx 20 | Yarn 1 (`yarn.lock`) | `nx run-many` |
| `agent-kiosk` | none (single project) | bun + npm | npm scripts |

This is the source of multiple concrete problems we have already hit:

- The Nx setup in `game-zone-management-fe` is partially broken
  (`eslint-config-next` referenced but Next.js is not installed,
  `nx.json` plugins reference packages that are not in `package.json`).
  Reviving Nx would cost real time.
- We have **three lockfile formats** in play across the three repos
  (`yarn.lock`, `package-lock.json`, `bun.lock`). Reproducible installs
  are not happening.
- Cross-repo refactors (e.g. shared API types) are impossible without
  copy-paste.
- CI runs duplicated `npm install` in each repo with no caching across
  packages.

The consolidation forces us to pick a single workspace tool + package
manager + task runner. This ADR records that decision.

## Decision

We will use **Turborepo + pnpm workspaces** for the consolidated repo.

Concretely:

- Root `package.json` sets `"packageManager": "pnpm@9.x"` and Corepack
  pins it for every contributor.
- Root `pnpm-workspace.yaml` lists `apps/*` and `packages/*`.
- Root `turbo.json` defines the `build`, `dev` (persistent),
  `lint`, `format`, `typecheck`, `test` pipelines with appropriate
  `dependsOn` and `outputs` entries.
- All Nx configuration (`nx.json`, every `project.json`, every
  `@nx/*` dependency) is deleted as part of the
  `game-zone-management-fe` move (see `docs/MIGRATION.md` section 2c).
- All Yarn / Bun lockfiles are deleted; only `pnpm-lock.yaml` is
  tracked.

## Consequences

### Positive

- **Single command surface.** `pnpm dev`, `pnpm build`, `pnpm test`
  work the same way for every workspace. Contributors learn one set
  of commands.
- **Fast, content-addressed caching** via Turborepo. Local
  `--filter='...[origin/main]'` runs only what changed; CI reuses the
  same cache via the Turborepo remote cache (or GitHub Actions cache,
  whichever we wire first).
- **Strict, hard-linked installs** via pnpm. No more accidental hoisted
  dependencies; phantom deps fail at install time, not at runtime.
- **`strict-peer-dependencies=true`** catches version skew (e.g. two
  versions of React) the first time we try to install.
- **Workspace protocol (`workspace:*`)** lets packages depend on each
  other by name and always get the local source.
- **No vendor lock-in.** Both tools are OSS, widely adopted, and have
  predictable release cadences. Migration off either is a documented
  operation, not a rescue mission.

### Negative

- pnpm's symlink layout occasionally trips tooling that assumes a flat
  `node_modules`. We mitigate by setting `node-linker=hoisted` only in
  `apps/kiosk/.npmrc` if Tauri's bundler complains (it has in the past
  on some platforms).
- Turborepo's remote-cache costs money at scale. For now, GitHub
  Actions cache is sufficient (well under the free quota for a team of
  this size).
- Switching imports from `@admin-panel/shared-*` (Nx paths) to
  `@gaming-cafe/*` (pnpm workspace names) is a mechanical change but
  touches every admin source file. The mass-rename is captured as a
  Phase 2 task in `docs/MIGRATION.md`.

### Risks

- **Risk**: a contributor uses `npm install` or `yarn install` by
  accident, producing an out-of-band lockfile.
  **Mitigation**: `.npmrc` sets `engine-strict=true` and the root
  `package.json` `packageManager` field combined with Corepack will
  refuse to operate under the wrong manager.
- **Risk**: Turborepo pipeline misconfiguration causes stale builds
  (e.g. forgetting `dependsOn: ["^build"]` for a downstream).
  **Mitigation**: keep `turbo.json` small, review changes carefully,
  and rely on `turbo run build --force` for any release build.
- **Risk**: pnpm strictness blocks legitimate work (a package with a
  too-narrow peer range).
  **Mitigation**: `.pnpmfile.cjs` is available for surgical overrides,
  but every override needs a comment and an issue link.

## Alternatives considered

### Nx (already present in `game-zone-management-fe`)

- Pros: rich generators, project graph, executor abstraction; existing
  team familiarity from the admin repo.
- Cons: heavy abstraction layer over plain `package.json` scripts;
  the current `nx.json` setup is partially broken and would need
  rescue; executor plugins lag behind underlying tools (Vite, Jest,
  ESLint); upgrade churn between major versions is non-trivial.
- **Why rejected**: we are already paying for a broken Nx config and
  do not need its generator/graph features. Turborepo gives us the
  caching benefit with a fraction of the surface area.

### Yarn 4 workspaces (Berry, with PnP optional)

- Pros: mature workspace support, plugin system, Constraints engine.
- Cons: slower installs than pnpm in our benchmarks; PnP mode is
  incompatible with Tauri's bundler today; default (`node-modules`
  linker) loses the deduplication advantage that motivated picking it.
- **Why rejected**: pnpm matches or beats Yarn 4 on every dimension
  we care about (install speed, strictness, disk usage) and is the
  default choice in the JS ecosystem right now.

### Bun workspaces

- Pros: very fast installs; built-in test runner.
- Cons: Tauri's ecosystem (`@tauri-apps/cli`) and NestJS's build chain
  both prefer npm-compatible package managers; Bun's workspace
  protocol is still maturing; we would be early adopters with no easy
  fallback if Bun's pace slows.
- **Why rejected**: too much risk for the kiosk app; we want a boring,
  well-trodden choice for the workspace tool.

### No workspace tool (multiple `package.json` files, no top-level)

- Pros: zero new tooling.
- Cons: no shared install, no cross-package refactors, no CI
  affected-graph; defeats the consolidation's purpose.
- **Why rejected**: this is the status quo we are leaving.

## Implementation notes

- Phase 1 of the consolidation plan scaffolds the root files
  (`package.json`, `pnpm-workspace.yaml`, `turbo.json`, `.npmrc`,
  `.nvmrc`, `tsconfig.base.json`); see `docs/MIGRATION.md` section 4.
- The first `pnpm install` happens after Phase 2 moves complete. At
  that point we expect `turbo run build` to succeed for every app.
- CI uses `pnpm install --frozen-lockfile` and `turbo run lint
  typecheck test build --filter='...[origin/main]'` to gate every PR.

## References

- Consolidation plan: `gaming-cafe_workspace_hardening_b5738884.plan.md`
- Turborepo docs: <https://turborepo.com/docs>
- pnpm workspace docs: <https://pnpm.io/workspaces>
- Related ADRs: 0006 (Biome tooling), 0004 (shared API types), 0005
  (testing strategy)
