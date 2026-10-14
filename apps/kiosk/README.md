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

Once startup session restoration confirms the station is signed out, installed
Windows builds close open desktop applications immediately and check again every
30 seconds while locked and signed out. The check reads process metadata only;
it does not scan drives or refresh CPU/memory statistics. Tray launchers, browsers,
and configured game executables are included along with their existing children.
Applications receive a close request and have three seconds to exit before forced
termination. Windows services, shell processes, WebView2, the kiosk, its
installer/ancestor processes, and known security, driver, and anti-cheat processes
are preserved. Unrelated background processes are left alone.

The worker sleeps while disabled during player sessions, sign-in, staff Setup,
startup restoration, and boot errors. Restored offline sessions are preserved.
Returning to the login screen re-enables an immediate check; there is no cleanup
on startup until that signed-out state is confirmed. Login/Setup cancels pending
close and terminate actions, including during the three-second grace period.
Unsaved work in applications that refuse to close can be lost.

While locked, the native keyboard hook blocks both Windows keys and all Win+key
combinations (including desktop, Settings, Run, Explorer, and Task View),
Ctrl+Esc/Ctrl+Shift+Esc, Alt+Esc, and dedicated app-launch keys. Alt+F4,
Alt+Space, and context-menu keys are blocked when the kiosk has focus and remain
available in other applications. Alt+Tab is always allowed, and the background
guard does not redirect the user's selected window. Normal browser/game
shortcuts remain available. The hook renews itself every 250 ms to recover from
Windows silently removing it after a timeout. Shell enforcement uses Windows
foreground/window-show events, with a two-second fallback instead of fast polling.
While a tracked game or launcher has focus, the guard skips full window scans;
newly shown restricted windows are handled individually. Alt+Tab queues a check
without a polling burst. Window work is dispatched to the Tauri main thread, and
staff setup cancels queued checks before restoring the shell. Swallowed Windows-key
presses remain tracked until key release, including across renewal.
If Show Desktop gets through, the foreground guard restores the
last kiosk or tracked application window; it leaves application Alt+Tab targets
alone. `HKCU` policies (`DisableTaskMgr`, `NoWinKeys`) also restrict Task Manager
and shell hotkeys for the actual signed-in kiosk account. Original policy values
are saved before lockdown, retained across crash recovery, and restored during
staff setup and normal exit. If a managed account denies policy writes, the kiosk
logs a warning and the hook plus Task Manager window guard remain active.
Staff admin login preserves
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

Windows application launches use `ShellExecuteExW` with the executable's directory
as the working directory and visible windows, matching Explorer launch semantics.
Arguments retain spaces, quotes, and empty values. Windows handles compatibility
settings and any required elevation prompt. If the normal shell launch returns
error 5 (access denied, including `0x80070005`) or 740 (elevation required), the
kiosk retries once with `runas`, asking for Windows administrator approval.
Canceling that prompt reports a launch error; other errors do not trigger
elevation. A persistent denial reports the executable path and asks staff to
check Windows permissions and application restrictions. The allow-list and active-session checks
still apply. Launching an existing application instance keeps it tracked, and new
launcher children are discovered from full process snapshots. Launched
applications retain their own window style and fullscreen settings. The kiosk
does not take focus back from an Alt+Tab selection while an application finishes
starting.

In each station's staff Setup panel, Allowed software searches installed apps on
entry and offers **Auto search installed apps** for another scan. Select a game
and use **Auto-detect launcher** to discover its platform client, including Riot's
custom-drive installation metadata. Both Valorant bootstrap and shipping
executables resolve to Riot Client. Older entries without a saved launcher profile
also resolve at launch time; a missing Riot Client reports a setup error instead
of starting Valorant directly.

The **Launch option** selector supports:

- **Automatic**: detected launch profiles; Riot opens the client for sign-in and
  Play, while other platform profiles retain their game launch arguments.
- **Start game through platform launcher**: starts the client with the configured
  game arguments, such as Riot's product/patchline or Steam's app ID.
- **Open platform launcher only**: opens the client without game arguments.
- **Launch game executable directly**: explicitly bypasses the saved profile.

Launcher arguments may be empty. Manual launcher settings, direct-launch choices,
and the **Run as Windows administrator** option persist across scans and JSON
export/import. Administrator launch uses Windows approval for the selected game
or client. **Test launch** uses the same route in Setup without a player session.
