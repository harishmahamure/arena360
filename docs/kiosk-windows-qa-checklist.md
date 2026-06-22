# Kiosk Windows QA checklist

Manual validation for overlay z-order, session-end cleanup, and client session clock.
Run on a **Windows 10/11** station with Riot Client / Valorant (or Steam) installed.

**Related:** [ADR-0020](adr/0020-kiosk-windows-lockdown.md), [session-time-clock.md](session-time-clock.md)

## Prerequisites

- [ ] Kiosk build deployed (`pnpm --filter kiosk tauri build` or CI artifact)
- [ ] Backend reachable; device provisioned and player account with time balance
- [ ] Admin access to force-end sessions from `SessionDetailPage`

## Z-order (Part 1)

| # | Steps | Expected | Pass |
|---|--------|----------|------|
| 1 | Launch Valorant (or another game) via Riot Client from kiosk library | Launcher and game windows come to front; kiosk tracks process | |
| 2 | **Alt+Tab** back to kiosk during session | Kiosk session UI visible; **Riot social overlay must not cover kiosk** | |
| 3 | **Alt+Tab** between game and kiosk several times | OS z-order respected; **no TOPMOST flicker** or fighting | |

## Session-end cleanup (Part 2)

| # | Steps | Expected | Pass |
|---|--------|----------|------|
| 4 | With Riot/Steam/game + browser/Discord open, end session **voluntarily** | All user apps closed; kiosk login fullscreen within ~3 s | |
| 5 | Repeat with apps launched outside kiosk allow-list (if any) | User-session sweep closes them | |
| 6 | Admin **force-end** while player is in-game | **Immediate** cleanup + login (no 5 min overlay) | |
| 7 | Login screen after force-end | Optional notice: “Your session was ended by staff.” | |
| 8 | WS `session.ended` while in game (force-end or auto-end from another path) | Cleanup runs even without poll | |
| 8a | Launch game → **Close app** from running-apps bar | Kiosk stays running; session HUD remains; can launch again | |
| 8b | End session / logout with game running | Games closed; **login screen with video or gradient**; kiosk process still running | |
| 8c | Fresh install with CDN blocked | Login shows bundled `/launch.webm` loop or gradient (not blank) | |

## Session clock & auto-end (Part 4)

| # | Steps | Expected | Pass |
|---|--------|----------|------|
| 9 | Start session; watch kiosk HUD countdown | Ticks every second from session start (no 15 s jumps) | |
| 10 | Compare same session on **admin Sessions list** | Clock matches kiosk (±1 s); stable without 30 s list refetch | |
| 11 | Let countdown reach **≤10 seconds** | Auto `PATCH …/end` once; cleanup + login | |
| 12 | Mid-session **recharge** (admin/plan purchase) | WS `balance.updated` re-anchors; countdown increases | |

## Setup & edge cases

| # | Steps | Expected | Pass |
|---|--------|----------|------|
| 13 | Open setup via **Staff login**, then exit without player session | **No** session-end process sweep | |
| 14 | Disconnect network briefly during session | Offline banner; local countdown continues; reconnect replays end intents | |
| 15 | Reconnect WS after brief outage (session still active) | One-shot `GET /kiosk/sessions/current` re-anchors; no periodic poll | |

## Logon autostart (Part 5)

Requires NSIS install with autostart (default). Run installer logged in as the kiosk user.

| # | Steps | Expected | Pass |
|---|--------|----------|------|
| 16 | Kill kiosk process from Task Manager while on login screen | Kiosk **stays closed** until next logon (no crash relaunch) | |
| 17 | Enter setup, click **Exit to desktop**, close kiosk | Desktop usable; kiosk does **not** relaunch until next logon | |
| 18 | Log off and log on again (or reboot with auto-logon) | Kiosk starts automatically | |
| 19 | Reboot station with auto-logon configured | **Arena360 Kiosk** task runs; kiosk appears without manual launch | |
| 20 | Uninstall kiosk | `Arena360 Kiosk` task removed (legacy `Arena360 Watchdog` too) | |
| 21 | Run `verify-station-startup.ps1` from repo after configure | Reports no issues; `-Repair` fixes user/path mismatch | |

## Sign-off

| Role | Name | Date | Notes |
|------|------|------|-------|
| QA | | | |
| Eng | | | |
