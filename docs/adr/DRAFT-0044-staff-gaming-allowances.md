# ADR-0044: Staff gaming allowances (30-day)

**Status**: Proposed
**Date**: 2026-06-21
**Deciders**: Platform team

**Amends**: [ADR-0017](0017-kiosk-player-device-auth.md) (kiosk session principal includes `staff`)
**Implements**: Staff comp gaming — admin-granted 30-day hour allowances per staff member

## Context

Staff members need off-shift access to kiosk gaming as an employment benefit. Admins must configure per-staff gaming hours on a fixed 30-day period. Staff use their existing credentials on the normal player login screen. Staff on an active work shift must not play on kiosk machines.

The platform already has:

- `player_plan_balances` wallet with ledger audit and session linkage
- Kiosk dual JWT auth ([ADR-0017](0017-kiosk-player-device-auth.md))
- Work shifts in `shifts` with `status = 'active'` while clocked in

Today `POST /auth/login/player` accepts only `role = 'player'`. There is no admin API or balance kind for staff comp time.

## Decision

1. **Wallet reuse** — Add `plan_kind` value `staff_allowance`. Admin grants create rows in `player_plan_balances` with NULL device scope (valid on any kiosk), 30-day expiry, and per-staff allotted minutes.

2. **Kiosk auth extension** — `POST /auth/login/player` accepts `role = 'staff'` with the same password flow as players (no TOTP on kiosk). Kiosk JWT carries `roles: ["staff"]`. `PlayerUser` middleware accepts `player` or `staff` roles.

3. **Shift gate** — Before staff kiosk login, reject when `ShiftService::get_active(user_id)` returns an active shift (`403 STAFF_SHIFT_ACTIVE`).

4. **Admin API** — Admin-only:
   - `GET /users/{id}/staff-gaming-allowance` — current period summary
   - `PATCH /users/{id}/staff-gaming-allowance` — grant/renew (`{ allottedHours }`)

5. **Ledger reasons** — `staff_allowance_grant`, `staff_allowance_renewal` for audit; unused hours expire at period end (no rollover).

## Consequences

### Positive

- Reuses session countdown, ledger, and auto-end without parallel time tracking
- Single staff identity (no linked player accounts)
- Shift gate prevents on-duty gaming

### Negative

- `player_plan_balances.playerId` FK name is misleading for staff rows (semantic only)
- Staff kiosk JWT uses `roles: ["staff"]` while route extractors remain named `PlayerUser`

### Risks

| Risk | Mitigation |
|------|------------|
| Staff clocks in while gaming session open | v1: enforce shift gate at login/resume only |
| NULL device scope breaks existing balance queries | Explicit `staff_allowance` kind handling in `BalanceService` |

## Alternatives Considered

### Dedicated `staff_gaming_allowances` table

- Pros: Clear domain separation
- Cons: Duplicates session billing and kiosk clock logic
- **Rejected** — wallet model is sufficient

### Linked player account per staff member

- Pros: No auth changes
- Cons: Dual identity, poor reporting
- **Rejected** — user requires same staff credentials on kiosk

## References

- [ADR-0017](0017-kiosk-player-device-auth.md) — kiosk dual JWT
- [DRAFT-0014](DRAFT-0014-player-plan-balances-wallet.md) — wallet model
