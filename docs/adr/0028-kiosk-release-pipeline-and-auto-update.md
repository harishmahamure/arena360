# ADR-0028: Kiosk release pipeline — version bump, Windows build, and auto-update manager

**Status**: Accepted
**Date**: 2026-06-03
**Deciders**: Platform team
**Relates to**: [ADR-0002](0002-kiosk-tauri-canonical.md), [ADR-0016](0016-kiosk-monorepo-reintroduce.md), [ADR-0003](0003-secrets-management.md), [ADR-0020](0020-kiosk-windows-lockdown.md)

## Amendment 2026-06-15: post-install station configuration

The NSIS installer ships [`configure-station.ps1`](../../apps/kiosk/scripts/windows/configure-station.ps1)
and runs it post-install (unless `/NOCONFIGURE`):

| Default action | Opt-out flag |
|----------------|--------------|
| Register **Arena360 Kiosk** at logon (main exe) | `/NOAUTOSTART` → `-SkipAutostart` (alias `-SkipWatchdog`) |
| Set HKLM policy keys (`DisableTaskMgr`, `DisableLockWorkstation`, `DisableChangePassword`, `NoRun`) | `/NOHARDENING` |
| Prompt for auto-logon password (interactive) or accept `-AutoLogonPassword` on manual re-run | skip by pressing Enter at prompt |

- Marker file `%ProgramData%\Arena360\registry-hardening.json` records values written by the
  installer; uninstall invokes the script with `-Uninstall` to remove them.
- `/KIOSKUSER=Name` targets the single Windows kiosk account (default: installing user).
- Watchdog at logon only — does **not** set `Winlogon\Shell` or replace `explorer.exe`.
- Assigned Access / shell replacement remain manual (see `KIOSK-WINDOWS-DEPLOYMENT.md` Layer 1).

## Context

The kiosk is a Tauri 2 app (`apps/kiosk`). Today `.github/workflows/kiosk-ci.yml`
builds the Windows NSIS/MSI bundle on `windows-latest` and uploads it as a
workflow artifact, but:

- The app **version is hand-edited** in three places that must stay in sync:
  `apps/kiosk/src-tauri/tauri.conf.json` (`version`),
  `apps/kiosk/src-tauri/Cargo.toml` (`package.version`), and
  `apps/kiosk/package.json` (`version`).
- **Auto-update is intentionally disabled** (`bundle.createUpdaterArtifacts:
  false`). `apps/kiosk/README.md` §"Auto-update" and `docs/PLANNER-KIOSK.md`
  (K9, `kiosk-auto-update`) explicitly record that enabling it requires adding
  `tauri-plugin-updater`, a signing key pair, and a `plugins.updater` block with
  release endpoints.

The maintainer wants a release pipeline (mirroring the backend deploy workflow's
`workflow_dispatch` style) that bumps the version, builds the signed Windows
installer, and feeds the Tauri **update manager** so deployed kiosks can
self-update.

Enabling the updater is an architectural change (per `20-adr-discipline.mdc`):
new external dependency that changes deployment shape, new HTTP surface (the
update endpoint), and new signing-key secrets (ADR-0003). Hence this ADR.

## Decision (resolved choices)

Maintainer decisions (2026-06-03):

- **Scope**: full pipeline — version bump + signed build + enabled auto-update.
- **Distribution host**: **GitHub Releases**.
- **Trigger**: manual `workflow_dispatch` with a `bump` (`patch`/`minor`/`major`) input.
- **Install scope**: keep **`perMachine`** (NSIS). Updates therefore require
  elevation; see risks.

Add a `workflow_dispatch` release workflow
(`.github/workflows/kiosk-release.yml`) and enable the Tauri v2 updater.

### 1. Version bump (single source → fan-out)

- Workflow input `bump` (`patch` | `minor` | `major`).
- `apps/kiosk/scripts/set-version.mjs` writes the new semver into all three
  files (`tauri.conf.json`, `Cargo.toml`, `package.json`) so they cannot drift,
  and prints `version=X.Y.Z` for the workflow.
- The workflow commits the bump (`chore(kiosk): release vX.Y.Z`) and tags
  `kiosk-vX.Y.Z` (conventional-commit form per `docs/CONTRIBUTING.md`).

### 2. Updater plugin (always compiled in)

- Add `tauri-plugin-updater` (Rust) + `@tauri-apps/plugin-updater` (JS) and
  `tauri-plugin-process` / `@tauri-apps/plugin-process` (to relaunch after
  install). Register both behind `#[cfg(desktop)]` in the Tauri builder, and add
  `updater:default` + `process:default` to the kiosk capability.
- The webview checks for updates **only while the station is idle** (kiosk
  `register`, `setup`, or `login` phase — no active player session), gated to
  respect ADR-0020 lockdown, with failures swallowed (offline /
  not-yet-configured is non-fatal).

### 3. Signing + endpoint injected at release time (not committed)

To keep `kiosk-ci.yml` (every PR) building without release keys and to avoid
committing any key material or a stale public key, the updater **runtime
config** is injected only by the release workflow via a `--config` overlay:

```jsonc
// generated in CI from repo settings, merged via `tauri build --config`
{
  "bundle": { "createUpdaterArtifacts": true },
  "plugins": {
    "updater": {
      "pubkey": "<vars.TAURI_UPDATER_PUBKEY>",
      "endpoints": [
        "https://github.com/<owner>/<repo>/releases/latest/download/latest.json"
      ]
    }
  }
}
```

- Build runs via `tauri-apps/tauri-action` on `windows-latest`, signing the
  update artifacts with `TAURI_SIGNING_PRIVATE_KEY` /
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (new GitHub Actions secrets), and
  publishes the installer, `.sig`, and generated `latest.json` to the GitHub
  Release for tag `kiosk-vX.Y.Z`.
- `TAURI_UPDATER_PUBKEY` is the **public** key (safe in repo settings as an
  Actions *variable*); only the private key lives in secrets (ADR-0003).
- Authenticode signing of the installer remains optional via the existing
  `signCommand` hooks (README §signing).

The base `tauri.conf.json` keeps `createUpdaterArtifacts: false` and no
`plugins.updater` block, so the existing CI build is unchanged and dev builds
simply have no update endpoint (checks no-op).

## Consequences

### Positive

- One-click, repeatable kiosk releases; version can never drift across the 3 files.
- Fleet self-updates without re-imaging each station.
- Reuses the existing Windows CI shape; no new infra to operate (Releases).
- No key material or environment-specific endpoint committed to the repo.

### Negative / Risks

- New signing keypair to generate, store (CI secret/variable), and back up.
  Losing the private key means shipped kiosks can no longer verify updates →
  must re-image.
- `perMachine` installs mean the NSIS updater needs **elevation** to replace
  machine-wide files; on a locked-down kiosk this surfaces a UAC prompt. Operator
  must run the kiosk with rights to self-update or accept a manual elevation step.
- Auto-update must never interrupt an active player session — hence the idle
  gate on `register` | `setup` | `login` phases (ADR-0020).
- GitHub Releases must be reachable from cafe networks; if blocked, fall back to
  object storage (see alternatives).

### Mitigation

- Document key generation (`tauri signer generate`) and storage in the README;
  keep a sealed backup of the private key outside CI.
- Gate `check()` / `downloadAndInstall()` behind the kiosk idle phases
  (`register`, `setup`, `login`).
- Revisit `perUser` install or an elevated update service if UAC prompts prove
  unworkable on locked stations.

## Alternatives considered

### Build + publish installer only, keep auto-update OFF — no ADR needed

- Bump version and upload the existing NSIS/MSI to a Release, leaving
  `createUpdaterArtifacts: false`, no plugin, no keys.
- Pros: smallest change; operators update kiosks manually.
- Cons: no fleet self-update (the maintainer's "update manager" ask).
- **Smallest alternative that would not require an ADR.** Rejected: maintainer
  wants self-update.

### Host update manifest on object storage (S3/MinIO, per DRAFT-0022)

- Pros: same infra as media/assets; works where github.com is blocked.
- Cons: pipeline must upload `latest.json` + artifacts and manage public read.
- Rejected for v1 in favour of GitHub Releases; kept as the documented fallback.

### Backend-served update endpoint on the Axum API

- Pros: full control, can gate by device/version, single trusted origin.
- Cons: new HTTP surface in the backend (ADR-0009) + artifact storage; largest
  change. Out of scope for v1.

## References

- `.github/workflows/kiosk-release.yml` (new), `.github/workflows/kiosk-ci.yml`
- `apps/kiosk/scripts/set-version.mjs` (new)
- `apps/kiosk/scripts/windows/configure-station.ps1` (post-install station config)
- `apps/kiosk/src-tauri/windows/hooks.nsh` (NSIS hooks)
- `apps/kiosk/src-tauri/tauri.conf.json`, `Cargo.toml`, capabilities/default.json
- `apps/kiosk/src/lib/updater.ts` (new)
- `apps/kiosk/README.md` §"Auto-update", §signing
- Tauri v2 updater plugin + `tauri-apps/tauri-action`
