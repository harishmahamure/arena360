# ADR-0054: Online payment UPI reference last-4

**Status**: Proposed
**Date**: 2026-07-16
**Deciders**: Platform team

## Context

Staff record online (UPI) receipts at the counter by selecting payment method
`online` or `split_payment`. Today the platform stores only method and amounts
— there is no structured field for the UPI/payment-app Transaction ID shown on
PhonePe, GPay, or bank receipts. Reconciliation against bank statements is
manual and error-prone.

Staff need a lightweight mandatory capture: the **last 4 digits** of that
external reference whenever any online amount is involved (pure online or
split with an online portion). Full UTR storage and payment-gateway integration
remain out of scope (see `docs/REQUIREMENTS.md`).

## Decision

1. Add nullable column `"onlinePaymentRefLast4" CHAR(4)` to:
   - `transactions`
   - `credit_settlements`
2. Require the field in application validation when:
   - `paymentMethod = 'online'`, or
   - `paymentMethod = 'split_payment'` and `onlineAmount > 0`
3. Value must be exactly four ASCII digits (`^\d{4}$`).
4. Expose as `onlinePaymentRefLast4` on create DTOs and read responses
   (OpenAPI / `@gaming-cafe/api-types`).
5. Admin POS (product sale, plan sale, credit settlement) shows a mandatory
   input when an online portion is selected.

Existing rows remain `NULL` (no backfill). Cash-only and credit purchases do
not require the field.

## Consequences

### Positive

- Structured, searchable last-4 for counter reconciliation
- Server-enforced; cannot bypass via API
- Minimal PII / receipt data (last 4 only)

### Negative

- New DB migration and OpenAPI surface change
- Historical online sales lack the field

### Risks

- Staff may mistype digits → mitigate with clear label and 4-digit-only input
- Collisions across days are expected; last-4 is a hint, not a unique key

## Alternatives Considered

### Free-text in `notes` (client-only)

- Pros: No migration
- Cons: No enforcement, unstructured, weak reconciliation
- Why rejected: Does not meet mandatory, auditable capture

### Full UPI/UTR string

- Pros: Stronger matching
- Cons: Longer input, more storage, not requested by ops
- Why rejected: Confirmed scope is last 4 only

## Implementation Notes

- Migration: `apps/backend/migrations/*_online_payment_ref_last4.sql`
- Validation helper: `validate_online_payment_ref_last4` in `validation.rs`
- Wire into transaction create and credit settle services after payment
  amounts are populated

## References

- `docs/REQUIREMENTS.md` (cash/UPI receipts; PG integration future)
- Plan: Mandatory UPI reference last-4 for online payments
