# @gaming-cafe/kiosk

Windows Tauri 2 + React 19 in-cafe kiosk. See [REQUIREMENTS-KIOSK.md](../../docs/REQUIREMENTS-KIOSK.md) and [PLANNER-KIOSK.md](../../docs/PLANNER-KIOSK.md).

## Environment

Copy the example env file and point it at your backend (default Axum port is `3000` locally):

```bash
cp apps/kiosk/.env.example apps/kiosk/.env
# edit VITE_API_URL if needed
```

| Variable | Purpose | Default |
|----------|---------|---------|
| `VITE_API_URL` | REST API base URL (WebSocket uses `/realtime` on the same host) | `http://localhost:3000` |
| `VITE_OFFLINE_GRACE_MINUTES` | Minutes a session keeps counting locally after the backend is unreachable before the station re-locks | `5` |

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

This produces both bundles under `apps/kiosk/src-tauri/target/release/bundle/`:

- **NSIS** (`nsis/*-setup.exe`) — recommended; per-machine install (`installMode: perMachine`).
- **MSI** (`msi/*.msi`) — for Group Policy / SCCM fleet deployment.

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

App-level lockdown blocks Alt+Tab, the Windows keys, Alt+F4 and Ctrl+Shift+Esc.
Ctrl+Alt+Del (the Secure Attention Sequence) cannot be intercepted from user mode.
For a hardened station, also apply at the OS level:

- Assigned Access / kiosk account, or group policy `DisableLockWorkstation` and
  `DisableTaskMgr`.
- Auto-login of the kiosk account and the app set to launch on logon.

### Code signing (release)

Production installers should be Authenticode-signed to avoid SmartScreen prompts.
Set the signing certificate via Tauri's Windows signing env vars in CI
(`TAURI_SIGNING_*` / `signCommand`) — see the Tauri v2 Windows signing guide. Signing
is intentionally left unconfigured in-repo so no certificate material is committed.

### Auto-update

Auto-update is **disabled** (`bundle.createUpdaterArtifacts: false`). To enable it
later, add the `tauri-plugin-updater` plugin, a signing key pair, and a
`plugins.updater` block with release endpoints, then flip the flag.
