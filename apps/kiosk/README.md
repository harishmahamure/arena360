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

Configuration, release behavior, diagnostics, and security boundaries are in
[deployment and operations](../../docs/DEPLOYMENT.md).

## Windows startup and shortcut protection

Installed release builds register startup at Windows sign-in and run a hidden
recovery process that restarts the kiosk after an unexpected exit. Updates pause
recovery; a failed or canceled update restores it. Intentional exits from staff
setup do not trigger recovery. Development builds do not register startup, run
recovery, or close desktop applications.

Each installed kiosk launch closes applications already open in the same Windows
session. Applications receive a close request and have three seconds to exit
before termination. Windows services, shell processes, WebView2, the kiosk, and
its installer/ancestor processes are preserved. Unsaved work in applications
that refuse to close can be lost.

While locked, the native keyboard hook blocks both Windows keys and all Win+key
combinations (including desktop, Settings, Run, Explorer, and Task View),
Ctrl+Esc/Ctrl+Shift+Esc, Alt+Esc, Alt+F4, Alt+Space, and dedicated app-launch keys.
Alt+Tab and normal browser/game shortcuts remain available. Switching to the
desktop or an application outside the kiosk's tracked launch list returns focus
to the kiosk or the last tracked application. Staff admin login preserves
`SetupRelaxed`: shortcut protection and foreground enforcement turn off and the
kiosk can be minimized.

Ctrl+Alt+Delete is handled by Windows before app hooks. Full suppression needs
[Windows Keyboard Filter](https://learn.microsoft.com/en-us/windows/configuration/keyboard-filter/),
which is available on Enterprise, Education, and IoT Enterprise editions. Use a
standard Windows kiosk account. From an elevated Windows PowerShell prompt:

```powershell
# Enable the Windows feature, then reboot the station.
& 'C:\Program Files\Arena360 Station Management\scripts\configure-keyboard-filter.ps1' -EnableFeature
# After reboot, run again to configure Ctrl+Alt+Delete suppression.
& 'C:\Program Files\Arena360 Station Management\scripts\configure-keyboard-filter.ps1'
```

Adjust the path to the installation directory. The installer also configures
Ctrl+Alt+Delete suppression if Keyboard Filter is already enabled. The script
preserves existing rules/settings for restoration on uninstall, keeps Alt+Tab
available, and disables the default five-Windows-key breakout. Restart after
applying the settings. This OS restriction remains active during staff setup;
a separate Windows administrator account is exempt for servicing. On Home/Pro,
the secure Ctrl+Alt+Delete screen cannot be suppressed by this app. Keyboard
Filter does not operate in Remote Desktop sessions.

For deployment verification, use a disposable Windows station: open Notepad and
Chrome before launching the installed kiosk; verify startup cleanup, sign-in
startup, and recovery after terminating only the main kiosk process. Launch an
approved game/browser and verify Alt+Tab and its ordinary shortcuts. Check
Win+D/R/I/E/Tab/L, Ctrl+Esc, Ctrl+Shift+Esc, and Alt+F4 while locked. Check
Ctrl+Alt+Delete locally after configuring Keyboard Filter. Finally, sign in as
staff, minimize the kiosk, and verify that returning to locked mode restores
protection. Test update failure and successful installation/relaunch as well.
