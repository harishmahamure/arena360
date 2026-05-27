# ADR-0010: Transaction product line items

**Status**: Accepted
**Date**: 2026-05-28
**Deciders**: Platform team

## Context

Product purchases in the admin panel currently record a **total amount** on the `transactions` row via `CreateTransactionDto`. The legacy NestJS UI expected nested `transactionProducts[]` (per-SKU quantity and price) for list/detail views and inventory deduction.

The Rust backend has no `transaction_products` repository or migration.

## Decision

Add a normalized `transaction_products` table per Alternative C:

- `transaction_products` table: `transaction_id`, `product_id`, `quantity`, `unit_price`, timestamps.
- On `product_purchase` create: insert parent transaction + line rows in one DB transaction; decrement `products.stockQuantity` per line.
- Expose line items on `GET /transactions/{id}` and paginated list via SQL join.
- Server-side total computation from line items; amount on `transactions` row = sum of (quantity x unit_price) across line items.
- Stock validation with `SELECT ... FOR UPDATE` to prevent overselling under concurrency.

## Consequences

### Positive

- Accurate product sales reporting and stock tracking.
- Admin list/detail can show which items were sold.
- Refunds can target specific SKUs later.
- Race-safe stock decrement via row-level locking.

### Negative

- Migration adds a new table with FK constraints.
- Create-transaction handler becomes more complex (atomic multi-table write).

### Risks

- Double stock decrement if create path runs twice → mitigated with single DB transaction + `SELECT ... FOR UPDATE`.
- Legacy data (existing transactions without line items) will show empty `lineItems` arrays on the frontend.

## Alternatives Considered

### A. Total amount only (previous implementation)

- Pros: No migration, already working.
- Cons: No per-product history, no stock sync.

### B. JSONB `lineItems` column on `transactions`

- Pros: Single-table writes, flexible schema.
- Cons: Harder to query/report; weak FK integrity.

### C. Normalized `transaction_products` table (chosen)

- Pros: Relational integrity, familiar pattern.
- Cons: Migration + repo layer required.

## Implementation Notes

- Regenerate OpenAPI and `pnpm gen:api-types` after API changes.
- Update `ProductTransactionsPage.tsx` and `ProductTransactionNewPage.tsx` to render/send line items.

## References

- [`apps/backend/src/models/transaction.rs`](../../apps/backend/src/models/transaction.rs)
- ADR discipline: [`.cursor/rules/20-adr-discipline.mdc`](../../.cursor/rules/20-adr-discipline.mdc)
