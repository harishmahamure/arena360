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

## Formula (matches backend `effective_remaining_for_session`)

```
consumed = weightedMinutesBetween(sessionStart, now, profile, cafeTz)
owed     = max(0, consumed - timeCreditsConsumed)
remaining = max(0, walletBalanceMinutes - owed)
```

- **Admin**: `walletBalanceMinutes` = `player_plan_balances.remainingMinutes`.
- **Kiosk**: derive `walletBalanceMinutes` when anchoring from server effective
  `remainingMinutes` + current `owed` (on start and `balance.updated`).

During peak windows the countdown moves **faster**; during low windows **slower**.

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
| Session start | Set `startTime`, wallet balance, profile, `timeCreditsConsumed` |
| WS `balance.updated` | Update wallet balance (kiosk derives from effective remaining) |
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
