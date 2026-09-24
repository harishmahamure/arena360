# M04 — Commercial operations

## Outcome

One location can manage its commercial day in the new console with explicit
money, entitlement, stock, and approval models.

## In scope

- New UX for POS, products, kiosk orders, shifts, registers, deposits, expenses,
  credit, inventory, procurement, and operational reporting.
- First-class membership lifecycle and separated cash, promotional,
  membership-credit, and loyalty balances.
- Transaction-safe wallet ledger, recharge, expiry, adjustment, and refund
  planning.
- Pricing quote/explanation and approved rule-set workflow.
- Promotions and loyalty earn/redeem foundation.
- Payment orchestration contract, including pending, succeeded, failed,
  duplicated, and refunded states; gateway selection can remain a separate
  delivery decision.

## UX validation

- Quick sale, split tender, membership sale, wallet recharge, refund, kiosk
  order acceptance, stock exception, and shift reconciliation.

## Non-goals

- No recipes/KOT unless separately accepted as a thin slice.
- No Arena360 Player implementation or cross-location Owner reporting.

## Exit gate

Financial ledgers reconcile, duplicate requests/callbacks are idempotent,
approval/audit rules are proven, and a location can close a business day with
explained exceptions.
