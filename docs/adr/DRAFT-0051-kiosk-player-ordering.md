# DRAFT-0051: Kiosk player ordering

**Status**: Proposed
**Date**: 2026-06-26
**Deciders**: Platform team (gaming-cafe kiosk working group)

**Extends**: [ADR-0017](0017-kiosk-player-device-auth.md) (Accepted), [ADR-0010](0010-transaction-products-line-items.md) (Accepted)

## Context

Kiosk v1 originally excluded in-station store ordering — players recharge time at the
counter only ([REQUIREMENTS-KIOSK.md](../REQUIREMENTS-KIOSK.md) §1.5). Operators now
want players to **request snacks and drinks from their gaming station** during an active
session. Staff prepare items manually, collect payment at the counter, and record the
sale in the existing POS.

Product sales today are staff-initiated via `POST /transactions` (`product_purchase`)
with an active shift. There is no order queue or kiosk-facing product menu.

## Decision

Introduce a **kiosk order domain** separate from financial transactions:

### Data model

- `kiosk_orders` — session-bound order with status lifecycle (`pending` → `preparing` → `fulfilled` | `cancelled`)
- `kiosk_order_items` — line items with price/name snapshotted at order time
- Optional `transaction_id` FK when staff converts to sale

Stock is deducted **at sale conversion**, not at order placement.

### Kiosk API (player + device JWT, active session required)

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/kiosk/products` | Scoped product menu (active products, effective price) |
| `POST` | `/kiosk/orders` | Submit order |
| `GET` | `/kiosk/orders/current` | Open order for current session |

One open (`pending`/`preparing`) order per session at a time.

### Staff API (admin/staff JWT; mutations require active shift)

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/kiosk-orders` | Order queue |
| `GET` | `/kiosk-orders/{id}` | Order detail |
| `PATCH` | `/kiosk-orders/{id}` | Update status |
| `POST` | `/kiosk-orders/{id}/convert` | Convert to `product_purchase` transaction |

`POST /transactions` accepts optional `kioskOrderId` to atomically link fulfillment.

### Notifications

- Activity kinds: `kiosk_order_placed`, `kiosk_order_fulfilled`, `kiosk_order_cancelled`
- On place: notify all staff with device name, player username, items
- WebSocket: `kiosk_order.placed` on `staff` channel

### Payment

**No payment on kiosk.** Staff chooses payment method (cash/credit/split) when
converting to sale via admin POS.

## Consequences

### Positive

- Players order without leaving their station
- Reuses existing product catalog, inventory deduction, and POS payment UI
- Clear separation: order = fulfillment intent; transaction = financial record

### Negative

- New HTTP surface and DB tables to maintain
- Staff must monitor order queue / notifications

### Risks

| Risk | Mitigation |
|------|------------|
| Ghost orders locking inventory | Stock deducted only at conversion |
| Missed orders | Persisted notifications + WS toast + order queue page |
| Duplicate orders | One open order per session (409) |

## Alternatives Considered

### Notification-only (no order table)

Rejected: no fulfillment queue, no status tracking, no POS prefill.

### Direct `POST /transactions` from kiosk

Rejected: bypasses staff fulfillment; requires shift on kiosk; mixes payment models.

### In-kiosk payment

Rejected for v1 — operators prefer counter payment; aligns with existing recharge model.

## Implementation Notes

- Kiosk UI: new "Menu" tab in session nav
- Admin UI: `/kiosk-orders` queue + POS prefill via `?orderId=`
- ErrorCodes: `KIOSK_ORDER_NOT_FOUND`, `KIOSK_ORDER_ALREADY_OPEN`, `KIOSK_NO_ACTIVE_SESSION`

## References

- [REQUIREMENTS-KIOSK.md](../REQUIREMENTS-KIOSK.md) — US-KORDER-001 (new)
- [ADR-0017](0017-kiosk-player-device-auth.md)
- [ADR-0010](0010-transaction-products-line-items.md)
- [DRAFT-0045](DRAFT-0045-persisted-notifications-and-activity-log.md)
