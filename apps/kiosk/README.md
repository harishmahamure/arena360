# Arena360 Windows kiosk

The kiosk is the locked-down Windows station client described in
[the current system documentation](../../docs/SYSTEM.md). It handles device
provisioning, player authentication and registration, timed sessions, the local
software allow-list, process launch and cleanup, ordering, realtime updates,
offline grace, and idle-only auto-update.

## Develop and test

```bash
cp apps/kiosk/.env.example apps/kiosk/.env
pnpm kiosk:dev
pnpm --filter @gaming-cafe/kiosk test
pnpm --filter @gaming-cafe/kiosk test:rust
```

Build the Windows NSIS installer on Windows:

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
<<<<<<< HEAD
=======

Windows application launches use `ShellExecuteExW` with the executable's directory
as the working directory and visible windows, matching Explorer launch semantics.
Arguments retain spaces, quotes, and empty values. Windows handles compatibility
settings and any required elevation prompt. If the normal shell launch returns
error 740 (elevation required), the kiosk retries once with `runas`, asking for
Windows administrator approval. Canceling that prompt reports a launch error;
other errors do not trigger elevation. The allow-list and active-session checks
still apply. Launching an existing application instance keeps it tracked, and new
launcher children are discovered from full process snapshots. Launched
applications retain their own window style and fullscreen settings. The kiosk
does not take focus back from an Alt+Tab selection while an application finishes
starting.
>>>>>>> 4cb036a (kiosk major update)
