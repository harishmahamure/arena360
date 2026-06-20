# @gaming-cafe/kiosk

Windows Tauri 2 + React 19 in-cafe kiosk. See [REQUIREMENTS-KIOSK.md](../../docs/REQUIREMENTS-KIOSK.md) and [PLANNER-KIOSK.md](../../docs/PLANNER-KIOSK.md).

**IT deployment runbook:** [docs/STATION-DEPLOYMENT-GUIDE.md](../../docs/STATION-DEPLOYMENT-GUIDE.md) (fleet checklist, lockdown, auto-start, provisioning).

## Environment

Copy the example env file and point it at your backend (default Axum port is `3000` locally):

```bash
cp apps/kiosk/.env.example apps/kiosk/.env
# edit VITE_API_URL if needed
```

| Variable | Purpose | Default |
|----------|---------|---------|
| `VITE_API_URL` | REST API base URL (WebSocket uses `/realtime` on the same host) | `http://localhost:3000` |
| `VITE_API_URL_WS` | Legacy WS hint (logged in CI; runtime derives WS from `VITE_API_URL`) | — |
| `VITE_GALLERY_URL` | CDN-hosted `gallery.json` for Setup media picker | `https://cdn.arena360.cloud/kiosk/gallery.json` |
| `VITE_LOGIN_BACKGROUND_VIDEO_URL` | Login screen background loop (CDN); falls back to bundled `public/launch.webm` | `https://cdn.arena360.cloud/launch.webm` |
| `VITE_KIOSK_LOGO_URL` | Venue logo on login / nav | shared theme default |
| `VITE_OFFLINE_GRACE_MINUTES` | Minutes a session keeps counting locally after the backend is unreachable before the station re-locks | `5` |

CI and release builds (`kiosk-ci.yml`, `kiosk-release.yml`) bake these in at build time. Override per environment via GitHub repository **Settings → Secrets and variables → Actions → Variables** (`VITE_API_URL`, `VITE_GALLERY_URL`, etc.).

## Dev

From repo root:

```bash
pnpm install
cp apps/kiosk/.env.example apps/kiosk/.env   # first time only
pnpm kiosk:dev
```

Or:

```bash
pnpm --filter @gaming-cafe/kiosk tauri:dev
```

Rust tests:

```bash
pnpm --filter @gaming-cafe/kiosk test:rust
```

Frontend unit tests (Vitest + RTL):

```bash
pnpm --filter @gaming-cafe/kiosk test
```

End-to-end smoke (against a running, seeded backend):

```bash
API_URL=http://localhost:3000 \
KIOSK_REG_CODE_A=... KIOSK_REG_CODE_B=... \
KIOSK_PLAYER_USERNAME=... KIOSK_PLAYER_PASSWORD=... \
pnpm --filter @gaming-cafe/kiosk test:e2e
```

## Windows packaging

The kiosk ships as a Windows installer. Build on Windows (or the `kiosk-windows-ci`
GitHub job):

```bash
pnpm --filter @gaming-cafe/kiosk tauri:build
```

This produces the NSIS installer under `apps/kiosk/src-tauri/target/release/bundle/nsis/`:

- **NSIS** (`nsis/*-setup.exe`) — per-machine install (`installMode: perMachine`). For SCCM/Intune, deploy silently (`/S`).

**Post-install:** the NSIS installer does **not** run PowerShell. Default install path:
`C:\Program Files\Arena360 Station Management\`. After install, IT runs
[`configure-station.ps1`](scripts/windows/configure-station.ps1) from the repo to register the
**Arena360 Kiosk** logon task (optional HKLM hardening and auto-logon). See
[KIOSK-WINDOWS-DEPLOYMENT.md](../../docs/KIOSK-WINDOWS-DEPLOYMENT.md).

### WebView2 runtime (Windows 10)

The app renders through the Microsoft **Edge WebView2** runtime. The installer is
configured with `webviewInstallMode: embedBootstrapper`, so the WebView2
bootstrapper is embedded and run during install (small download at install time).

- Windows 11 ships WebView2 by default.
- Windows 10 may not have it. The embedded bootstrapper installs it automatically;
  for fully offline imaging, switch `webviewInstallMode` to `offlineInstaller`
  (embeds the full runtime, ~150 MB) in `src-tauri/tauri.conf.json`, or pre-install the
  [Evergreen Standalone Installer](https://developer.microsoft.com/microsoft-edge/webview2/).

### Lockdown / kiosk OS configuration

App-level lockdown blocks the Windows keys, Alt+F4 and Ctrl+Shift+Esc, hides the taskbar and Start menu flyout while locked, and Alt+Tab uses native Windows switching.

**Staff shortcuts (login screen):**

| Shortcut | Action |
|----------|--------|
| Ctrl+Shift+A | Open setup / admin login |
| Ctrl+Shift+B | Clear player login lockout (“too many attempts”) |

Ctrl+Alt+Del (the Secure Attention Sequence) cannot be intercepted from user mode.
For a hardened station, also apply at the OS level:

- Assigned Access / kiosk account, or group policy `DisableLockWorkstation` and
  `DisableTaskMgr` (optional — via `configure-station.ps1 -SkipAutostart` or GPO).
- Auto-login and logon autostart: run [`configure-station.ps1`](scripts/windows/configure-station.ps1)
  after install (not invoked by the installer). This registers the **Arena360 Kiosk**
  ONLOGON task only — it does **not** replace `explorer.exe` as the shell.

**Full roadmap:** [docs/KIOSK-WINDOWS-DEPLOYMENT.md](../../docs/KIOSK-WINDOWS-DEPLOYMENT.md)
(Assigned Access, shell replacement, logon autostart, fleet rollout).
**IT entry point:** [docs/STATION-DEPLOYMENT-GUIDE.md](../../docs/STATION-DEPLOYMENT-GUIDE.md).

### Code signing (release)

Production installers should be Authenticode-signed to avoid SmartScreen prompts.
Set the signing certificate via Tauri's Windows signing env vars in CI
(`TAURI_SIGNING_*` / `signCommand`) — see the Tauri v2 Windows signing guide. Signing
is intentionally left unconfigured in-repo so no certificate material is committed.

### Auto-update (ADR-0028)

The kiosk ships with the Tauri update manager (`tauri-plugin-updater` +
`tauri-plugin-process`). It checks for a newer signed build **only while the
station is idle** (`register`, `setup`, or `login` phase — no active player
session) and, if found, downloads, installs, and relaunches.

Updates are produced and published by the **Release Kiosk (Windows)** workflow
(`.github/workflows/kiosk-release.yml`), triggered manually with a
`patch`/`minor`/`major` bump. It fans the new version across `tauri.conf.json`,
`src-tauri/Cargo.toml`, and `package.json` (`scripts/set-version.mjs`), tags
`kiosk-vX.Y.Z`, builds a signed bundle, and publishes the installer, `.sig`, and
`latest.json` to a GitHub Release. The deployed kiosks poll
`https://github.com/<owner>/<repo>/releases/latest/download/latest.json`.

The runtime updater config (public key + endpoint) is **not committed**; the
release workflow injects it via a `--config` overlay so `kiosk-ci.yml` keeps
building without keys and no key material lives in the repo. Base
`tauri.conf.json` keeps `createUpdaterArtifacts: false`.

#### One-time setup before the first release

1. Generate a signing keypair:

   ```bash
   pnpm --filter @gaming-cafe/kiosk exec tauri signer generate -w kiosk.key
   ```

2. In **Settings → Secrets and variables → Actions**:
   - Secret `TAURI_UPDATER_PUBKEY` = the printed **public** key.
   - Secret `TAURI_SIGNING_PRIVATE_KEY` = contents of `kiosk.key` (private).
   - Secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` = the key password (if set).
3. Keep a sealed backup of the private key **outside CI**. Losing it means
   shipped kiosks can no longer verify updates and must be re-imaged.

> Note: installs are `perMachine` (NSIS), so applying an update requires
> elevation (a UAC prompt). Run the kiosk with rights to self-update, or revisit
> a `perUser` install / elevated update path (ADR-0028 risks).

Authenticode signing of the installer (to avoid SmartScreen) remains optional
via the `signCommand` hooks above and is independent of the updater key.
