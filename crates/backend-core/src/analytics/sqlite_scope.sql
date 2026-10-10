-- These placeholders are substituted only from typed UUID/date/calendar values.
-- Tenant isolation is the database file; facts retain their venue snapshot.
report_inventory_locations AS NOT MATERIALIZED (
  SELECT * FROM inventory_locations
  WHERE __ALL__ OR venue_location_id IN (__VENUES__)
),
report_transactions AS NOT MATERIALIZED (
  SELECT * FROM transactions
  WHERE (__ALL__ OR location_id IN (__VENUES__))
),
report_sessions AS NOT MATERIALIZED (
  SELECT * FROM sessions
  WHERE NOT is_staff_allowance AND (__ALL__ OR location_id IN (__VENUES__))
),
report_shifts AS NOT MATERIALIZED (
  SELECT * FROM shifts
  WHERE (__ALL__ OR location_id IN (__VENUES__))
),
report_credit_settlements AS NOT MATERIALIZED (
  SELECT * FROM credit_settlements
  WHERE (__ALL__ OR location_id IN (__VENUES__))
),
report_expenses AS NOT MATERIALIZED (
  SELECT * FROM expenses
  WHERE (__ALL__ OR location_id IN (__VENUES__))
),
report_cash_registers AS NOT MATERIALIZED (
  SELECT * FROM cash_registers
  WHERE (__ALL__ OR location_id IN (__VENUES__))
),
report_cash_deposits AS NOT MATERIALIZED (
  SELECT * FROM cash_deposits
  WHERE (__ALL__ OR location_id IN (__VENUES__))
),
report_transaction_lines AS NOT MATERIALIZED (
  SELECT * FROM transaction_lines
  WHERE transaction_id IN (SELECT id FROM report_transactions)
),
report_credit_settlement_items AS NOT MATERIALIZED (
  SELECT * FROM credit_settlement_items
  WHERE settlement_id IN (SELECT id FROM report_credit_settlements)
    AND transaction_id IN (SELECT id FROM report_transactions)
),
report_users AS NOT MATERIALIZED (
  SELECT * FROM users
  WHERE __ALL__ OR id IN (
    SELECT player_id FROM report_transactions
    UNION SELECT player_id FROM report_sessions
  )
),
report_wallets AS NOT MATERIALIZED (
  SELECT * FROM wallets
  WHERE coalesce(kind,'time') != 'staff_allowance'
    AND (__ALL__ OR player_id IN (SELECT id FROM report_users))
),
report_devices AS NOT MATERIALIZED (
  SELECT * FROM devices WHERE __ALL__ OR location_id IN (__VENUES__)
),
-- Historical names remain usable when a device or a staff member changes venue.
report_device_names AS NOT MATERIALIZED (
  SELECT * FROM devices WHERE __ALL__ OR location_id IN (__VENUES__)
    OR id IN (SELECT device_id FROM report_sessions)
),
report_staff AS NOT MATERIALIZED (
  SELECT * FROM users WHERE role IN ('admin','staff') AND (
    __ALL__ OR id IN (
      SELECT user_id FROM report_shifts
      UNION SELECT created_by FROM report_transactions
      UNION SELECT created_by FROM report_sessions
    )
  )
),
report_location_stock AS NOT MATERIALIZED (
  SELECT * FROM location_stock
  WHERE location_id IN (SELECT id FROM report_inventory_locations)
),
report_reorder_rules AS NOT MATERIALIZED (
  SELECT * FROM reorder_rules
  WHERE location_id IN (SELECT id FROM report_inventory_locations)
),
report_purchase_orders AS NOT MATERIALIZED (
  SELECT * FROM purchase_orders
  WHERE __ALL__ OR destination_location_id IN (SELECT id FROM report_inventory_locations)
),
report_stock_transfer_requests AS NOT MATERIALIZED (
  SELECT * FROM stock_transfer_requests
  WHERE __ALL__ OR from_location_id IN (SELECT id FROM report_inventory_locations)
    OR to_location_id IN (SELECT id FROM report_inventory_locations)
),
report_stock_receipts AS NOT MATERIALIZED (
  SELECT * FROM stock_receipts
  WHERE location_id IN (SELECT id FROM report_inventory_locations)
),
report_stock_receipt_lines AS NOT MATERIALIZED (
  SELECT * FROM stock_receipt_lines
  WHERE receipt_id IN (SELECT id FROM report_stock_receipts)
),
report_stock_waste_events AS NOT MATERIALIZED (
  SELECT * FROM stock_waste_events
  WHERE location_id IN (SELECT id FROM report_inventory_locations)
),
report_stock_waste_lines AS NOT MATERIALIZED (
  SELECT * FROM stock_waste_lines
  WHERE waste_event_id IN (SELECT id FROM report_stock_waste_events)
),
report_stock_movements AS NOT MATERIALIZED (
  SELECT * FROM stock_movements
  WHERE location_id IN (SELECT id FROM report_inventory_locations)
)
