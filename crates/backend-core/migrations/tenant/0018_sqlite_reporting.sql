-- Secret-free, nonmaterialized report views over operational tenant data.
-- Monetary columns remain INTEGER in scale-4 units; conversion occurs only at the API boundary.
CREATE VIEW report_base_users AS
SELECT r.id AS id,
       r.username AS username,
       r.first_name AS first_name,
       r.last_name AS last_name,
       r.role AS role,
       r.is_active AS is_active,
       r.credit_limit AS credit_limit,
       r.created_at AS created_at
FROM main.users r WHERE r.deleted_at IS NULL;

CREATE VIEW report_base_venue_locations AS
SELECT r.id AS id,
       r.name AS name
FROM main.venue_locations r WHERE 1=1;

CREATE VIEW report_base_devices AS
SELECT r.id AS id,
       r.name AS name,
       r.status AS status,
       r.location AS area_label,
       r.device_type AS device_type,
       r.location_id AS location_id
FROM main.devices r WHERE r.deleted_at IS NULL;

CREATE VIEW report_base_plans AS
SELECT r.id AS id,
       r.name AS name,
       r.price AS price,
       r.time_credits AS time_credit_minutes,
       r.plan_type AS plan_type,
       r.validity_days AS validity_days
FROM main.plans r WHERE r.deleted_at IS NULL;

CREATE VIEW report_base_products AS
SELECT r.id AS id,
       r.name AS name,
       r.purchase_price_per_box AS purchase_price_per_box,
       NULL AS purchase_price,
       r.units_per_purchase_unit AS units_per_purchase_unit
FROM main.products r WHERE r.deleted_at IS NULL;

CREATE VIEW report_base_vendors AS
SELECT r.id AS id,
       r.name AS name
FROM main.vendors r WHERE 1=1;

CREATE VIEW report_base_inventory_locations AS
SELECT r.id AS id,
       r.name AS name,
       r.venue_location_id AS venue_location_id
FROM main.inventory_locations r WHERE r.deleted_at IS NULL;

CREATE VIEW report_base_expense_categories AS
SELECT r.id AS id,
       r.name AS name,
       r.is_active AS is_active,
       r.budget_amount AS budget_amount,
       r.budget_period AS budget_period
FROM main.expense_categories r WHERE 1=1;

CREATE VIEW report_base_transactions AS
SELECT r.id AS id,
       r.transaction_date AS occurred_at,
       r.created_at AS created_at,
       r.location_id AS location_id,
       r.player_id AS player_id,
       r.plan_id AS plan_id,
       r.shift_id AS shift_id,
       r.created_by AS created_by,
       r.transaction_type AS transaction_type,
       r.payment_method AS payment_method,
       r.payment_status AS payment_status,
       r.amount AS amount,
       r.paid_amount AS paid_amount,
       r.cash_amount AS cash_amount,
       r.online_amount AS online_amount
FROM main.transactions r WHERE r.deleted_at IS NULL;

CREATE VIEW report_base_transaction_lines AS
SELECT r.id AS id,
       r.transaction_id AS transaction_id,
       COALESCE(r.product_id,'00000000-0000-0000-0000-000000000000') AS product_id,
       r.quantity AS quantity,
       r.unit_price AS unit_price
FROM main.transaction_products r WHERE 1=1;

CREATE VIEW report_base_sessions AS
SELECT r.id AS id,
       r.device_id AS device_id,
       r.balance_id AS balance_id,
       r.player_id AS player_id,
       b.kind = 'staff_allowance' AS is_staff_allowance,
       r.location_id AS location_id,
       r.start_time AS start_time,
       r.end_time AS end_time,
       NULL AS start_local_date,
       r.duration_minutes AS duration_minutes,
       r.shift_id AS shift_id,
       r.created_by AS created_by,
       r.source_plan_id_at_start AS source_plan_id
FROM main.usage_sessions r JOIN main.player_plan_balances b ON b.id=r.balance_id WHERE r.deleted_at IS NULL;

CREATE VIEW report_base_wallets AS
SELECT r.id AS id,
       r.player_id AS player_id,
       r.status AS status,
       r.kind AS kind,
       r.remaining_minutes AS remaining_minutes,
       r.expiry_date AS expiry_date,
       r.source_plan_id AS source_plan_id,
       r.created_at AS created_at
FROM main.player_plan_balances r WHERE r.deleted_at IS NULL;

CREATE VIEW report_base_shifts AS
SELECT r.id AS id,
       r.user_id AS user_id,
       r.location_id AS location_id,
       r.clock_in AS clock_in,
       r.clock_out AS clock_out,
       r.status AS status
FROM main.shifts r WHERE 1=1;

CREATE VIEW report_base_credit_settlements AS
SELECT r.id AS id,
       r.player_id AS player_id,
       r.shift_id AS shift_id,
       (SELECT s.location_id FROM main.shifts s WHERE s.id=r.shift_id) AS location_id,
       r.amount AS amount,
       r.payment_method AS payment_method,
       r.cash_amount AS cash_amount,
       r.online_amount AS online_amount,
       r.settled_at AS settled_at
FROM main.credit_settlements r WHERE r.deleted_at IS NULL;

CREATE VIEW report_base_credit_settlement_items AS
SELECT r.id AS id,
       r.settlement_id AS settlement_id,
       r.transaction_id AS transaction_id,
       r.amount_applied AS amount_applied
FROM main.credit_settlement_items r WHERE 1=1;

CREATE VIEW report_base_expenses AS
SELECT r.id AS id,
       r.category_id AS category_id,
       r.shift_id AS shift_id,
       r.location_id AS location_id,
       r.amount AS amount,
       r.approval_status AS approval_status,
       r.expense_date AS expense_date
FROM main.expenses r WHERE r.deleted_at IS NULL;

CREATE VIEW report_base_cash_registers AS
SELECT r.id AS id,
       r.shift_id AS shift_id,
       (SELECT s.location_id FROM main.shifts s WHERE s.id=r.shift_id) AS location_id,
       r.status AS status,
       r.variance AS variance,
       r.closing_balance AS closing_balance,
       r.expected_closing AS expected_closing,
       r.created_at AS created_at,
       r.updated_at AS updated_at
FROM main.cash_registers r WHERE 1=1;

CREATE VIEW report_base_cash_deposits AS
SELECT r.id AS id,
       r.shift_id AS shift_id,
       (SELECT s.location_id FROM main.shifts s WHERE s.id=r.shift_id) AS location_id,
       r.status AS status,
       r.amount AS amount,
       r.deposit_type AS deposit_type,
       r.created_at AS created_at
FROM main.cash_deposits r WHERE 1=1;

CREATE VIEW report_base_location_stock AS
SELECT r.inventory_location_id || ':' || r.product_id AS id,
       r.inventory_location_id AS location_id,
       r.product_id AS product_id,
       r.quantity_pieces AS quantity_pieces
FROM main.location_stock r WHERE 1=1;

CREATE VIEW report_base_reorder_rules AS
SELECT r.id AS id,
       r.inventory_location_id AS location_id,
       r.product_id AS product_id,
       r.is_active AS is_active,
       r.minimum_pieces AS minimum_pieces
FROM main.inventory_reorder_rules r WHERE 1=1;

CREATE VIEW report_base_purchase_orders AS
SELECT r.id AS id,
       r.status AS status,
       r.destination_location_id AS destination_location_id
FROM main.purchase_orders r WHERE 1=1;

CREATE VIEW report_base_stock_transfer_requests AS
SELECT r.id AS id,
       r.status AS status,
       r.from_location_id AS from_location_id,
       r.to_location_id AS to_location_id
FROM main.stock_transfer_requests r WHERE 1=1;

CREATE VIEW report_base_stock_receipts AS
SELECT r.id AS id,
       r.vendor_id AS vendor_id,
       r.inventory_location_id AS location_id,
       r.created_at AS received_at
FROM main.stock_receipts r WHERE 1=1;

CREATE VIEW report_base_stock_receipt_lines AS
SELECT r.id AS id,
       r.receipt_id AS receipt_id,
       r.product_id AS product_id,
       r.box_quantity AS box_quantity,
       r.pieces_added AS pieces_added
FROM main.stock_receipt_lines r WHERE 1=1;

CREATE VIEW report_base_stock_waste_events AS
SELECT r.id AS id,
       r.inventory_location_id AS location_id,
       r.status AS status,
       r.approved_at AS approved_at
FROM main.stock_waste_events r WHERE 1=1;

CREATE VIEW report_base_stock_waste_lines AS
SELECT r.id AS id,
       r.waste_event_id AS waste_event_id,
       r.product_id AS product_id,
       r.reason_code AS reason_code,
       r.quantity_pieces AS quantity_pieces
FROM main.stock_waste_lines r WHERE 1=1;

CREATE VIEW report_base_stock_movements AS
SELECT r.id AS id,
       r.inventory_location_id AS location_id,
       r.product_id AS product_id,
       r.delta AS delta,
       r.movement_type AS movement_type,
       r.reference_id AS reference_id,
       r.reference_type AS reference_type,
       r.created_by AS created_by,
       r.created_at AS created_at
FROM main.stock_movements r WHERE 1=1;

-- Report time predicates use raw canonical UTC columns, so these indexes can
-- serve range scans through the views. Existing venue/parent indexes are reused.
CREATE INDEX transactions_created_live ON transactions(created_at, id) WHERE deleted_at IS NULL;
CREATE INDEX credit_settlements_time_live ON credit_settlements(settled_at, id) WHERE deleted_at IS NULL;
CREATE INDEX cash_deposits_created ON cash_deposits(created_at, id);
CREATE INDEX cash_registers_created ON cash_registers(created_at, id);
CREATE INDEX cash_registers_variance_time ON cash_registers(status, updated_at, id)
  WHERE variance IS NOT NULL AND status IN ('closed','reconciled');
CREATE INDEX expenses_date_live ON expenses(expense_date, id) WHERE deleted_at IS NULL;
CREATE INDEX usage_sessions_end_live ON usage_sessions(end_time, start_time) WHERE deleted_at IS NULL;
CREATE INDEX shifts_clock_out ON shifts(clock_out, clock_in);
CREATE INDEX stock_receipts_created ON stock_receipts(created_at, id);
