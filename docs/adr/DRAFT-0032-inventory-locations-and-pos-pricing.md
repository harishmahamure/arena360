# ADR-0032: Inventory locations, warehouse/store stock, and POS day/night pricing

**Status**: Proposed
**Date**: 2026-06-06
**Deciders**: Platform team

**Relates to**: [ADR-0010](ADR-0010-transaction-product-line-items.md), [DRAFT-0011](DRAFT-0011-pos-shifts-registers-expenses.md)

## Context

POS today tracks a single global `products.stockQuantity` and one `price` per SKU. The cafe needs:

- One **warehouse** + one **store** (configurable locations)
- Receive stock in **boxes**, sell in **pieces**
- Store **stock requests** fulfilled by admin from warehouse
- **Waste** recording with admin approval
- **Day/night** retail pricing (23:00–08:00 venue local time)

This requires new tables, `/inventory/*` HTTP surface, and changes to product purchase stock deduction (extends ADR-0010).

## Decision

### Schema

- `inventory_locations` (warehouse | store)
- `location_stock` (pieces per location × product)
- `stock_movements` ledger (`receipt`, `transfer_out`, `transfer_in`, `sale`, `waste`, `adjustment`)
- `stock_receipts` + lines (warehouse receive)
- `stock_transfer_requests` + lines (store request → admin fulfill)
- `stock_waste_events` + lines (staff submit → admin approve)
- Extend `products`: `unitId`, `purchaseUnitId`, `unitsPerPurchaseUnit`, `dayPrice`, `nightPrice`, `purchasePricePerBox`

### API

Prefix `/inventory/*` for locations, stock, receipts, transfers, waste.

Product purchases require `saleLocationId` (store); decrement `location_stock`; price from `effective_product_price()` using `CAFE_TZ`.

### Transition

Backfill store `location_stock` from `products.stockQuantity`. Stop writing `stockQuantity` on sale; sync store total on each movement.

## Consequences

### Positive

- Accurate multi-location inventory, waste audit, time-based pricing.

### Negative

- Large migration + admin UI surface.
- Requires `pnpm gen:api-types` after OpenAPI update.

## References

- Plan: POS Warehouse/Store Inventory Enhancement (2026-06-06)
