# ADR-0041: Remove watchdog sidecar — logon scheduled task autostart

**Status**: Accepted
**Date**: 2026-06-17
**Deciders**: Platform team

**Supersedes**: [ADR-0020](0020-kiosk-windows-lockdown.md) (2026-06-15 watchdog pause IPC amendment)
**Relates to**: [ADR-0028](0028-kiosk-release-pipeline-and-auto-update.md)

## Context

Windows kiosk stations used a two-process startup chain: an **Arena360 Watchdog**
ONLOGON scheduled task ran `arena360-watchdog.exe`, which polled every 5s and
spawned the main kiosk when absent. Pause-file IPC (`%ProgramData%\Arena360\watchdog.pause`)
prevented relaunch during setup, exit-to-desktop, updates, and power handoff.

After PC restart, stations often failed to start because:

- Stale `watchdog.pause` (e.g. maintenance/setup before reboot) blocked spawn for up to 15 min
- `schtasks` registration failed for the wrong Windows user during silent deploy
- The sidecar added operational complexity without matching fleet needs

Operators requested **simple autolaunch on Windows login** without crash-relaunch monitoring.

## Decision

1. **Remove** `arena360-watchdog.exe` sidecar binary, build pipeline, and all pause-file IPC.
2. **Register** scheduled task **Arena360 Kiosk** at **ONLOGON** to run the main kiosk
   executable directly (e.g. `Arena360 Station Management.exe`).
3. **Keep** optional auto-logon in `configure-station.ps1` (installer prompt / `-AutoLogonPassword`).
4. **Keep** optional HKLM hardening and logon-task setup via manual `configure-station.ps1` (not in NSIS).
5. **Keep** kiosk single-instance mutex (`Global\Arena360KioskInstance`) in the main process.
6. **Migrate** on install: delete legacy **Arena360 Watchdog** task if present.

## Consequences

### Positive

- Reboot → user logon → kiosk starts without pause-file failure modes
- Simpler bundle (one exe), faster builds, less Rust surface area
- Exit to desktop and setup no longer require watchdog coordination

### Negative

- **No crash/kill auto-relaunch** — kiosk stays down until next user logon or manual start
- Auto-update no longer pauses a watchdog; perMachine NSIS replace may race if process
  does not exit cleanly (mitigated: `prepare_for_update` + idle-phase gate before `downloadAndInstall`)

## Alternatives Considered

### Keep watchdog, fix pause-on-reboot only

Rejected: treats symptom; sidecar complexity remains.

### HKLM Run key instead of scheduled task

Rejected: less explicit per-user binding; scheduled task already works with explicit user resolution.

### Windows Service

Rejected: over-engineered for v1; requires service account and recovery policy.

## References

- [KIOSK-WINDOWS-DEPLOYMENT.md](../KIOSK-WINDOWS-DEPLOYMENT.md)
- [configure-station.ps1](../../apps/kiosk/scripts/windows/configure-station.ps1)
