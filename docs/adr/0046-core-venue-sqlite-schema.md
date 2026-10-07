# ADR-0046: Core venue SQLite schema

**Status**: Accepted (2026-10-07, owner)
**Date**: 2026-10-07
**Deciders**: Founder / backend owner
**Extends**: ADR-0043, ADR-0044, and ADR-0045

## Context

M4 `DB-0010b` must add the tenant-local schema needed for a venue to run a normal day: catalog
and pricing, devices, players and staff projections, plan wallets and sessions, checkout and
credit, plus the inventory stock and recipe data consumed by checkout.

The transitional PostgreSQL schema accumulated these capabilities across many migrations and
contains shared-tenant keys, PostgreSQL enums, triggers, row locks, authentication secrets,
compatibility columns, and tables that belong to later milestones. Copying it directly would
violate physical tenant isolation and preserve accidental complexity.

This migration establishes the contract that M4 repositories will target. It includes the
minimal shift identity required by sessions and payments; M5 still owns shift/cash workflows and
the back-office tables for procurement, inventory adjustments and transfers, expenses,
notifications, broader configuration, and kitchen operations.

## Decision

1. Add `apps/backend/migrations/tenant/0003_core_venue.sql`. It is an additive, transactional
   SQLx migration and all created tables are SQLite `STRICT` tables.
2. Use snake_case names and the ADR-0044 representations throughout:
   - canonical UUID v7 `TEXT` IDs generated in Rust;
   - fixed-width UTC timestamp `TEXT`;
   - money and decimal quantities as scale-4 `INTEGER`;
   - booleans as constrained `INTEGER`;
   - JSON as validated `TEXT`;
   - no `tenant_id`, `organizationId`, or equivalent tenant discriminator.
3. Create the following core projections and catalog tables:
   - `venue_locations`;
   - `users`, containing tenant-local players and the operational projection of global staff;
   - `access_assignments` and `location_role_assignments`, referencing ADR-0045 `access_roles`;
   - `devices`;
   - `plans`, `products`, and `games`;
   - `product_locations` and `plan_locations`, carrying selected-location availability and an
     optional location price;
   - `pricing_rule_sets` and immutable `pricing_rule_versions`;
   - `pricing_rule_set_locations`;
   - `product_recipe_items`, `product_option_groups`, `product_options`, and
     `product_option_ingredients`.
4. Tenant `users` contains two explicitly constrained shapes:
   - tenant-local players may store an email, username, and bcrypt password hash because player
     registration and login are cell-local business operations;
   - projected global staff use their control-plane user ID and store no password hash.
   Neither shape stores OTP values, TOTP secrets, refresh tokens, or authentication challenges.
   A member revision plus `access_assignments` and `location_role_assignments` makes staff
   projection updates idempotent and preserves location-scoped authorization.
5. Create the wallet and session tables:
   - `player_plans`, preserving the purchased-plan snapshot and lifecycle;
   - `player_plan_balances`, the current consumable balance;
   - `player_plan_ledger`, an append-only minute/usage ledger;
   - `usage_sessions`, including the balance, device, source-plan snapshot, deduction profile,
     start/end instants, consumed credits, and end reason.
6. Wallet and session constraints enforce non-negative remaining balances, positive purchased
   amounts, valid expiry ordering, and at most one open session per device. Repository write
   paths use `BEGIN IMMEDIATE`; no `SELECT ... FOR UPDATE` compatibility abstraction is added.
   `usage_sessions` also stores `player_id` so partial unique indexes enforce one open session per
   device, balance, and player. Active-wallet uniqueness uses expression indexes with
   `COALESCE` so nullable device scopes cannot create duplicate active wallets. Local wallet
   windows use `HH:MM:SS` text and permit midnight wrap.
7. Create checkout, payment, and credit tables:
   - a minimal `shifts` root needed by sessions, transactions, credit settlement, and kiosk
     system operation; M5 adds cash-management behavior around it;
   - `transactions`;
   - `transaction_products` and `transaction_product_options`, with immutable product, option,
     quantity, and price snapshots;
   - `credit_settlements` and `credit_settlement_items`;
   - `kiosk_orders` and `kiosk_order_items`, with immutable item snapshots.
8. Payment totals use scale-4 integers. Database checks require non-negative components,
   `paid_amount <= amount`, and `cash_amount + online_amount <= paid_amount`. A completed
   non-credit transaction requires `paid_amount = amount` and
   `cash_amount + online_amount = amount`; credit transactions may be partially settled over
   time. Business-level transition rules remain in Rust services.
9. Create only the inventory tables required in the checkout transaction:
   - `inventory_locations`;
   - `location_stock`, keyed by `(inventory_location_id, product_id)`;
   - append-only `stock_movements`.
   Checkout deducts finished products and recipe ingredients from `location_stock` and appends
   movements in the same `BEGIN IMMEDIATE` transaction. Stock cannot become negative.
   Stock quantities, recipe quantities, and units-per-purchase-unit are whole integers; only
   genuinely decimal quantities use ADR-0044 scale-4 storage. `location_stock` is the stock source
   of truth; `products` does not keep a denormalized stock quantity.
10. Inventory adjustments, receipts, transfers, waste, vendors, purchase orders, reorder rules,
    and archival workflow state remain in `DB-0010c`/M5.
11. Units and setting overrides continue to use the tables introduced by ADR-0045. Add
    `setting_revisions` now because M4 price overrides require optimistic revision checks and
    audit history. Setting JSON preserves the current API representation (including decimal JSON
    numbers); Rust validates and converts values at operational boundaries. The migration does
    not reseed or replace tenant customizations.
12. Use text status/type columns with explicit `CHECK` constraints instead of SQLite enum lookup
    tables. Values follow the current public API vocabulary so M4 does not change HTTP contracts.
13. Foreign keys use restrictive deletion by default. Child snapshot rows that must survive
    catalog soft deletion retain their immutable display and price fields. Soft-deletable roots
    have `deleted_at`; uniqueness for user-visible names, usernames, SKUs, and serial identifiers
    applies only to live rows.
    Aggregate-document children (transaction lines/options, kiosk items, settlement items, recipe
    groups/options) cascade only when their parent root is physically deleted. Catalog references
    from immutable snapshots are restrictive or `SET NULL`.
14. Add indexes for the current repository access paths:
    - live-name and live-identifier lookups;
    - location-scoped catalog and device lists;
    - active wallets by player and expiry;
    - open sessions by player, device, and balance;
    - transactions by player, location, payment status, and transaction time;
    - unsettled credit by player;
    - kiosk order status and creation time;
    - stock by location/product and movement history.
15. Pricing versions, deduction profiles, permissions, fingerprints, and other structured
    snapshots are validated JSON text. Frequently filtered identifiers and statuses remain typed
    columns rather than being hidden in JSON.
    Product, plan, and pricing-set availability is relational rather than a JSON UUID array:
    each root has `availability_scope` constrained to `ALL` or `SELECTED`, and `SELECTED` rows
    reference `venue_locations` through their location tables. Empty location rows are valid only
    for `ALL`. Location prices live on the same relationship row. The legacy singular
    `pricing_rule_sets.location_id` is removed.
16. Product prices have one canonical day price, night price, and purchase-price-per-box value;
    legacy `price` and `purchase_price` mirrors are derived in Rust and are not stored. Likewise,
    actor IDs remain canonical UUID text without a foreign key so control-plane projection lag
    cannot block a valid venue write. Plan and deduction local clock windows use constrained
    `HH:MM[:SS]` text and are not UTC instants.
    `venue_locations` does not duplicate tenant time zone or currency; those remain in the
    control-plane entitlement and effective `pricing.currency` setting. Device status/type values
    preserve the current public API vocabulary until a separate contract change removes legacy
    console labels.
17. `player_plans` retains remaining-count and remaining-minute snapshot fields for current API
    compatibility, while `player_plan_balances` is authoritative for session consumption.
    `usage_sessions.end_reason` is stored with the current
    `voluntary | auto | force | offline_reconcile` vocabulary. Transaction and kiosk lines copy
    product name, SKU, option names, quantity, and unit prices so receipts survive catalog edits.
18. Rust repositories explicitly maintain `updated_at`, enforce state transitions, and call the
    tenant outbox helper in the same transaction. No trigger writes outbox events. Plain triggers
    are not introduced in this migration.
19. Migration contract tests inspect every table, foreign key, index, and check-sensitive field;
    reject tenant discriminator columns and credential fields; exercise money/timestamp/JSON
    constraints; and verify representative session, checkout, credit, and recipe relationships.

## Consequences

### Positive

- M4 repositories get one coherent SQLite contract rather than a mechanical PostgreSQL port.
- Authentication secrets remain outside tenant files and analytical events.
- Checkout, stock deduction, wallet consumption, and outbox writes can be one local transaction.
- Strict types and database constraints catch representation errors before analytics ingestion.
- M5 can add back-office workflows without changing M4's core keys.

### Negative

- The migration is intentionally broad and creates many related tables at once.
- Repository SQL must be rewritten for snake_case, integer money, JSON text, and SQLite locking.
- The staff and location projections duplicate selected control-plane data and require explicit
  synchronization in later API tasks.
- Text status checks require migrations when the public vocabulary expands.

## Alternatives

### Port the PostgreSQL schema verbatim

This would reduce initial mapping work, but retain tenant discriminator columns, authentication
secrets, PostgreSQL-only types and locks, compatibility fields, and trigger behavior that conflict
with accepted ADRs.

### Create tables one repository at a time

Smaller migrations are easier to review, but M4's session, checkout, wallet, credit, recipe, and
stock foreign keys form one operational graph. A single reviewed foundation avoids temporary
nullable links and repeated table rebuilds in SQLite.

### Store catalog and transaction snapshots as JSON documents

This reduces DDL, but weakens referential integrity and makes operational filtering, uniqueness,
stock updates, and later DuckDB rebuilds harder to verify.

### Include all M5 back-office tables now

This would reduce future migrations, but expands the critical path and forces premature decisions
for procurement, cash reconciliation, notifications, and kitchen workflows that M4 does not need.

## Risks

- The broad schema misses a repository query or preserves an obsolete compatibility field.
  - Mitigation: map every M4 repository query before finalizing DDL and add one contract test per
    repository table group.
- SQLite table rebuilds become expensive if status or nullability choices are wrong.
  - Mitigation: preserve the current API vocabulary, keep snapshots immutable, and use nullable
    links only where the existing domain permits absence.
- Stock or wallet values drift under concurrent writes.
  - Mitigation: one lease-gated writer, `BEGIN IMMEDIATE`, non-negative checks, append-only
    movement/ledger rows, and transaction-level tests.
- Control-plane staff or location projections become stale.
  - Mitigation: projection revisions are explicit; M4 API ports reject older revisions and later
    synchronization writes are idempotent.

## References

- `docs/adr/0043-storage-cells-sqlite-duckdb.md`
- `docs/adr/0044-tenant-sqlite-baseline.md`
- `docs/adr/0045-tenant-provisioning-defaults.md`
- `docs/plans/data-platform-build-plan.md` (`DB-0010b`)
- `docs/architecture/duckdb-analytics-schema.md`
- `apps/backend/src/repositories/`
