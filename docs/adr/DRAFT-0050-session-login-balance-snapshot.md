# ADR-0050: Session login balance snapshot

**Status**: Proposed
**Date**: 2026-06-26
**Deciders**: Platform team (gaming-cafe working group)

**Relates to**: [DRAFT-0014](DRAFT-0014-player-plan-balances-wallet.md) (wallet model), [DRAFT-0045](DRAFT-0045-persisted-notifications-and-activity-log.md) (activity log)

## Context

`usage_sessions` stores only `balanceId` and JOINs the live
`player_plan_balances` row for list/detail views. When staff recharges or
extends a plan mid-session, `remainingMinutes` and `sourcePlanId` on the wallet
change. Admin sessions and activity log then show post-recharge values — the
balance and plan at player login are lost for audit purposes.

Live wallet data must still drive the countdown and recharge UX; operators also
need an immutable snapshot of wallet minutes and source plan at session start.

## Decision

Add nullable snapshot columns to `usage_sessions`:

```sql
ALTER TABLE usage_sessions
  ADD COLUMN IF NOT EXISTS "walletMinutesAtStart" INTEGER,
  ADD COLUMN IF NOT EXISTS "sourcePlanIdAtStart" UUID REFERENCES plans(id);
```

- Written **once** at session INSERT in `SessionService::start` from the balance
  row loaded before create. Crash-resume must not overwrite them.
- No backfill for historical sessions — UI shows `—` when NULL.
- `UsageSessionResponse` exposes `walletMinutesAtStart`, `sourcePlanIdAtStart`,
  and enriched `planAtStart` (JOIN on snapshot plan id).
- `session.started` activity payload includes `walletMinutesAtStart` for logging.

Live `balance.remainingMinutes` and `balance.plan` remain the current wallet state.

## Consequences

### Positive

- Operators can compare login balance vs current balance after mid-session recharge.
- Queryable snapshot without parsing activity_log JSON.
- Kiosk HUD unchanged (uses live wallet via WebSocket).

### Negative

- Two plan references per session (snapshot vs live wallet).
- Nullable columns on historical rows.

### Risks

| Risk | Mitigation |
|------|------------|
| Snapshot not set on create | Integration test asserts value after start |
| Resume overwrites snapshot | Only set in INSERT path, not resume |

## Alternatives considered

### A. Derive from activity_log payload only

- Pros: No migration.
- Cons: Not queryable; payload lacks raw wallet minutes today; no data for old sessions. **Rejected**.

### B. Ledger reconstruction

- Pros: Full audit trail exists.
- Cons: Complex; does not capture plan at login. **Rejected**.

## References

- Plan: session login balance snapshot (2026-06-26)
- [`apps/backend/src/repositories/session_repo.rs`](../../apps/backend/src/repositories/session_repo.rs)
- [`apps/backend/src/services/balance_service.rs`](../../apps/backend/src/services/balance_service.rs)
