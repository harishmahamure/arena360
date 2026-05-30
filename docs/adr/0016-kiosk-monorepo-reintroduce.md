# ADR-0016: Re-introduce `apps/kiosk` into the monorepo

**Status**: Accepted
**Date**: 2026-05-30
**Deciders**: Platform team (gaming-cafe consolidation working group)

**Implements (deferred portion of)**: ADR-0002
**Supersedes**: _(none — ADR-0002 remains Accepted)_

## Context

[ADR-0002](0002-kiosk-tauri-canonical.md) (Accepted, 2026-05-27) already decides that the
**Tauri 2 kiosk** is canonical and should live at `apps/kiosk/` with co-located
`src-tauri/`, consuming shared packages per ADR-0004 and ADR-0007. The Electron
kiosk was deleted during consolidation.

During the same consolidation sprint, implementation scope was **explicitly deferred**:

- [PLANNER.md](../PLANNER.md) records a **2026-05-27 scope pivot**: kiosk tasks
  (`move-tauri-kiosk`, `kiosk-cleanup`, `ci-kiosk-build`, etc.) were removed from
  the active tracker.
- [MIGRATION.md](../MIGRATION.md) states that `agent-kiosk/` was **removed from scope
  entirely** (line 14).
- `apps/kiosk/` **does not exist** in this repository today.

Product direction has since been restored:

- [REQUIREMENTS-KIOSK.md](../REQUIREMENTS-KIOSK.md) defines the full v1 kiosk PRD
  (Windows-only, session lockdown, ggLeap-aligned).
- [PLANNER-KIOSK.md](../PLANNER-KIOSK.md) defines K0–K9 implementation tasks.

Several repo files **already assume** a kiosk workspace member without implementing it:

| File | Forward reference |
|------|-------------------|
| [CONTRIBUTING.md](../CONTRIBUTING.md) | `pnpm --filter @gaming-cafe/kiosk tauri:dev` |
| [.changeset/config.json](../../.changeset/config.json) | `@gaming-cafe/kiosk` in `ignore` |
| [turbo.json](../../turbo.json) | `build` inputs/outputs include `tauri.conf.json`, `src-tauri/target/release/**` |
| [pnpm-workspace.yaml](../../pnpm-workspace.yaml) | Lists only `apps/admin` — kiosk **missing** |

Per [`.cursor/rules/20-adr-discipline.mdc`](../../.cursor/rules/20-adr-discipline.mdc), adding
`apps/kiosk` to `pnpm-workspace.yaml`, `turbo.json`, or root `package.json` scripts
is an **architectural change** and requires a DRAFT ADR before coding.

This ADR **reverses the process deferral only**. It does not change the Tauri-vs-Electron
decision in ADR-0002.

Companion K0 ADRs (auth, WebSocket ACL, allow-list, lockdown) remain **separate** and
are required before K1 backend APIs and K3+ kiosk features — they are not bundled here.

## Decision

We will **re-activate kiosk implementation** in the monorepo and add `apps/kiosk` as a
first-class pnpm workspace package.

### Package layout

| Path | Purpose |
|------|---------|
| `apps/kiosk/` | Tauri 2 + React 19 front-end (`@gaming-cafe/kiosk`) |
| `apps/kiosk/src-tauri/` | Rust shell: lockdown, fingerprint, scan, process supervision |
| `apps/kiosk/.cursor/rules/` | Kiosk-local Cursor rules (moved from `agent-kiosk/`) |
| `docs/adr-kiosk-archive/` | Historical ADRs from `agent-kiosk/docs/adr/` (per ADR-0002) |

### Source baseline

1. **Preferred**: Import from `_archive/agent-kiosk-2026-05-27.tgz` (local tarball per
   [PLANNER.md](../PLANNER.md) `archive-old-repos`).
2. **Fallback**: Greenfield `create tauri-app` scaffold if the archive is unavailable;
   feature work still follows [REQUIREMENTS-KIOSK.md](../REQUIREMENTS-KIOSK.md).

### Platform scope (v1)

- **Windows 10/11 only** for release artefacts (per REQUIREMENTS-KIOSK §1.3).
- Dev on macOS/Linux allowed for UI work; lockdown/process features require Windows QA.

### Workspace contract (implemented in K2 — not part of accepting this ADR alone)

When this ADR is **Accepted**, the following changes are authorized (task
`kiosk-workspace-wire` in PLANNER-KIOSK):

**`pnpm-workspace.yaml`** — add workspace member:

```yaml
packages:
  - "apps/admin"
  - "apps/kiosk"
  - "packages/*"
```

**`turbo.json`** — add Tauri tasks (keep existing `build` outputs for `src-tauri`):

```json
"tauri:dev": {
  "cache": false,
  "persistent": true
},
"tauri:build": {
  "dependsOn": ["^build"],
  "inputs": ["src/**", "package.json", "tsconfig.json", "vite.config.*", "tauri.conf.json", "src-tauri/**"],
  "outputs": ["src-tauri/target/release/**"]
}
```

**Root `package.json`** — convenience scripts:

```json
"kiosk:dev": "pnpm --filter @gaming-cafe/kiosk tauri:dev",
"kiosk:build": "pnpm --filter @gaming-cafe/kiosk tauri:build"
```

**`.changeset/config.json`** — no change; `@gaming-cafe/kiosk` stays in `ignore`.

**`apps/kiosk/.npmrc`** — `node-linker=hoisted` for Tauri compatibility (per
[MIGRATION.md](../MIGRATION.md) Table 4).

**Documentation alignment** (K2 / follow-on):

- Update [MIGRATION.md](../MIGRATION.md) Table 4 kiosk rows from `pending` → `moved` / `verified`.
- Confirm [CONTRIBUTING.md](../CONTRIBUTING.md) commands match the workspace contract above.

### Shared package consumption

`@gaming-cafe/kiosk` will consume:

- `@gaming-cafe/theme` — CSS tokens (`--gz-*`) per ADR-0007 (no MUI on kiosk).
- `@gaming-cafe/api-types` — generated from backend OpenAPI per ADR-0004.
- `@gaming-cafe/contracts` — `ErrorCode`, roles per ADR-0008.
- `@gaming-cafe/utils` — HTTP client factory per ADR-0004 consolidation plan.

The duplicate `open-api.json` in the legacy kiosk repo must **not** be copied;
types come only from `@gaming-cafe/api-types`.

## Consequences

### Positive

- Fulfills the deferred implementation path of ADR-0002 in a single repo.
- Admin, backend, and kiosk share one lockfile, one Turbo graph, and one CI policy.
- Turbo can cache Tauri release builds via `src-tauri/target/release/**` outputs.
- Contributors follow one documented dev loop in CONTRIBUTING.md.

### Negative

- **Rust toolchain** becomes mandatory for anyone working on `apps/kiosk/src-tauri/`
  (in addition to existing backend contributors).
- **WebView2** must be documented for Windows 10 field installs.
- **Windows CI runner** required for release builds (see PLANNER-KIOSK K8 `ci-kiosk-build`).
- Re-introducing a large app surface increases monorepo CI time unless path filters are used.

### Risks

| Risk | Mitigation |
|------|------------|
| Archive tarball missing or stale | Greenfield Tauri scaffold; REQUIREMENTS-KIOSK is source of truth for behaviour |
| K0 companion ADRs delayed | K2 scaffold may proceed after **this** ADR is Accepted; K1/K3+ remain blocked on auth/WS/lockdown ADRs |
| `turbo.json` / workspace edit without ADR | This DRAFT must be Accepted before `kiosk-workspace-wire` |

## Alternatives considered

### A. Keep kiosk deferred (status quo since 2026-05-27)

- Pros: No Rust/Tauri CI cost; smaller active scope.
- Cons: Contradicts ADR-0002, REQUIREMENTS-KIOSK, and customer-facing roadmap; stale forward refs in CONTRIBUTING/changeset.
- **Why rejected**: Product and planning docs already commit to v1 kiosk.

### B. Separate repository for `agent-kiosk`

- Pros: Isolated CI; no monorepo graph changes.
- Cons: Violates ADR-0001; duplicates lockfile/tooling; shared types drift returns.
- **Why rejected**: Consolidation goal is one monorepo.

### C. Revive Electron kiosk from `game-zone-management-fe`

- Pros: Familiar stack for some contributors.
- Cons: Explicitly rejected by ADR-0002; worse lockdown and binary size.
- **Why rejected**: Regresses on security and ADR-0002.

## Implementation notes

**Order of execution after Acceptance:**

1. `kiosk-workspace-wire` (K2) — apply workspace contract above.
2. `kiosk-app-scaffold` (K2) — import archive or scaffold; delete duplicate OpenAPI file.
3. Remaining K0 ADRs (player-auth, ws-acl, allow-list, lockdown) — **before** K1 APIs.
4. K1–K9 per [PLANNER-KIOSK.md](../PLANNER-KIOSK.md).

**Do not** merge `apps/kiosk` in the same PR as this ADR document unless the ADR is
already Accepted — ADR discipline requires approval before workspace file edits.

## References

- [ADR-0001](0001-monorepo-turborepo-pnpm.md) — Monorepo on Turborepo + pnpm
- [ADR-0002](0002-kiosk-tauri-canonical.md) — Tauri kiosk canonical (Accepted)
- [ADR-0004](0004-shared-api-types-openapi.md) — Shared API types
- [ADR-0007](0007-design-tokens-shared.md) — Design tokens
- [REQUIREMENTS-KIOSK.md](../REQUIREMENTS-KIOSK.md) — Kiosk product requirements
- [PLANNER-KIOSK.md](../PLANNER-KIOSK.md) — Kiosk implementation tracker
- [PLANNER.md](../PLANNER.md) — Consolidation tracker (2026-05-27 deferral)
- [MIGRATION.md](../MIGRATION.md) — Per-file move checklist
