# DRAFT-0033: Dynamic plan wallet deduction by time of day

**Status**: Proposed
**Date**: 2026-06-07
**Deciders**: Platform team

**Relates to**: [DRAFT-0014](DRAFT-0014-player-plan-balances-wallet.md), [DRAFT-0024](DRAFT-0024-kiosk-session-heartbeat-and-staff-end.md)

## Context

Time-based plans grant a fixed number of wallet minutes on purchase. Cafe operators
want **time-of-day consumption rates** without changing the purchased wallet size:

- **Peak hours**: wallet burns faster (`ratio > 1`).
- **Normal hours**: 1 wall-clock minute = 1 wallet minute.
- **Low hours**: wallet burns slower (`ratio < 1`).

The kiosk HUD must reflect the current burn rate between server syncs. The backend
must remain authoritative for deductions and remaining time.

## Decision

### Plan catalog

Add to `plans`:

- `dynamicDeductionEnabled` (boolean, default false)
- `deductionProfile` (JSONB, required when enabled)

Profile shape:

```json
{
  "peakWindowStart": "18:00:00",
  "peakWindowEnd": "23:00:00",
  "peakRatio": 1.5,
  "lowWindowStart": "07:00:00",
  "lowWindowEnd": "11:00:00",
  "lowRatio": 0.8
}
```

Validation: `peakRatio > 1`, `0 < lowRatio < 1`, windows support wrap-around,
peak and low windows must not overlap.

### Balance snapshot

`player_plan_balances.deductionProfile` stores a copy at purchase/recharge when
the source plan has dynamic deduction enabled. Plan edits do not affect active wallets.

### Consumption model

`wallet_minutes_consumed = wall_minutes × ratio(venue_local_time)`.

`usage_sessions.timeCreditsConsumed` stores cumulative **weighted wallet minutes**
charged. Heartbeat and session end deduct only the delta not yet charged (DRAFT-0024).

Venue timezone: `CAFE_TZ` (default `Asia/Kolkata`).

### Kiosk API

Extend `KioskSessionResponseDto` with:

- `deductionProfile` (nullable)
- `cafeTimezone` (string)
- `timeCreditsConsumed` (number)

### Frontend session clock

Admin and kiosk use `useSessionRemainingMinutes` (`@gaming-cafe/utils`) to
interpolate the countdown between server syncs using the current venue-local
ratio. Display is not used for billing. See [session-time-clock.md](../session-time-clock.md).

## Consequences

### Positive

- Operators configure peak/low windows per plan with live preview.
- Players always receive full catalog minutes; burn rate varies during play.
- Kiosk countdown accelerates/decelerates to match server math.

### Negative

- Session remaining time is harder to reason about across window boundaries.
- Requires shared Rust/TS deduction logic kept in sync via fixture tests.

### Risks

| Risk | Mitigation |
|------|------------|
| Double deduction on heartbeat + end | `timeCreditsConsumed` delta model |
| Client/server drift | Server authoritative; client re-syncs on heartbeat/poll |
| Wrap-around window bugs | Unit tests for overnight windows |

## Alternatives Considered

### Purchase-time minute override

Reduce minutes granted at sale during promo windows. Rejected — user requires full
wallet credits with variable session burn.

### Global cafe-wide deduction schedule

Single configuration for all plans. Rejected — per-plan windows and ratios required.

## References

- [dynamic_deduction_pricing plan](/.cursor/plans/dynamic_deduction_pricing_9d54d707.plan.md)
