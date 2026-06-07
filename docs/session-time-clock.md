# Session time clock (admin + kiosk)

> How remaining session time is displayed on frontends and charged on the backend.
> Last updated: 2026-06-07.

## Summary

| Layer | Role |
|-------|------|
| **Backend** | Authoritative wallet balance and weighted minute charges |
| **Frontend clock** | Display-only interpolation between server syncs |
| **Session end** | Always uses backend calculation (except offline reconcile) |

Both **admin** and **kiosk** use the same hook: `useSessionRemainingMinutes` from
`@gaming-cafe/utils`. The backend never trusts client countdowns for billing.

## What the clock shows

The UI displays **wallet minutes remaining** on the player's active balance — not
a fixed wall-clock end time derived from `startTime + remainingMinutes`.

- **Normal plans** (`ratio = 1`): one wallet minute burns per wall minute.
- **Dynamic plans** ([DRAFT-0033](./adr/DRAFT-0033-dynamic-plan-deduction.md)):
  burn rate follows the balance snapshot's `deductionProfile` in venue local time
  (`CAFE_TZ`, default `Asia/Kolkata`).

During peak windows the countdown moves **faster**; during low windows **slower**.

## Shared frontend implementation

```
packages/utils/src/lib/session-clock/
├── useSessionRemainingMinutes.ts   # shared hook
└── formatRemainingClock.ts         # HH:MM:SS / label formatters
```

### Hook behaviour

1. **Anchor** when the server sends a new `remainingMinutes` (poll, heartbeat,
   WebSocket `balance.updated`, or session start).
2. **Tick** every second locally:
   - `localRemaining = anchor.remaining − elapsedWallMinutes × currentRatio`
3. **Re-anchor** whenever the authoritative value changes (recharge, heartbeat).

Kiosk passes `deductionProfile` and `cafeTimezone` from `KioskSessionResponseDto`.
Admin passes `deductionProfile` from the linked `player_plan_balances` record
(enriched via `/player-plans` lookup).

### Where it is used

| App | Component | Sync source |
|-----|-----------|-------------|
| Kiosk | `SessionPage` → `SessionNav` | Heartbeat (2 min), poll (15–60 s), WS |
| Admin | `SessionsPage` → `SessionRemainingClock` | List refetch every 30 s (active tab) |
| Admin | `SessionDetailPage` → `SessionRemainingClock` | Detail refetch every 30 s (active) |
| Admin | `DashboardLayout` staff notifications (10/5/1 min) | Shared interpolation + 30 s session refetch |
| Backend | Player login `activeSession.remainingMinutes` | `effective_remaining_for_session` (weighted) |

The frontend clock **must not** call session end when it reaches zero. Expiry is
handled by the kiosk (auto-end API) or the next server heartbeat.

## Backend: authoritative charging

Weighted consumption is implemented in `apps/backend/src/services/session_service.rs`
and `apps/backend/src/services/deduction_profile.rs`.

### During session (heartbeat)

1. Load open session + balance (with snapshot `deductionProfile`).
2. `weighted_minutes_between(lastChargePoint, now, profile, cafe_tz)`.
3. Deduct delta from `player_plan_balances.remainingMinutes`.
4. Persist cumulative `usage_sessions.timeCreditsConsumed`.
5. If `remainingMinutes ≤ 0`, call `auto_end_expired` (`reason: auto`).

### Session end (all paths)

`session_service.end()` charges the final delta when `time_credits_consumed` is
**not** supplied in the request:

| End path | Reason | Who triggers | Client sends `timeCreditsConsumed`? |
|----------|--------|--------------|--------------------------------------|
| Kiosk player logout | `voluntary` | Kiosk `PATCH /kiosk/sessions/{id}/end` | No |
| Wallet exhausted | `auto` | Backend heartbeat / kiosk poll at 0 | No |
| Admin force-end | `force` | Admin `PATCH /sessions/{id}/end` | No |
| Staff end (detail) | (optional) | Admin form | No |
| Offline reconnect | `offline_reconcile` | Kiosk queued intent | Yes (exception) |

Balance updates always come from `charge_session_delta()` using **server
`end_time`** (or `Utc::now()`), never from the frontend countdown.

### Formula

```
wallet_minutes_consumed = Σ (wall_minutes_in_window × ratio_for_that_window)
```

`timeCreditsConsumed` on the session stores the running total of wallet minutes
charged. Heartbeat and end only deduct `total − already_charged`, preventing
double billing.

## Operator expectations

- **Purchase**: player receives the full catalog `timeCredits` wallet.
- **Play**: visible countdown reflects current burn rate; staff see the same
  logic on the active sessions list.
- **Recharge mid-session**: server pushes new `remainingMinutes`; both UIs
  re-anchor immediately.
- **Force / staff end**: backend computes final charge at end time; kiosk receives
  `session.ended` / `session.force_logout` with updated balance.

## Related documents

- [DRAFT-0024: Kiosk heartbeat and staff end](./adr/DRAFT-0024-kiosk-session-heartbeat-and-staff-end.md)
- [DRAFT-0033: Dynamic plan deduction](./adr/DRAFT-0033-dynamic-plan-deduction.md)
- [REQUIREMENTS-KIOSK.md](./REQUIREMENTS-KIOSK.md) — session sync and countdown UX
