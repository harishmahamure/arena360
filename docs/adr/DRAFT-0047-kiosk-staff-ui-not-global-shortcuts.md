# ADR-0047: Kiosk staff actions via UI; launcher foreground on launch

**Status**: Proposed
**Date**: 2026-06-22
**Deciders**: Platform team (gaming-cafe kiosk working group)

**Supersedes**: [ADR-0020](0020-kiosk-windows-lockdown.md) amendment 2026-05-30 (Ctrl+Shift+A setup entry) and subsequent staff shortcut additions (Ctrl+Shift+B, Ctrl+Shift+H)

**Implements**: US-KLOCK-001 (shell lockdown without in-game staff hotkey leakage), launcher/game foreground at kiosk launch

## Context

The kiosk native keyboard hook (`WH_KEYBOARD_LL`) registered global **Ctrl+Shift+A** (setup), **Ctrl+Shift+B** (login lockout reset), and **Ctrl+Shift+H** (return to shell). These combinations fire even when a launched game has foreground focus, causing unintended setup transitions and lockout clears while playing.

Separately, launcher-mediated launches (especially Riot Client) did not call `SetForegroundWindow` on the launcher UI. Riot’s login window runs in `riotclientux.exe` and was classified as a suppressible overlay, so it stayed behind the fullscreen kiosk while Steam often self-activated.

Staff still need setup entry and login lockout reset on the **login screen** without global hotkeys.

## Decision

1. **Remove global staff shortcuts** — Delete Ctrl+Shift+A/B/H handling from the native keyboard hook and from React `keydown` listeners. Shell blocking (Win keys, Alt+F4, etc.) and Alt+Tab allowance unchanged.

2. **Staff actions on login UI only**
   - **Staff login** button → existing `enterSetup()` (unchanged)
   - **Clear sign-in lock** button → visible when player login lockout is active; calls `resetLoginLockoutByStaff()`

3. **Return to kiosk during session** — Alt+Tab only (no Ctrl+Shift+H). `on_kiosk_focused` re-applies session foreground when the kiosk regains focus.

4. **Launcher foreground at launch** — After `launch_allowed` spawns a launcher executable, poll for the largest visible launcher window (including child PIDs such as `riotclientux.exe`) and call `bring_hwnd_to_foreground` once.

5. **App/game foreground after launcher login** — The process monitor foregrounds the main tracked window **once per entry** when a non-launcher-login HWND first appears (e.g. Valorant after Riot sign-in).

6. **Overlay classification split**
   - `is_small_or_tool_window` — exclude tiny/tool windows from HWND selection only
   - `is_suppressible_overlay` — full overlay rules when returning focus to the kiosk (`suppress_tracked_overlays`)

7. **SetupRelaxed** — Uninstall the keyboard hook entirely (no idle staff-combo-only mode).

## Consequences

### Positive

- Staff shortcuts no longer leak into games (Ctrl+Shift+A/B common in game bindings)
- Riot Client and other launchers appear in front at launch
- Post-login games/apps foreground once without repeated focus stealing

### Negative

- Staff cannot open setup or clear lockout from the login screen via keyboard alone
- Returning to Arena360 during gameplay requires Alt+Tab (no global H shortcut)

### Risks

| Risk | Mitigation |
|------|------------|
| `SetForegroundWindow` ignored by anti-cheat | Document as best-effort; same as any shell |
| Slow launcher UI (>15s) | 15s launch poll; monitor continues tracking |

## Alternatives Considered

### Gate hotkeys to kiosk-foreground only

- Pros: Keeps keyboard paths for staff
- Cons: Does not fix in-game leakage when kiosk briefly has focus; rejected

### Keep Ctrl+Shift+H only

- Pros: Quick return to shell
- Cons: Still a global binding that can conflict with games; user chose remove all

## References

- [ADR-0020](0020-kiosk-windows-lockdown.md)
- [ADR-0019](0019-kiosk-device-allow-list.md)
- `apps/kiosk/src-tauri/src/process.rs`
- `apps/kiosk/src-tauri/src/lockdown/keyboard.rs`
