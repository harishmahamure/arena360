-- These placeholders are substituted only from typed UUID/date/calendar values.
-- Tenant isolation is the database file; facts retain their venue snapshot.
report_inventory_locations AS (
  SELECT * FROM inventory_locations
  WHERE __ALL__ OR venue_location_id IN (__VENUES__)
),
report_transactions AS (
  SELECT * FROM transactions
  WHERE local_date >= DATE '__HOT_DATE__'
    AND (__ALL__ OR location_id IN (__VENUES__))
),
report_sessions AS (
  SELECT * FROM sessions
  WHERE NOT is_staff_allowance
    AND (end_time IS NULL OR end_time >= TIMESTAMP '__HOT_UTC__')
    AND (__ALL__ OR location_id IN (__VENUES__))
),
report_session_hours AS (
  SELECT * FROM session_hours
  WHERE local_date >= DATE '__HOT_DATE__'
    AND session_id IN (SELECT id FROM report_sessions)
    AND (__ALL__ OR location_id IN (__VENUES__))
),
report_shifts AS (
  SELECT * FROM shifts
  WHERE (clock_out IS NULL OR clock_out >= TIMESTAMP '__HOT_UTC__')
    AND (__ALL__ OR location_id IN (__VENUES__))
),
report_credit_settlements AS (
  SELECT * FROM credit_settlements
  WHERE settled_at >= TIMESTAMP '__HOT_UTC__'
    AND (__ALL__ OR location_id IN (__VENUES__))
),
report_expenses AS (
  SELECT * FROM expenses
  WHERE local_date >= DATE '__HOT_DATE__'
    AND (__ALL__ OR location_id IN (__VENUES__))
),
report_cash_registers AS (
  SELECT * FROM cash_registers
  WHERE created_at >= TIMESTAMP '__HOT_UTC__'
    AND (__ALL__ OR location_id IN (__VENUES__))
),
report_cash_deposits AS (
  SELECT * FROM cash_deposits
  WHERE created_at >= TIMESTAMP '__HOT_UTC__'
    AND (__ALL__ OR location_id IN (__VENUES__))
),
report_transaction_lines AS (
  SELECT * FROM transaction_lines
  WHERE transaction_id IN (SELECT id FROM report_transactions)
),
report_credit_settlement_items AS (
  SELECT * FROM credit_settlement_items
  WHERE settlement_id IN (SELECT id FROM report_credit_settlements)
    AND transaction_id IN (SELECT id FROM report_transactions)
),
report_users AS (
  SELECT * FROM users
  WHERE __ALL__ OR id IN (
    SELECT player_id FROM report_transactions
    UNION SELECT player_id FROM report_sessions
  )
),
report_wallets AS (
  SELECT * FROM wallets
  WHERE coalesce(kind,'time') != 'staff_allowance'
    AND (__ALL__ OR player_id IN (SELECT id FROM report_users))
),
report_devices AS (
  SELECT * FROM devices WHERE __ALL__ OR location_id IN (__VENUES__)
),
-- Historical names remain usable when a device or a staff member changes venue.
report_device_names AS (
  SELECT * FROM devices WHERE __ALL__ OR location_id IN (__VENUES__)
    OR id IN (SELECT device_id FROM report_sessions)
),
report_staff AS (
  SELECT * FROM users WHERE role IN ('admin','staff') AND (
    __ALL__ OR id IN (
      SELECT user_id FROM report_shifts
      UNION SELECT created_by FROM report_transactions
      UNION SELECT created_by FROM report_sessions
    )
  )
),
report_location_stock AS (
  SELECT * FROM location_stock
  WHERE location_id IN (SELECT id FROM report_inventory_locations)
),
report_reorder_rules AS (
  SELECT * FROM reorder_rules
  WHERE location_id IN (SELECT id FROM report_inventory_locations)
),
report_purchase_orders AS (
  SELECT * FROM purchase_orders
  WHERE __ALL__ OR destination_location_id IN (SELECT id FROM report_inventory_locations)
),
report_stock_transfer_requests AS (
  SELECT * FROM stock_transfer_requests
  WHERE __ALL__ OR from_location_id IN (SELECT id FROM report_inventory_locations)
    OR to_location_id IN (SELECT id FROM report_inventory_locations)
),
report_stock_receipts AS (
  SELECT * FROM stock_receipts
  WHERE received_at >= TIMESTAMP '__HOT_UTC__'
    AND location_id IN (SELECT id FROM report_inventory_locations)
),
report_stock_receipt_lines AS (
  SELECT * FROM stock_receipt_lines
  WHERE receipt_id IN (SELECT id FROM report_stock_receipts)
),
report_stock_waste_events AS (
  SELECT * FROM stock_waste_events
  WHERE (approved_at IS NULL OR approved_at >= TIMESTAMP '__HOT_UTC__')
    AND location_id IN (SELECT id FROM report_inventory_locations)
),
report_stock_waste_lines AS (
  SELECT * FROM stock_waste_lines
  WHERE waste_event_id IN (SELECT id FROM report_stock_waste_events)
),
report_stock_movements AS (
  SELECT * FROM stock_movements
  WHERE created_at >= TIMESTAMP '__HOT_UTC__'
    AND location_id IN (SELECT id FROM report_inventory_locations)
)
