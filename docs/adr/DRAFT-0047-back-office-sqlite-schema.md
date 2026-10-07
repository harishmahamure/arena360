# ADR-0047: Back-office SQLite schema

**Status**: Proposed
**Date**: 2026-10-07
**Deciders**: Founder / backend owner
**Extends**: ADR-0043, ADR-0044, ADR-0045, and ADR-0046

## Context

M5 `DB-0010c` must add the tenant-local schema needed to move the remaining business APIs off
PostgreSQL: inventory and procurement workflows, shifts and cash reconciliation, expenses,
settings-related access state, notifications, and kitchen operations.

ADR-0046 intentionally created only the stock, location, and minimal shift roots needed by M4.
It deferred receipts, transfers, waste, adjustments, purchase orders, vendors, cash registers,
deposits, expenses, notifications, kitchen tickets, and access audit state until their repository
contracts could be mapped. Copying the PostgreSQL DDL would retain organization discriminators,
enums, sequences, advisory-lock assumptions, triggers, compatibility columns, and configuration
tables that conflict with the accepted Storage Cell architecture.

The migration must preserve M4 keys and existing tenant data. In particular, `location_stock`
remains authoritative, `stock_movements` remains append-only, `setting_overrides` is already live,
and `shifts` is already referenced by sessions, transactions, and credit settlements.

## Decision

1. Add one additive, transactional SQLx migration:
   `apps/backend/migrations/tenant/0004_back_office.sql`. All new tables are SQLite `STRICT`
   tables. The migration contains no triggers and no tenant discriminator.
2. Apply ADR-0044 representations throughout:
   - UUID v7 IDs are generated in Rust and stored as canonical lowercase UUID `TEXT`;
   - instants use fixed-width UTC timestamp `TEXT`;
   - money and decimal rates use scale-4 `INTEGER`;
   - booleans use constrained `INTEGER`;
   - structured values use validated JSON `TEXT`;
   - calendar dates and local purchase-order years are labels, not UTC instants.
3. Add the procurement and inventory workflow graph:
   - `vendors`;
   - `purchase_orders`, `purchase_order_lines`, and `purchase_order_counters`;
   - `stock_receipts` and `stock_receipt_lines`;
   - `stock_transfer_requests` and `stock_transfer_lines`;
   - `stock_waste_events` and `stock_waste_lines`;
   - `stock_adjustments` and `stock_adjustment_lines`;
   - `inventory_reorder_rules`.
4. Keep `location_stock` as the stock source of truth. Receipt, transfer, waste, adjustment, and
   sale operations update it and append `stock_movements` in the same `BEGIN IMMEDIATE`
   transaction. Stock cannot become negative. Whole boxes and pieces remain integers.
5. Purchase-order numbers use a per-year counter updated inside the tenant writer transaction.
   Rust derives the year from the tenant's control-plane IANA time zone. Receipt invoice
   references and purchase-order/product lines have uniqueness constraints that make retries
   detectable without `MAX()`-based numbering.
6. Add cash and expense tables:
   - `cash_registers` and `cash_register_entries`;
   - `cash_deposits`;
   - `expense_categories`;
   - `expenses`, including optional `source_type` and `source_id` for idempotent system expenses.
   One open register is permitted per shift. Variance may be negative; other money components are
   non-negative. Cash entries use positive amounts with an explicit `cash_in` or `cash_out` type.
7. Preserve the existing `shifts.status IN ('active', 'closed')` contract rather than rebuilding a
   referenced table. Add nullable `close_kind IN ('normal', 'force')`; repositories map
   `closed + close_kind` to the public completed and force-closed states. Kiosk-system shifts are
   not required to own a cash register.
8. Add kitchen, notification, activity, and access-operation tables:
   - `kitchen_menu_settings`, `kitchen_tickets`, and `kitchen_ticket_events`;
   - `activity_log` and `user_notifications`;
   - `access_modules` and append-only `access_audit`.
   `kitchen_tickets.transaction_id` is unique so checkout retry cannot enqueue twice. Module and
   menu updates use explicit revisions.
9. Do not add a `configurations` table. Tenant and location configuration continues to use the
   ADR-0045 `setting_overrides` table and the existing `setting_revisions` history. PostgreSQL
   advisory locks become tenant `BEGIN IMMEDIATE` serialization in API-0031.
10. Use checked text vocabularies matching current public contracts:
    - purchase orders: `draft`, `submitted`, `approved`, `rejected`, `ordered`,
      `partially_received`, `received`, `cancelled`;
    - transfers: `pending`, `approved`, `rejected`, `fulfilled`;
    - waste: `pending`, `approved`, `rejected`, with reasons `expired`, `damaged`, `spoilage`,
      `sample`, `other`;
    - registers: `open`, `closed`, `reconciled`;
    - deposits and expense approval: `pending`, `approved`, `rejected`;
    - kitchen tickets: `queued`, `preparing`, `ready`, `served`, `cancelled`.
    Business transition matrices remain in Rust.
11. Foreign keys are restrictive by default. Aggregate children cascade only with their document
    root. Actor IDs remain nullable canonical UUID text without a foreign key so control-plane
    staff-projection lag cannot block a venue write. `expenses` uses soft deletion; current
    hard-delete behavior remains for vendors and expense categories.
12. Add indexes and uniqueness constraints for existing repository access paths: vendor and
    category lookup, purchase-order status/vendor/destination, receipt invoice idempotency,
    transfer/waste/adjustment queues, reorder rules, one open register per shift, register entries,
    deposits, expense filters and live sources, unread notifications, activity history, access
    audit, and kitchen queues.
13. Later API ports use one fenced tenant writer transaction for each aggregate transition:
    - API-0029 receipt processing updates the purchase order, stock, movements, system expense,
      and optional cash entry atomically;
    - API-0030 shift/register start, close, reconcile, deposit, and cash expense transitions are
      atomic, including activity rows;
    - API-0031 kitchen enqueue joins checkout atomically, while settings, kitchen state, and access
      modules use revision compare-and-set.
14. Handover TOTP stays in the control plane and is verified before a tenant transaction. No TOTP,
    password hash, membership, entitlement, lease, billing, realtime-delivery, or analytics
    transport table is added to a tenant file.
15. Seed the stable “Inventory purchases” expense category from Rust with `INSERT OR IGNORE`.
    Provisioning and migration hooks may ensure it exists, but neither the SQL migration nor retry
    logic overwrites a tenant customization.
16. Contract tests inspect every new table, foreign key, index, and check-sensitive field; reject
    credential and tenant-discriminator columns; exercise invalid money, timestamp, JSON, and
    status values; and cover representative receipt, transfer, waste, shift/register, deposit,
    expense, notification, and kitchen relationships.
17. The migration is forward-only and runs after `0003_core_venue.sql`. A failure rolls back the
    migration and leaves the tenant schema version unchanged. Recovery uses a forward fix or a
    pre-migration SQLite snapshot; existing M4 rows are not rewritten or reseeded.

## Consequences

### Positive

- API-0029, API-0030, and API-0031 target one coherent tenant-local contract.
- Inventory, cash, expense, and kitchen transitions can commit with their outbox events atomically.
- Existing M4 foreign keys and setting customizations remain intact.
- Database constraints provide concrete idempotency and non-negative-stock boundaries.

### Negative

- The migration is broad and creates several related workflow aggregates at once.
- Repository SQL must be rewritten for snake_case, scale-4 integers, JSON text, and SQLite locking.
- Text status checks require a later migration when public workflow vocabularies change.
- TOTP verification and the resulting tenant write cannot share one cross-database transaction.

## Alternatives

### Port the PostgreSQL DDL verbatim

This would preserve organization IDs, PostgreSQL enums and sequences, triggers, advisory locks,
and obsolete configuration tables, conflicting with ADR-0043 and ADR-0044.

### Split one migration per repository

Smaller files are easier to review, but receipt, stock, expense, register, and kitchen foreign keys
form a single operational graph. Intermediate schemas would require nullable compatibility links
or repeated SQLite table rebuilds.

### Rebuild `shifts` to widen its status check

This preserves PostgreSQL status words directly, but rebuilding a parent already referenced by
sessions, transactions, and settlements adds avoidable migration risk. `close_kind` preserves the
same public distinction without a rebuild.

### Store workflow documents as JSON

This reduces DDL but weakens foreign keys, queue indexes, line uniqueness, stock checks, and
idempotent receipt and kitchen processing.

### Defer access modules and audit

This narrows the migration but leaves part of API-0031 on PostgreSQL after settings and kitchen
move, preventing the M5 operational cutover.

## Risks

- A receipt retry could double-increment stock or duplicate an expense.
  - Mitigation: unique invoice/source constraints and one fenced transaction for all effects.
- Purchase-order numbers could collide under concurrency.
  - Mitigation: update `purchase_order_counters` only inside `BEGIN IMMEDIATE`.
- Shift close code could write a PostgreSQL-only status and fail the SQLite check.
  - Mitigation: repositories store `closed` plus `close_kind` and test public-state mapping.
- Cash, stock, or kitchen state could commit while its activity/outbox row is lost.
  - Mitigation: all business and audit rows are written before the same local commit.
- A schema constraint could require rebuilding an M4 table with live references.
  - Mitigation: keep `shifts` and `setting_overrides` in place and make only additive changes.
- A control-plane handover verification can succeed while the tenant write fails.
  - Mitigation: make the tenant half idempotent and safe to retry; never copy TOTP secrets.

## References

- `docs/adr/0043-storage-cells-sqlite-duckdb.md`
- `docs/adr/0044-tenant-sqlite-baseline.md`
- `docs/adr/0045-tenant-provisioning-defaults.md`
- `docs/adr/0046-core-venue-sqlite-schema.md`
- `docs/plans/data-platform-build-plan.md` (`DB-0010c`, API-0029, API-0030, API-0031)
- `docs/architecture/data-platform.md`
- `docs/architecture/duckdb-analytics-schema.md`
- `apps/backend/src/repositories/`
- `apps/backend/src/services/`
