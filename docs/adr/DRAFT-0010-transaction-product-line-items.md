# DRAFT-0010: Transaction product line items

**Status**: Proposed
**Date**: 2026-05-27
**Deciders**: Platform team

## Context

Product purchases in the admin panel currently record a **total amount** on the `transactions` row via `CreateTransactionDto`. The legacy NestJS UI expected nested `transactionProducts[]` (per-SKU quantity and price) for list/detail views and inventory deduction.

The Rust backend has no `transaction_products` repository or migration. Before adding schema, we must confirm whether a legacy table exists in Postgres and how stock should be updated.

## Decision

**Pending DB spike.** Do not implement line-item persistence until:

1. The DB spike checklist in [`docs/spikes/transaction-products-db-spike.md`](../spikes/transaction-products-db-spike.md) is completed.
2. This ADR is reviewed and moved to **Accepted**.

**Proposed direction (subject to spike):**

- Add a normalized `transaction_products` table: `transaction_id`, `product_id`, `quantity`, `unit_price`, timestamps.
- On `product_purchase` create: insert parent transaction + line rows in one DB transaction; decrement `products.stockQuantity` per line.
- Expose line items on `GET /transactions/{id}` and paginated list via SQL join (or embedded JSON only if spike shows low volume and no legacy table).

## Consequences

### Positive

- Accurate product sales reporting and stock tracking.
- Admin list/detail can show which items were sold.
- Refunds can target specific SKUs later.

### Negative

- Requires first sqlx migration in `apps/backend/migrations/`.
- Migration path from legacy NestJS data if table shape differs.
- Create-transaction handler becomes more complex (atomic multi-table write).

### Risks

- Double stock decrement if create path runs twice → mitigate with idempotency or single service transaction.
- Legacy data orphan rows → spike must document backfill strategy.

## Alternatives Considered

### A. Total amount only (current)

- Pros: No migration, already working.
- Cons: No per-product history, no stock sync.

### B. JSONB `lineItems` column on `transactions`

- Pros: Single-table writes, flexible schema.
- Cons: Harder to query/report; weak FK integrity.

### C. Normalized `transaction_products` table (preferred if legacy table exists)

- Pros: Relational integrity, familiar to TypeORM schema.
- Cons: Migration + repo layer required.

## Implementation Notes

- Blocked until ADR accepted.
- Regenerate OpenAPI and `pnpm gen:api-types` after API changes.
- Update [`ProductTransactionsPage.tsx`](../../apps/admin/src/pages/dashboard/product-transactions/ProductTransactionsPage.tsx) to render line items when present.

## References

- [`docs/spikes/transaction-products-db-spike.md`](../spikes/transaction-products-db-spike.md)
- [`apps/backend/src/models/transaction.rs`](../../apps/backend/src/models/transaction.rs)
- ADR discipline: [`.cursor/rules/20-adr-discipline.mdc`](../../.cursor/rules/20-adr-discipline.mdc)
