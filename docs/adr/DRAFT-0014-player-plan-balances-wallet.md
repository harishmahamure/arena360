# ADR-0014: Player plan balances wallet model

**Status**: Proposed
**Date**: 2026-05-29
**Deciders**: Platform team

## Context

The current `player_plans` table (inherited from the TypeORM baseline) creates one
row per plan purchase. It carries fields for a "move to next plan" workflow
(`movedToPlanId`, `movedCreditsCount`, status `moved_to_next_plan`) that were
never wired into the Rust backend.

Business requirements have simplified: only two plan kinds exist:

- **Time Plan** (`time_based`): a minutes wallet with a validity window in days.
  Rechargeable — buying the same plan again tops up the minutes and resets expiry.
- **Happy Hours** (`weekend_special`): a minutes wallet restricted to a daily
  time window (e.g. 06:00–11:59). Always purchased fresh; no carry-over.

A player may hold both concurrently if they cover different device scopes.

The per-purchase model makes recharging awkward: every top-up creates a new row,
and the legacy "moved" fields were supposed to link them but are unused. Active
sessions point at specific `player_plans` rows via
`usage_sessions."playerPlanId"`, so plan changes scatter the session history.

## Decision

Replace `player_plans` with a **wallet model** backed by three new database
objects:

1. **`player_plan_balances`** — one row per (player, deviceType, deviceSubType,
   kind) while active. Holds `remainingMinutes`, `expiryDate`, and an optional
   time window snapshot.
2. **`player_plan_ledger`** — append-only audit trail: every purchase, recharge,
   session deduction, expiry, and manual adjustment is a signed delta row.
3. **Three Postgres enums** — `plan_kind` (`time`, `happy_hours`),
   `balance_status` (`active`, `expired`, `exhausted`, `cancelled`),
   `ledger_reason` (`purchase`, `recharge`, `session_usage`, `expiry`,
   `adjustment`, `migration`).

`usage_sessions` gains a `"balanceId"` FK to the new balances table. The old
`player_plans` table is renamed to `player_plans_legacy` and retained for
rollback and historical queries.

The `plans` catalog table is **kept** (preserving `transactions."planId"`). The
application restricts new plan creation to `time_based` and `weekend_special`.

### Recharge rules

- Time Plan only. On a completed `plan_purchase` where `planType = time_based`:
  1. Find the player's active balance for that device scope + `time` kind.
  2. If found: `remainingMinutes += plan.timeCredits`, `expiryDate = now + validity`.
  3. If not: create a fresh balance.
- Happy Hours always creates a fresh balance (separate kind).

### Migration strategy

Additive first: create new tables, backfill from `player_plans` (merge by
player+scope+kind: sum minutes, max expiry), add `balanceId` to sessions,
rename old table. App code switches in the same deploy. Rollback = revert app
+ drop new tables (legacy retained).

## Consequences

### Positive

- Recharge is a single UPDATE + ledger INSERT (no row proliferation).
- Full audit trail via the ledger (replaces unused `movedTo` fields).
- Partial unique index guarantees at most one active balance per scope+kind.
- Session history unified under the balance rather than scattered across purchase rows.

### Negative

- New migration required on a TypeORM-baseline database.
- Data migration must handle NULL device scopes and legacy plan types.
- API shape change for `/player-plans` endpoints (returns balance, not purchase row).
- Frontend must adapt to the new response shape.

### Risks

- **Risk**: Backfill produces incorrect merged balances.
  **Mitigation**: Only active, non-expired plans are merged; expired/cancelled
  stay in legacy. Migration ledger rows provide a verifiable opening balance.
- **Risk**: `usage_sessions."playerPlanId"` becomes stale for new sessions.
  **Mitigation**: Column kept nullable; new sessions use `"balanceId"` exclusively.

## Alternatives considered

### A. Extend player_plans with recharge columns in-place

- Pros: No new tables; smaller migration.
- Cons: Per-purchase rows remain; no natural place for ledger; scattered session
  history. Does not clean up the six unused plan types.
- **Why rejected**: Carries forward the structural problem.

### B. Add a separate recharge_history table alongside player_plans

- Pros: Additive only; existing code untouched.
- Cons: Two sources of truth for remaining minutes; complex reconciliation.
- **Why rejected**: Dual-state is error-prone.

## References

- ADR-0009: Rust Axum backend (layered architecture)
- `apps/backend/src/models/player_plan.rs` — current model
- `apps/backend/src/repositories/player_plan_repo.rs` — current repo
- `apps/backend/src/services/player_plan_service.rs` — current service
- `apps/backend/migrations/README.md` — TypeORM baseline context
