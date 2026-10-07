-- Tenant DuckDB schema v1 (docs/architecture/duckdb-analytics-schema.md).
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
