# ADR-0048: Re-introduce kiosk watchdog sidecar

**Status**: Proposed
**Date**: 2026-06-26
**Deciders**: Platform team

**Supersedes**: [ADR-0041](0041-remove-watchdog-logon-autostart.md) (partial — logon task target changes from main exe to watchdog)
**Relates to**: [ADR-0020](0020-kiosk-windows-lockdown.md), [ADR-0028](0028-kiosk-release-pipeline-and-auto-update.md)

## Context

[ADR-0041](0041-remove-watchdog-logon-autostart.md) removed the `arena360-watchdog.exe` sidecar in
favor of a direct ONLOGON scheduled task launching the main kiosk executable. That simplified
deployment but removed crash/kill auto-relaunch and made in-app NSIS updates harder to coordinate
(perMachine binary replace races with a running process).

Operators now require:

1. **Auto-relaunch** when the kiosk exits unexpectedly (crash, kill, OOM).
2. **Update handoff** — force-close tracked games, pause watchdog, apply update, relaunch cleanly.
3. **Maintenance pause** — setup mode and exit-to-desktop must not fight the watchdog.

ADR-0041 identified stale `watchdog.pause` after reboot as a failure mode. This ADR addresses
that with TTL-based pause files and mandatory expiry cleanup on watchdog startup.

## Decision

1. **Restore** `arena360-watchdog.exe` as a lightweight sidecar shipped in the NSIS bundle.
2. **Register** ONLOGON scheduled task **Arena360 Watchdog** to run the sidecar (not the main exe).
3. **Watchdog behavior**: poll every 2 s; spawn main kiosk when absent unless a valid pause file exists.
4. **Pause IPC**: `%ProgramData%\Arena360\watchdog.pause` as JSON `{ "expiresAt": "<ISO8601>", "reason": "<string>" }`.
5. **Boot safety**: on watchdog startup, delete pause file when `expiresAt` is in the past.
6. **Tauri commands**: `set_watchdog_pause(duration_secs, reason)`, `clear_watchdog_pause`.
7. **Lockdown integration**:
   - `SetupRelaxed` / exit-to-desktop → `set_watchdog_pause(900, "maintenance")`
   - Normal `Locked` startup → `clear_watchdog_pause()`
8. **Update path** (`prepare_for_update`): close/kill tracked apps, `set_watchdog_pause(600, "update")`, then NSIS install + relaunch; new process clears pause on boot.
9. **Keep** main-process single-instance mutex (`Global\Arena360KioskInstance`).
10. **Keep** watchdog single-instance mutex (`Global\Arena360WatchdogInstance`).
11. **Keep** manual `configure-station.ps1` for fleet setup (not NSIS post-install).

## Consequences

### Positive

- Crash/kill recovery within ~10 s without user logon
- Update handoff coordinated via pause file; watchdog does not relaunch stale binary during NSIS replace
- Setup and exit-to-desktop no longer require disabling autostart tasks

### Negative

- Two-process startup chain returns (operational complexity)
- Sidecar adds build pipeline surface and Rust test burden
- Stale pause remains a risk if TTL logic regresses (mitigated by startup purge)

### Risks

| Risk | Mitigation |
|------|------------|
| Stale pause after reboot | TTL + watchdog startup purge |
| Relaunch loop during bad install | Update pause (10 min) + backoff on repeated spawn failures |
| perMachine NSIS UAC | Document; idle-phase gate unchanged |

## Alternatives Considered

### Keep ADR-0041 direct logon task only

Rejected: no crash relaunch; update races persist.

### Windows Service instead of sidecar

Rejected: over-engineered; service account and recovery policy add fleet burden.

### Watchdog without pause file (kill-only coordination)

Rejected: cannot distinguish intentional maintenance from crash.

## Implementation Notes

- Shared pause-file module: `apps/kiosk/watchdog-common/` (Rust crate used by kiosk + watchdog).
- Watchdog binary: `apps/kiosk/watchdog/`.
- Marker file `%ProgramData%\Arena360\station.json` stores install dir for kiosk exe resolution.

## References

- [KIOSK-WINDOWS-DEPLOYMENT.md](../KIOSK-WINDOWS-DEPLOYMENT.md)
- [configure-station.ps1](../../apps/kiosk/scripts/windows/configure-station.ps1)
- [PLANNER-KIOSK.md](../PLANNER-KIOSK.md) — `kiosk-watchdog` task spec
