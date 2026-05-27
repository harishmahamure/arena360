# ADR-0006: Biome replaces ESLint + Prettier

**Status**: Accepted
**Date**: 2026-05-27
**Deciders**: Platform team (gaming-cafe consolidation working group)

## Context

Each source repo carries a different (and partially broken) lint +
format setup:

- `arena360-backend`: ESLint (NestJS default) + Prettier. Functional
  but slow on a clean install and pulls a long dep tail.
- `game-zone-management-fe`: ESLint (Nx flat config) + Prettier, with
  an `eslint-config-next` extension that references a Next.js install
  that does not exist. `pnpm lint` fails out-of-the-box.
- `agent-kiosk`: ESLint (rough config) + Prettier; rules disagree
  with the admin's Prettier config on quotes, trailing commas, and
  line length.

For the consolidation we need **one** lint + format toolchain that
works the same across backend (Nest, Node-flavoured TS), admin (React,
DOM-flavoured TS), kiosk-React (React 19, Tauri-flavoured), and the
shared packages. Carrying three sub-configs and bridging them with
shared presets is the obvious choice; it is also the trap that the
current repos fell into.

[Biome](https://biomejs.dev) (`@biomejs/biome`, currently `^2.4.15`)
is a Rust-implemented linter and formatter that replaces both ESLint
and Prettier with a single binary, a single config file, and
single-digit-millisecond runs on typical PRs. As of 2.x it has
React rules (`useExhaustiveDependencies`, `useHookAtTopLevel`, etc.),
flat-config-style overrides, and a CI-friendly `biome ci` command.

## Decision

We will adopt **Biome `^2.4.15` as the sole lint + format tool**
across the monorepo, replacing ESLint and Prettier entirely.

Concretely:

1. **Shared preset**: `packages/biome-config` exports a `biome.json`
   preset with the project's house rules. Every workspace references
   it via `"extends": ["@gaming-cafe/biome-config/biome.json"]` in
   its local `biome.json`.

2. **Root `biome.json`** sets the project-wide includes / excludes
   (e.g. ignore `dist/`, `.turbo/`, `coverage/`, `target/`,
   `packages/api-types/src/schema.ts`).

3. **Root scripts**:

   - `pnpm lint`  → `biome check .`
   - `pnpm format` → `biome format --write .`
   - `pnpm lint:ci` → `biome ci .` (treats warnings as errors,
     emits machine-readable output for CI annotations)

4. **All ESLint + Prettier artefacts are deleted** during Phase 2:
   `.eslintrc*`, `.eslintignore`, `eslint.config.{js,mjs,cjs}`,
   `.prettierrc*`, `.prettierignore`, and every related devDep
   (`eslint`, `prettier`, `@nx/eslint*`, `eslint-config-*`,
   `@typescript-eslint/*`). See `docs/MIGRATION.md` for the per-repo
   deletions.

5. **Editor integration**: contributors install the Biome IDE plugin
   (VS Code, Cursor, JetBrains) and disable any Prettier/ESLint
   plugins on this repo to prevent fights.

6. **CI gate**: `biome ci .` runs as part of the `lint` task in the
   Turborepo pipeline. PRs are blocked on a clean lint.

## Consequences

### Positive

- **Single tool, single config.** No more ESLint-vs-Prettier
  conflicts; no shared-preset rabbit holes.
- **Order-of-magnitude speed**. Biome is Rust-native. On the current
  source code, `biome check` completes in <2 s where the existing
  ESLint runs take ~30 s on a warm cache and ~2 min cold.
- **Fewer devDeps.** Removing ESLint + Prettier + their plugin
  ecosystem cuts hundreds of transitive packages.
- **One install across all platforms.** Biome ships pre-built
  binaries for every dev OS we use (macOS arm64/x64, Linux x64,
  Windows x64). No Node-version skew.
- **First-class formatter included.** No second tool to wire up.
- **Built-in `biome ci`** with sensible defaults for CI annotation
  output.

### Negative

- **Smaller plugin ecosystem than ESLint.** Some ESLint plugins have
  no Biome equivalent (most notably bespoke import-ordering rules
  and project-specific custom rules). For React, Biome 2.x has
  built-in equivalents for the rules we actually relied on
  (`useExhaustiveDependencies` covers `react-hooks/exhaustive-deps`;
  `noUnusedImports` covers Prettier-import-sorting-style cleanups).
- **No NestJS-specific rules.** ESLint had a few community Nest
  plugins; we never used them meaningfully, so this is acceptable.
- **Some opinionated formatting differences from Prettier** (line
  width default, JSX prop wrapping). Documented in
  `packages/biome-config/README.md` so contributors aren't surprised.
- **Tied to one tool's roadmap.** If Biome's pace slows, falling back
  to ESLint + Prettier is a multi-day operation (recreate the
  configs, restore the plugins).

### Risks

- **Risk**: a rule we depend on is missing.
  **Mitigation**: Biome 2.x covers our day-one needs (formatting,
  unused-imports, hook rules, accessibility basics via
  `useJsxKeyInIterable` et al.). If we hit a true gap, we either
  add a Biome rule request upstream or codify the rule in PR review
  guidelines.
- **Risk**: `react-hooks/exhaustive-deps` parity isn't perfect.
  **Mitigation**: Biome's `useExhaustiveDependencies` rule has been
  the de-facto replacement for ~12 months. Documented as the
  workaround; the rule is enabled in
  `packages/biome-config/biome.json`.
- **Risk**: contributors keep ESLint/Prettier plugins enabled in
  their IDE and get phantom warnings.
  **Mitigation**: `docs/CONTRIBUTING.md` documents the IDE setup;
  `.vscode/settings.json` in the repo can disable conflicting
  formatters locally (added in Phase 1).
- **Risk**: Biome major version bump (2 → 3) changes rules.
  **Mitigation**: pin to `^2.4.15` (minor + patch only). Major
  bumps require a deliberate PR with a CHANGELOG read.

## Alternatives considered

### Keep ESLint + Prettier (status quo, fixed)

- Pros: well-known, infinite plugin ecosystem.
- Cons: the current setup is broken in `game-zone-management-fe`;
  reviving it costs real time; slow on every contributor's machine
  and in CI; two configs to keep in sync.
- **Why rejected**: this is what we are explicitly leaving behind.

### oxlint (Oxc) + Prettier

- Pros: also Rust-native; ESLint-rule-compatible by design; very
  fast.
- Cons: still maturing — at the time of this decision, oxlint's
  rule coverage is incomplete vs the rules we want enabled (notably
  no formatter; pairing with Prettier reintroduces the two-tool
  problem we want to delete).
- **Why rejected**: not ready for sole-tool use today; revisit in
  ADR if Biome stalls.

### dprint (formatter) + a separate linter

- Pros: dprint is fast and pluggable.
- Cons: same two-tool problem; smaller community than Biome for the
  JS/TS lane.
- **Why rejected**: no advantage over Biome.

### Rome (Biome's predecessor)

- N/A — Rome is archived; Biome is its continuation.

## Implementation notes

- `packages/biome-config/biome.json` enables, at minimum:
  - `recommended` rule set.
  - `useExhaustiveDependencies` (React hook deps parity).
  - `useHookAtTopLevel`.
  - `noUnusedImports`, `noUnusedVariables`.
  - `noConsole` (warn — we want `pino` on the backend, structured
    logger on the FE).
  - `useImportType` (TS-only `import type` for type imports).
  - Formatter: 2-space indent, 100-col width, single quotes for JS,
    double quotes for JSX attributes, trailing commas `all`,
    semicolons `always`. Documented in
    `packages/biome-config/README.md`.
- Per-workspace `biome.json` files exist only when overrides are
  needed (e.g. `apps/kiosk` may need to allow `console.log` from
  Tauri command logging during dev).
- The shared `.skip` test lint rule referenced in ADR-0005 is
  implemented via Biome's `noFocusedTests` / a project-specific
  pattern; documented in `packages/biome-config/biome.json`.
- Removal of the old configs is captured row-by-row in
  `docs/MIGRATION.md`.

## References

- Biome docs: <https://biomejs.dev/>
- ADR-0001: Turborepo + pnpm (where `lint`/`format` are plumbed).
- ADR-0005: Testing strategy (the `.skip` lint rule).
- Consolidation plan, `fix-broken-eslint` todo (now satisfied by
  this ADR's tool swap, not by reviving the broken ESLint setup).
