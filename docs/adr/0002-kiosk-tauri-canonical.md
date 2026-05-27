# ADR-0002: Tauri 2 kiosk is canonical; Electron kiosk is deleted

**Status**: Accepted
**Date**: 2026-05-27
**Deciders**: Platform team (gaming-cafe consolidation working group)

## Context

Two kiosk applications currently exist:

| Repo / path | Stack | Status |
|---|---|---|
| `agent-kiosk/` (standalone repo) | Tauri 2 + React 19 + Rust crate at `src-tauri/`; feature-sliced layout; in-repo cursor rules and ADRs | Active development |
| `game-zone-management-fe/apps/kiosk/` | Electron + React; lives inside the Nx workspace | Stale; thin layer over the same backend |

Both surfaces target the same use case — the in-cafe gaming PC running
in lockdown mode, talking to `apps/backend` — and the cafe has
standardised on the Tauri kiosk for in-field deployments. The Electron
copy is residual from an earlier experiment and has not been kept in
sync with the backend's API changes for some time (e.g. it still
references hand-written response types that no longer match the
NestJS DTOs).

The consolidation forces a choice: ship two kiosks (and pay the
maintenance tax of two stacks, two build pipelines, two install
flows), or pick one and delete the other.

## Decision

We will **keep the Tauri kiosk as canonical** (`agent-kiosk/` →
`apps/kiosk/` in the new monorepo) and **delete the Electron kiosk**
(`game-zone-management-fe/apps/kiosk/`) outright.

Concretely:

- `agent-kiosk/` moves to `apps/kiosk/`; `src-tauri/` stays
  co-located. The cursor rules under `agent-kiosk/.cursor/rules/`
  move to `apps/kiosk/.cursor/rules/`. The existing kiosk-local ADRs
  under `agent-kiosk/docs/adr/` are archived to
  `docs/adr-kiosk-archive/` so their history is preserved without
  polluting the repo-wide ADR index.
- `game-zone-management-fe/apps/kiosk/` is removed in its entirety in
  Phase 2 of the migration (see `docs/MIGRATION.md` section 2c). No
  Electron files or dependencies survive in the new repo.
- The shared frontend libraries (`game-zone-management-fe/libs/shared/*`)
  remain useful and migrate to `packages/*`; the kiosk app may consume
  them when appropriate, but is not required to do so during the
  consolidation.

## Consequences

### Positive

- **Smaller binary.** A Tauri release artefact for our kiosk is
  ~10-15 MB versus 80-150 MB for the equivalent Electron app. Faster
  downloads on cafe-owner internet, faster startup, less disk used on
  the gaming PCs.
- **Better security posture.** Tauri's IPC is allow-listed per command;
  the Rust core has no Node `require()` to abuse; the WebView runs as
  the host system's webview (WebView2 on Windows) so security patches
  arrive via OS updates.
- **Native lockdown surface in Rust.** Kiosk-mode requirements
  (`US-KIOSK-006`: block task switcher, suppress shell hotkeys,
  process supervision for the launched game) are easier to implement
  correctly and portably in Rust + small platform shims than via
  Electron's JS-only main process.
- **Already organised.** `agent-kiosk` already has a feature-sliced
  layout, cursor rules, and ADRs documenting decisions. Adopting it
  costs nothing extra; recreating the equivalent in Electron would be
  weeks.
- **One stack to learn.** New contributors learn Tauri once instead of
  Tauri + Electron.

### Negative

- **Rust toolchain becomes a hard dev requirement** for kiosk work.
  Mitigation: only kiosk contributors need it; the backend and admin
  apps remain Rust-free. Documented in `docs/CONTRIBUTING.md`.
- **Tauri's plugin ecosystem is smaller than Electron's.** We have
  not yet hit a missing plugin; if we do, the workaround is a custom
  Rust command, which is acceptable.
- **WebView differences across OS versions.** WebView2 on Windows 10
  must be installed manually; on Windows 11 it ships built-in. The
  installer flow handles this; documented in
  `apps/kiosk/README.md` (post-migration).
- **One-way door for any Electron-only feature** we might have wanted
  later (e.g. arbitrary native modules). Acceptable: no such feature
  is on the roadmap.

### Risks

- **Risk**: a future product requirement turns out to need an
  Electron-only capability.
  **Mitigation**: Tauri 2's plugin system + raw Rust commands cover
  every requirement we can articulate today. If a hard blocker
  appears, we revisit this ADR — superseding it is cheap relative to
  the day-to-day cost of carrying two kiosks.
- **Risk**: existing field deployments are running the Electron
  kiosk and break on the next backend release.
  **Mitigation**: there are no production deployments of the
  Electron kiosk today; the Tauri kiosk is the only field-deployed
  build. Verified with the deployment team before this ADR was
  accepted.
- **Risk**: knowledge of the old Electron behaviour is lost.
  **Mitigation**: the source remains in the archived
  `game-zone-management-fe` remote backup branch (see consolidation
  plan, Phase 1). Anyone needing to compare a behaviour can clone
  the archive.

## Alternatives considered

### Keep both kiosks, ship two products

- Pros: maximal optionality.
- Cons: double the build matrix, double the test matrix, drift between
  the two is guaranteed (already happening). No business reason to
  ship two.
- **Why rejected**: cost without benefit.

### Migrate the Tauri kiosk to Electron (consolidate on Electron)

- Pros: larger plugin ecosystem; the team has more Electron
  experience historically.
- Cons: throws away the working Tauri kiosk, including its lockdown
  primitives in Rust; bigger binaries; weaker security model; would
  add an active Node main process to a machine that is supposed to be
  locked down.
- **Why rejected**: regresses on every dimension we care about.

### Web-only kiosk (no native shell) in fullscreen browser

- Pros: zero native build pipeline.
- Cons: cannot enforce kiosk-mode lockdown; cannot launch local game
  processes; cannot manage installed software inventory.
- **Why rejected**: fails `US-KIOSK-003`, `US-KIOSK-004`,
  `US-KIOSK-006`.

## Implementation notes

- The move is captured row-by-row in `docs/MIGRATION.md` section 3.
- The Electron deletion is captured in `docs/MIGRATION.md` section 2c
  (`game-zone-management-fe/apps/kiosk/**`).
- `apps/kiosk/README.md` is updated (post-move) to reflect monorepo
  paths and the new `pnpm --filter @gaming-cafe/kiosk tauri:dev`
  command.
- The duplicate OpenAPI spec at `agent-kiosk/open-api.json` is deleted
  during the move; the kiosk consumes types from `@gaming-cafe/api-types`
  per ADR-0004.

## References

- `agent-kiosk/docs/adr/` (archived to `docs/adr-kiosk-archive/`):
  historical kiosk-specific ADRs preserved for context.
- ADR-0004: shared API types via OpenAPI.
- ADR-0007: design tokens shared via CSS variables — kiosk consumes
  these without a UI rewrite.
- Consolidation plan, Phase 2, `move-tauri-kiosk` and
  `drop-electron-kiosk` todos.
