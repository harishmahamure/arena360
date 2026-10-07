# Tenant DuckDB Analytics Schema

Implements the analytics side of `docs/architecture/data-platform.md` (§18–19, §32–37, §44) under `docs/adr/0043-storage-cells-sqlite-duckdb.md`. It replaces the ClickHouse schema in `apps/backend/src/analytics/schema.sql`.

## Design rules

1. **One DuckDB file per tenant** (`tenant-<id>/analytics.duckdb`). The file is the tenant boundary, so no table carries `organizationId` and the `ReportScope` SQL rewriting in `analytics/scope.rs` goes away. Location filtering is a plain predicate on `location_id`.
2. **Current state, not version history.** ClickHouse needed `ReplacingMergeTree` plus `FINAL` views to deduplicate. Here, each event batch is applied as upserts and deletes in one DuckDB transaction, so tables always hold the latest row.
3. **Soft-deleted rows are removed.** Every current report filters `deletedAt IS NULL`, so a soft delete is applied as a `DELETE`. Names of removed players fall back to `'Deleted player'` exactly as today.
4. **Exact money.** Money is `DECIMAL(19,4)`, matching the ledger. Reports cast to `DOUBLE` only where today's DTOs already expose floats, and to `VARCHAR` where today's finance report returns decimal strings.
5. **Timestamps are UTC; calendar labels are derived.** Every timestamp column is UTC (ADR-0043 decision 27). `local_date`, `local_hour`, and `weekday` are labels computed in Rust (`chrono-tz`) at ingest from the UTC timestamp and the tenant's IANA time zone, which comes from the global PostgreSQL (decision 28). Changing a tenant's time zone triggers a rebuild.
6. **Session hours precomputed.** The heaviest current query (`HOURS` in `analytics/business.rs`) splits sessions at local hour boundaries at query time. Closed sessions are split once at ingest into `session_hours`; only open sessions are split at query time.
7. **Hot window only.** Fact rows older than the hot-window start (18 months, aligned to the first of a month) are deleted by the nightly retention job. `monthly_summary` keeps long-term trend rows (§44).
8. **Idempotent by sequence.** `_ingest_state.last_sequence` records the last applied outbox sequence. Events at or below it are skipped; a gap stops ingestion for the tenant (ADR-0043 decision 20).

Per-tenant size estimate for a 20-PC venue over 18 months: about 70,000 transactions, 65,000 sessions, 200,000 session-hour rows, and 150,000 transaction lines. That's tens of MB, so plain joins are fast and no further denormalization is needed.

## Event envelope

Outbox events (§15) carry a full post-change snapshot of one aggregate, so the consumer can upsert without per-event business logic.

| Field | Type | Notes |
|---|---|---|
| `sequence` | integer | SQLite `AUTOINCREMENT`; equals commit order |
| `event_id` | UUID v7 | |
| `tenant_id` | UUID | |
| `location_id` | UUID, nullable | Venue location; resolved at write time (for shift-scoped facts, from the shift) |
| `aggregate_type` | text | One of the table names below, e.g. `transaction`, `session` |
| `aggregate_id` | UUID | |
| `event_type` | text | Domain name, e.g. `session.completed`, `payment.completed`, `inventory.adjusted` |
| `occurred_at` | timestamp | UTC |
| `schema_version` | integer | Payload schema version per aggregate type |
| `deleted` | boolean | `true` means delete the row |
| `payload` | JSON | Allowlisted post-change row snapshot; never credentials, OTPs, tokens, or free-form notes |

Within one batch the consumer collapses events to the last state per `(aggregate_type, aggregate_id)` before applying. This keeps each key to one write per transaction and avoids DuckDB index conflicts from repeated updates to the same key in one transaction.

## DDL (schema version 1)

```sql
-- Ingestion and lifecycle state (single row).
CREATE TABLE _ingest_state (
  id                    TINYINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
  schema_version        INTEGER   NOT NULL,
  status                VARCHAR   NOT NULL CHECK (status IN ('READY','LAGGING','REBUILDING','FAILED')),
  last_sequence         UBIGINT   NOT NULL DEFAULT 0,
  rebuild_boundary_seq  UBIGINT,             -- T0 of the last rebuild
  hot_window_start      DATE      NOT NULL,
  timezone              VARCHAR   NOT NULL,  -- IANA zone copied from the control plane; labels were derived with it
  last_event_at         TIMESTAMP,
  updated_at            TIMESTAMP NOT NULL
);

-- People: tenant players plus the tenant's staff membership projection.
CREATE TABLE users (
  id            UUID PRIMARY KEY,
  username      VARCHAR NOT NULL,
  first_name    VARCHAR,
  last_name     VARCHAR,
  role          VARCHAR NOT NULL,            -- 'player' | 'staff' | 'admin'
  is_active     BOOLEAN NOT NULL,
  credit_limit  DECIMAL(19,4) NOT NULL DEFAULT 0,
  created_at    TIMESTAMP
);

CREATE TABLE venue_locations (
  id    UUID PRIMARY KEY,
  name  VARCHAR NOT NULL
);

CREATE TABLE devices (
  id           UUID PRIMARY KEY,
  name         VARCHAR NOT NULL,
  status       VARCHAR NOT NULL,
  area_label   VARCHAR,                      -- today's free-text devices.location
  device_type  VARCHAR,
  location_id  UUID                          -- venue location
);

CREATE TABLE plans (
  id                   UUID PRIMARY KEY,
  name                 VARCHAR NOT NULL,
  price                DECIMAL(19,4),
  time_credit_minutes  BIGINT,
  plan_type            VARCHAR,
  validity_days        BIGINT
);

CREATE TABLE products (
  id                       UUID PRIMARY KEY,
  name                     VARCHAR NOT NULL,
  purchase_price_per_box   DECIMAL(19,4),
  purchase_price           DECIMAL(19,4),
  units_per_purchase_unit  BIGINT
);

CREATE TABLE vendors (
  id    UUID PRIMARY KEY,
  name  VARCHAR NOT NULL
);

CREATE TABLE inventory_locations (
  id                 UUID PRIMARY KEY,
  name               VARCHAR NOT NULL,
  venue_location_id  UUID
);

CREATE TABLE expense_categories (
  id             UUID PRIMARY KEY,
  name           VARCHAR NOT NULL,
  is_active      BOOLEAN NOT NULL,
  budget_amount  DECIMAL(19,4),
  budget_period  VARCHAR
);

-- Sales ledger.
CREATE TABLE transactions (
  id                UUID PRIMARY KEY,
  occurred_at       TIMESTAMP NOT NULL,      -- transactionDate, UTC
  local_date        DATE      NOT NULL,      -- tenant calendar date of occurred_at
  location_id       UUID,
  player_id         UUID,
  plan_id           UUID,
  shift_id          UUID,
  created_by        UUID,
  transaction_type  VARCHAR NOT NULL,        -- 'plan_purchase' | 'product_purchase' | ...
  payment_method    VARCHAR NOT NULL,        -- 'cash' | 'online' | 'split_payment' | 'credit'
  payment_status    VARCHAR NOT NULL,        -- 'completed' | 'credit' | 'pending' | 'refunded'
  amount            DECIMAL(19,4) NOT NULL,
  paid_amount       DECIMAL(19,4) NOT NULL,
  cash_amount       DECIMAL(19,4),
  online_amount     DECIMAL(19,4),
  is_booked         BOOLEAN GENERATED ALWAYS AS (payment_status IN ('completed','credit')) VIRTUAL
);

CREATE TABLE transaction_lines (
  id              UUID PRIMARY KEY,
  transaction_id  UUID NOT NULL,
  product_id      UUID NOT NULL,
  quantity        BIGINT NOT NULL,
  unit_price      DECIMAL(19,4) NOT NULL,
  line_total      DECIMAL(19,4) GENERATED ALWAYS AS (quantity * unit_price) VIRTUAL
);

-- Play time.
CREATE TABLE sessions (
  id                UUID PRIMARY KEY,
  device_id         UUID NOT NULL,
  balance_id        UUID NOT NULL,
  player_id         UUID,                    -- from the wallet at write time
  is_staff_allowance BOOLEAN NOT NULL,       -- wallet kind = 'staff_allowance'
  location_id       UUID,                    -- venue snapshot at start (no reattribution on device move)
  start_time        TIMESTAMP NOT NULL,
  end_time          TIMESTAMP,               -- NULL while running
  start_local_date  DATE NOT NULL,
  duration_minutes  BIGINT,
  shift_id          UUID,
  created_by        UUID,
  source_plan_id    UUID
);

-- Closed sessions split at tenant-local hour boundaries. Replaced whenever the session changes.
CREATE TABLE session_hours (
  session_id        UUID     NOT NULL,
  hour_start        TIMESTAMP NOT NULL,      -- UTC instant of the local hour start
  local_date        DATE     NOT NULL,
  weekday           TINYINT  NOT NULL,       -- ISO 1 = Monday … 7 = Sunday (matches toDayOfWeek(h, 0))
  local_hour        TINYINT  NOT NULL,
  occupied_seconds  INTEGER  NOT NULL,
  is_start_hour     BOOLEAN  NOT NULL,
  device_id         UUID     NOT NULL,
  location_id       UUID,
  PRIMARY KEY (session_id, hour_start)
);

-- Prepaid wallets (current state).
CREATE TABLE wallets (
  id                 UUID PRIMARY KEY,
  player_id          UUID NOT NULL,
  status             VARCHAR NOT NULL,
  kind               VARCHAR,                -- NULL treated as 'time'
  remaining_minutes  BIGINT,
  expiry_date        TIMESTAMP,
  source_plan_id     UUID,
  created_at         TIMESTAMP
);

CREATE TABLE shifts (
  id           UUID PRIMARY KEY,
  user_id      UUID NOT NULL,
  location_id  UUID,
  clock_in     TIMESTAMP NOT NULL,
  clock_out    TIMESTAMP,
  status       VARCHAR NOT NULL
);

-- Credit.
CREATE TABLE credit_settlements (
  id              UUID PRIMARY KEY,
  player_id       UUID NOT NULL,
  shift_id        UUID,
  location_id     UUID,                      -- from the shift at write time
  amount          DECIMAL(19,4) NOT NULL,
  payment_method  VARCHAR NOT NULL,
  cash_amount     DECIMAL(19,4),
  online_amount   DECIMAL(19,4),
  settled_at      TIMESTAMP NOT NULL
);

CREATE TABLE credit_settlement_items (
  id              UUID PRIMARY KEY,
  settlement_id   UUID NOT NULL,
  transaction_id  UUID NOT NULL,
  amount_applied  DECIMAL(19,4) NOT NULL
);

-- Expenses and cash handling.
CREATE TABLE expenses (
  id               UUID PRIMARY KEY,
  category_id      UUID NOT NULL,
  shift_id         UUID,
  location_id      UUID,                     -- from the shift at write time
  amount           DECIMAL(19,4) NOT NULL,
  approval_status  VARCHAR NOT NULL,
  expense_date     TIMESTAMP NOT NULL,
  local_date       DATE NOT NULL
);

CREATE TABLE cash_registers (
  id                UUID PRIMARY KEY,
  shift_id          UUID NOT NULL,
  location_id       UUID,
  status            VARCHAR NOT NULL,        -- 'open' | 'closed' | 'reconciled'
  variance          DECIMAL(19,4),
  closing_balance   DECIMAL(19,4),
  expected_closing  DECIMAL(19,4),
  created_at        TIMESTAMP NOT NULL,
  updated_at        TIMESTAMP NOT NULL
);

CREATE TABLE cash_deposits (
  id            UUID PRIMARY KEY,
  shift_id      UUID,
  location_id   UUID,
  status        VARCHAR NOT NULL,            -- 'pending' | 'approved' | 'rejected'
  amount        DECIMAL(19,4) NOT NULL,
  deposit_type  VARCHAR,                     -- 'bank' | 'home'
  created_at    TIMESTAMP NOT NULL
);

-- Inventory (current state plus movement history).
CREATE TABLE location_stock (
  id               UUID PRIMARY KEY,
  location_id      UUID NOT NULL,            -- inventory location
  product_id       UUID NOT NULL,
  quantity_pieces  BIGINT NOT NULL
);

CREATE TABLE reorder_rules (
  id              UUID PRIMARY KEY,
  location_id     UUID NOT NULL,
  product_id      UUID NOT NULL,
  is_active       BOOLEAN NOT NULL,
  minimum_pieces  BIGINT NOT NULL
);

CREATE TABLE purchase_orders (
  id                       UUID PRIMARY KEY,
  status                   VARCHAR NOT NULL,
  destination_location_id  UUID
);

CREATE TABLE stock_transfer_requests (
  id                UUID PRIMARY KEY,
  status            VARCHAR NOT NULL,
  from_location_id  UUID,
  to_location_id    UUID
);

CREATE TABLE stock_receipts (
  id           UUID PRIMARY KEY,
  vendor_id    UUID,
  location_id  UUID NOT NULL,
  received_at  TIMESTAMP NOT NULL            -- today's stock_receipts.createdAt
);

CREATE TABLE stock_receipt_lines (
  id            UUID PRIMARY KEY,
  receipt_id    UUID NOT NULL,
  product_id    UUID NOT NULL,
  box_quantity  BIGINT NOT NULL,
  pieces_added  BIGINT NOT NULL
);

CREATE TABLE stock_waste_events (
  id           UUID PRIMARY KEY,
  location_id  UUID NOT NULL,
  status       VARCHAR NOT NULL,             -- 'pending' | 'approved' | ...
  approved_at  TIMESTAMP
);

CREATE TABLE stock_waste_lines (
  id               UUID PRIMARY KEY,
  waste_event_id   UUID NOT NULL,
  product_id       UUID NOT NULL,
  reason_code      VARCHAR NOT NULL,
  quantity_pieces  BIGINT NOT NULL
);

CREATE TABLE stock_movements (
  id              UUID PRIMARY KEY,
  location_id     UUID NOT NULL,
  product_id      UUID NOT NULL,
  delta           BIGINT NOT NULL,
  movement_type   VARCHAR NOT NULL,
  reference_id    UUID,
  reference_type  VARCHAR,
  created_by      UUID,
  created_at      TIMESTAMP NOT NULL
);

-- Long-term trend rows kept beyond the hot window (§44).
-- location_id = '00000000-0000-0000-0000-000000000000' holds the all-locations row,
-- because distinct visitor counts cannot be summed across locations.
CREATE TABLE monthly_summary (
  month           DATE NOT NULL,             -- first day of the month, tenant calendar
  location_id     UUID NOT NULL,
  revenue         DECIMAL(19,4) NOT NULL,
  plan_revenue    DECIMAL(19,4) NOT NULL,
  pos_revenue     DECIMAL(19,4) NOT NULL,
  transactions    BIGINT NOT NULL,
  session_starts  BIGINT NOT NULL,
  occupied_hours  DOUBLE NOT NULL,
  visitors        BIGINT NOT NULL,
  PRIMARY KEY (month, location_id)
);
```

Tables in the ClickHouse schema with no current report consumer are not carried over: `games`, `organization_memberships`, and `analytics_ready` (replaced by `_ingest_state.status`). Add a table only when a report needs it.

## Report coverage

| Endpoint | Tables read |
|---|---|
| `GET /stats/dashboard` | `transactions`, `sessions`, `users`, `wallets`, `devices` |
| `GET /stats/staff-dashboard` | `transactions`, `sessions`, `users`, `devices` |
| `GET /stats/revenue/by-payment-method` | `transactions` |
| `GET /stats/usage` | `sessions` |
| `GET /stats/finance/reconciliation` | `cash_registers`, `cash_deposits` |
| `GET /stats/finance/deposits` | `cash_deposits` |
| `GET /stats/finance/variance` | `cash_registers` |
| `GET /stats/business` | `transactions`, `transaction_lines`, `sessions`, `session_hours`, `devices`, `users`, `plans`, `products`, `wallets`, `shifts` |
| Finance report (`handlers/finance_report.rs`) | `transactions`, `expenses`, `expense_categories`, `credit_settlements` |
| `GET /expenses/summary` | `expenses`, `expense_categories` |
| `GET /credit/summary` | `transactions`, `users`, `credit_settlements` |
| `GET /inventory/overview` | `location_stock`, `products`, `reorder_rules`, `purchase_orders`, `stock_transfer_requests`, `stock_waste_events`, `stock_movements`, `inventory_locations` |
| `GET /inventory/receipts/summary` | `stock_receipts`, `stock_receipt_lines`, `products`, `vendors` |
| `GET /inventory/waste/summary` | `stock_waste_events`, `stock_waste_lines`, `products`, `inventory_locations` |

### Location scoping

Today `scope.rs` rewrites every table reference into a scoped CTE. In DuckDB the tenant is the file, so scoping is only by location:

| Table | Location predicate |
|---|---|
| `transactions`, `sessions`, `session_hours`, `shifts`, `devices`, `credit_settlements`, `expenses`, `cash_registers`, `cash_deposits` | `location_id IN (…)` |
| `transaction_lines`, `credit_settlement_items` | through their parent transaction |
| `stock_*`, `location_stock`, `reorder_rules` | `location_id IN (SELECT id FROM inventory_locations WHERE venue_location_id IN (…))` |
| `purchase_orders` | `destination_location_id` via `inventory_locations` |
| `stock_transfer_requests` | `from_location_id` or `to_location_id` via `inventory_locations` |
| `users`, `wallets` | players who transacted or played at the selected locations (same rule as today) |
| Catalog tables | unscoped (historical names retained) |

`credit_settlements`, `expenses`, `cash_registers`, and `cash_deposits` carry `location_id` resolved from the shift when the event is written, replacing today's `shiftId IN (SELECT … FROM shifts …)` subqueries.

## ClickHouse → DuckDB translation

| ClickHouse | DuckDB |
|---|---|
| `countIf(c)` | `count(*) FILTER (WHERE c)` |
| `sumIf(x, c)` | `sum(x) FILTER (WHERE c)` |
| `uniqExact(x)` / `uniqExactIf(x, c)` | `count(DISTINCT x)` / `count(DISTINCT x) FILTER (WHERE c)` |
| `avgOrNull(x)` | `avg(x)` |
| `toDate(t, 'Asia/Kolkata')` | precomputed `local_date` |
| `toDate(t, 'UTC')` | `CAST(t AS DATE)` (timestamps are stored in UTC) |
| `toStartOfHour` + `ARRAY JOIN range(…)` | precomputed `session_hours`; open sessions split in Rust with the same function |
| `dateDiff('second', a, b)` | `date_diff('second', a, b)` |
| `dateDiff('day', a, b)` | `date_diff('day', a, b)`; verify boundary semantics in the retention tests |
| `toDecimalString(x, 2)` | `CAST(CAST(x AS DECIMAL(19,2)) AS VARCHAR)`; verify rounding against current outputs |
| `x::Float64` | `x::DOUBLE` |
| `toString(uuid)` | `CAST(id AS VARCHAR)` |
| `… FINAL WHERE _deleted = 0` views | not needed |

### Example: daily sales (`SALES`)

```sql
SELECT CAST(local_date AS VARCHAR)                                        AS date,
       sum(amount)::DOUBLE                                                AS revenue,
       (sum(amount) FILTER (WHERE transaction_type = 'plan_purchase'))::DOUBLE    AS plan_revenue,
       (sum(amount) FILTER (WHERE transaction_type = 'product_purchase'))::DOUBLE AS pos_revenue,
       count(*)                                                           AS transactions,
       count(DISTINCT player_id)                                          AS buyers
FROM transactions
WHERE is_booked AND occurred_at >= $1 AND occurred_at < $2
GROUP BY local_date
ORDER BY local_date;
```

### Example: busy hours (`HOURS`)

Closed sessions come from `session_hours`:

```sql
SELECT CAST(local_date AS VARCHAR), weekday, local_hour,
       sum(occupied_seconds) / 3600.0             AS hours,
       count(*) FILTER (WHERE is_start_hour)      AS starts
FROM session_hours
WHERE hour_start >= $1 AND hour_start < $2
GROUP BY 1, 2, 3
ORDER BY 1, 3;
```

Open sessions (at most one per device) are read from `sessions WHERE end_time IS NULL`. They're split in Rust with the same hour-splitting function the consumer uses to build `session_hours`, clipped to `[start, now)`, and merged into the result. Keeping one implementation of the split means closed and open sessions can't bucket differently.

Buckets are tenant-local hours. IST is UTC+05:30, so local hour boundaries fall on UTC half-hours; truncating to UTC hours would put time in the wrong bucket. The parity tests must cover a session that crosses local midnight and one that starts and ends within a single UTC hour but spans two local hours.

### Example: customers (`CUSTOMER_CTE`)

```sql
WITH visits AS (
  SELECT player_id                                            AS player,
         min(start_time)                                      AS first_visit,
         max(start_time)                                      AS last_visit,
         count(*) FILTER (WHERE start_time >= $1)             AS visits,
         count(*) FILTER (WHERE start_time >= $3 AND start_time < $1) AS prior
  FROM sessions
  WHERE NOT is_staff_allowance AND start_time < $2
  GROUP BY player_id
)
SELECT …  -- unchanged summary and detail projections
```

The wallet join disappears because `player_id` and `is_staff_allowance` are resolved on the session when the event is written.

## Behaviours preserved deliberately

These differences exist in today's queries and are kept as-is so reports don't change during the migration. Resolve them in a separate change if they're unintended:

- The finance report groups days by **UTC** date; the business report uses the tenant calendar (IST today).
- Stock value in `/inventory/overview` uses `purchase_price_per_box / units_per_purchase_unit`, falling back to `purchase_price`. Waste cost in `/inventory/waste/summary` uses `coalesce(purchase_price_per_box, purchase_price) / greatest(units_per_purchase_unit, 1)`.
- `/stats/*` filters use inclusive `BETWEEN` bounds; the business and finance reports use half-open `[start, end)`.

## Ingestion

- **Live**: the cell's analytics consumer buffers JetStream events per tenant and flushes at 500–1,000 events or 1 second, whichever comes first (§18). Each flush is one DuckDB transaction:
  1. collapse to the last state per key;
  2. upsert or delete the target rows;
  3. replace `session_hours` rows for affected closed sessions;
  4. advance `_ingest_state.last_sequence`.

  The JetStream acknowledgement happens after the commit.
- **Canonical snapshots**: tenant migration 0015 adds a separate `analytics_snapshot` column to the outbox envelope. The outbox writer captures an explicit, secret-free row projection inside the business transaction; public/realtime payloads are unchanged. Money travels as exact decimal text converted from scale-4 SQLite integers. Child collections are complete parent replacements, and stock changes include the current composite stock row alongside each immutable movement. `analytics_snapshot.rs` shares the source projection with rebuilds. Pre-upgrade rows without a full projection require a consistent rebuild; the consumer never substitutes a later read of live SQLite. Derived stock IDs are deterministic UUIDs made from the location/product key.
- **Replay and isolation**: a filtered durable consumer per tenant buffers up to 500 messages or one second (with an eight-MiB byte cap), validates tenant/subject/version/sequences, and collapses replacements within one fenced DuckDB transaction. Old sequences are skipped; ACKs follow the commit. A gap leaves facts/checkpoints unchanged, marks LAGGING and re-fetches unacknowledged messages. A second gap requests REBUILDING. Empty polling does not keep tenant handles alive. Ingestion/event-gap/failure counters are exported. Production feature activation and initial/recovery builds follow in API-0042.
- **Initial build and rebuild**: follow ADR-0043 decision 21. Take a `VACUUM INTO` snapshot, read T0 from the snapshot's `sqlite_sequence` watermark for `outbox_events` (zero before the first event), `ATTACH` it read-only (DuckDB `sqlite` extension), and run one `INSERT … SELECT` per table for rows inside the hot window. Then build `session_hours`, rebuild `monthly_summary`, replay events with sequence > T0, and set status `READY`.
- **Retention**: nightly, in batches. Delete fact rows older than `hot_window_start`, then refresh `monthly_summary` for the current and previous month.
- **Schema change** (§32 Case C): create the new version's tables alongside, backfill, validate, switch, then drop the old tables.

## Parity tests

Before ClickHouse is removed, every report must return identical results on the ported demo dataset (`pnpm demo:seed`). The existing ClickHouse test cases carry over:

- duplicate and out-of-order delivery;
- tombstones;
- refunds;
- exact decimal totals;
- overnight session clipping;
- repurchase, retention and attach-rate definitions;
- exclusion of refunded sales and credit collections.

New cases:

- a sequence gap stops ingestion;
- a rebuild during concurrent writes loses no events after T0;
- a session crossing local midnight;
- retention keeps `monthly_summary` while deleting old facts.
