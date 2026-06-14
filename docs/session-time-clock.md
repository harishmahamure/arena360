# Session time clock (admin + kiosk + console TV)

> How remaining session time is displayed on frontends and charged on the backend.
> Last updated: 2026-06-14.

## Summary

| Layer | Role |
|-------|------|
| **Backend** | Authoritative wallet balance and weighted minute charges at session **end** |
| **Frontend clock** | Local 1 s tick from `session.startTime` using shared `weightedMinutesBetween` |
| **Auto end** | Kiosk + console TV call `PATCH …/end` (`reason: auto`) at ≤ 10 s remaining |
| **Manual end** | Player logout, staff force-end — then native app cleanup on kiosk |

All clients use `useSessionRemainingMinutes` from `@gaming-cafe/utils` with the same
inputs: `sessionStartTime`, `walletBalanceMinutes`, `timeCreditsConsumed`,
`deductionProfile`, `cafeTimezone`.

All clients tick locally every **`SESSION_CLOCK_TICK_MS` (1000 ms)** — kiosk HUD,
admin `SessionRemainingClock`, staff dashboard “ending soon”, and console TV.

## Formula (matches backend `effective_remaining_for_session`)

```
consumed = weightedMinutesBetween(sessionStart, now, profile, cafeTz)
owed     = max(0, consumed - timeCreditsConsumed)
walletRemaining = max(0, walletBalanceMinutes - owed)
displayRemaining = min(walletRemaining, minutesUntilExpiry(expiryDate))
```

When plan expiry is sooner than wallet minutes, the countdown caps at expiry.
When wallet is lower, wallet minutes win. `expiryDate` comes from
`player_plan_balances` on session APIs (`balance.expiryDate`, kiosk `expiryDate`,
console TV `expiryDate`). Display-only — charging still happens at session end.

- **Admin** and **Kiosk**: `walletBalanceMinutes` = raw `player_plan_balances.remainingMinutes`
  from session start, login `activeSession`, `GET /kiosk/sessions/current`, or WS
  `balance.updated` (recharge). Display countdown is computed locally from
  `startTime` + wallet — never stored as `remainingMinutes` in kiosk state.

During peak windows the countdown moves **faster**; during low windows **slower**.

API responses also include `remainingMinutes` as server-computed **effective**
display remaining (console TV / legacy clients). Kiosk ignores this field.

### Admin `GET /sessions`

Enriched list and detail responses include:

- `balance.deductionProfile` — snapshot from `player_plan_balances` at purchase
- `cafeTimezone` — server `CAFE_TZ` (IANA) for peak/low window boundaries

Without `deductionProfile`, admin countdown falls back to **1 wallet minute per wall
minute** (1 display second per wall second). Always pass `deductionProfile`,
`cafeTimezone`, and `expiryDate` (when available) into `SessionRemainingClock` /
`useSessionRemainingMinutes`.

## Shared frontend implementation

```
packages/contracts/src/deductionProfile.ts  — weightedMinutesBetween, AUTO_END_REMAINING_SECONDS
packages/utils/src/lib/session-clock/
├── useSessionRemainingMinutes.ts
└── formatRemainingClock.ts
```

### Re-anchor events

| Event | Action |
|-------|--------|
| Session start | Set `startTime`, `walletBalanceMinutes`, profile, `timeCreditsConsumed` |
| WS `balance.updated` | Update `walletBalanceMinutes` (raw wallet from recharge) |
| WS `session.ended` | End session + cleanup (kiosk) |
| WS reconnect / app resume | One-shot `GET /kiosk/sessions/current` (kiosk only, when in session) |

No periodic `GET /kiosk/sessions/current` poll or `PATCH …/heartbeat` on kiosk.

### Auto-end threshold

`AUTO_END_REMAINING_SECONDS = 10` — kiosk and console TV fire end API once when
`floor(remainingMinutes × 60) <= 10`. Admin display only.

## Backend: authoritative charging

Weighted consumption is implemented in `apps/backend/src/services/session_service.rs`
and `apps/backend/src/services/deduction_profile.rs`.

### During session

No incremental heartbeat deduction in current kiosk builds. Display is client-side.

### Session end (all paths)

`session_service.end()` charges the full delta via `charge_session_delta` when
`time_credits_consumed` is not supplied.

## Related documents

- [DRAFT-0024: Kiosk session heartbeat and staff end](./adr/DRAFT-0024-kiosk-session-heartbeat-and-staff-end.md) — Amendment 2026-06-14
- [DRAFT-0033: Dynamic plan deduction](./adr/DRAFT-0033-dynamic-plan-deduction.md)
- [ADR-0020: Kiosk Windows lockdown](./adr/0020-kiosk-windows-lockdown.md) — Amendment 2026-06-14
- [Kiosk Windows QA checklist](./kiosk-windows-qa-checklist.md) — manual validation (Part 3)
