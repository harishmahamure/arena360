# ADR-0015: Player credit buys (tab / khata)

**Status**: Proposed
**Date**: 2026-05-30
**Deciders**: Platform team

## Context

Players currently pay for plans and products via cash, online, or split payment at
point of sale. There is no per-player credit limit, no "buy now pay later" flow,
and no staff workflow to view outstanding balances or settle bills partially or
across multiple transactions.

Business requires:

- Per-player configurable credit limit (0 = credit disabled).
- Plans and products purchasable on credit with immediate benefit (minutes / stock).
- Staff visibility of members with outstanding credit.
- Settlement of one or many transactions, with partial payment per transaction.
- Settlement payments recorded into the current shift cash register.

## Decision

Add a **player credit / tab** system:

1. **`users."creditLimit"`** — per-player limit (`DECIMAL`, default 0).
2. **New enum values** on existing transaction enums:
   - `paymentMethod = 'credit'`
   - `paymentStatus = 'credit'` (outstanding; flips to `completed` when fully paid).
3. **`transactions."paidAmount"`** — tracks partial payments; remaining =
   `amount - paidAmount`.
4. **`credit_settlements`** + **`credit_settlement_items`** — audit each settlement
   and per-transaction amounts applied.
5. **Eligibility gate** before any credit purchase: active player role, credit
   enabled, `outstanding + amount <= creditLimit`.
6. **HTTP surface** (AdminOrStaff unless noted):
   - `GET /credit/accounts` — players with outstanding credit.
   - `GET /credit/players/{id}` — summary + outstanding transactions.
   - `POST /credit/settlements` — settle selected/partial lines.
   - `PATCH /users/{id}/credit-limit` (AdminUser) — set limit.

Settlement posts one `cash_in` entry to the staff member's active shift register
for the cash portion (same pattern as completed transactions).

## Consequences

### Positive

- Reuses existing `transactions` table for credit purchases (no parallel purchase model).
- Partial settlement per transaction with full audit trail.
- Credit limit enforced server-side before any benefit is granted.

### Negative

- New migration on TypeORM-baseline DB; Postgres enum values are not removable.
- `TransactionService` gains dependency on `CreditService`.
- Admin UI must pre-validate eligibility and surface distinct error states.

### Risks

- **Risk**: Partial payments desync from limit checks.
  **Mitigation**: Outstanding derived from `SUM(amount - paidAmount)` on credit-status rows.
- **Risk**: Settlement without active shift.
  **Mitigation**: Handler requires active shift (same as transaction create).

## Alternatives considered

### A. Reuse `paymentStatus = 'pending'` with no settlement tables

- Pros: No migration beyond a limit column.
- Cons: No partial payment tracking, no settlement audit, ambiguous status semantics.
- **Why rejected**: Cannot support partial settlement or clearance history.

### B. Separate `player_credit_accounts` wallet table

- Pros: Dedicated balance column.
- Cons: Dual source of truth vs transactions; harder reconciliation.
- **Why rejected**: Transaction rows already represent purchases; `paidAmount` is sufficient.

## References

- ADR-0009: Rust Axum backend
- ADR-0011: POS shifts and cash registers
- `apps/backend/src/services/transaction_service.rs`
- `apps/backend/src/services/cash_register_service.rs`
