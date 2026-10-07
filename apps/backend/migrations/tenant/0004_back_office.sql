-- M5 back-office storage. Monetary values and tax rates are scale-4 integers.
-- Services supply UUID v7 IDs and fixed-width UTC timestamps at write boundaries.

CREATE TABLE vendors (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 200),
  contact_person TEXT,
  phone TEXT,
  email TEXT,
  address TEXT,
  gst_number TEXT,
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  notes TEXT,
  created_by TEXT,
  updated_by TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z')
) STRICT;
CREATE INDEX vendors_name ON vendors(lower(name));

CREATE TABLE expense_categories (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 100),
  description TEXT,
  parent_id TEXT REFERENCES expense_categories(id),
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  budget_amount INTEGER CHECK (budget_amount IS NULL OR budget_amount >= 0),
  budget_period TEXT,
  created_by TEXT,
  updated_by TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z'),
  CHECK (parent_id IS NULL OR parent_id <> id)
) STRICT;
CREATE UNIQUE INDEX expense_categories_name ON expense_categories(lower(name));
CREATE INDEX expense_categories_parent ON expense_categories(parent_id);

CREATE TABLE purchase_order_number_counter (
  singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
  next_value INTEGER NOT NULL CHECK (next_value > 0)
) STRICT;
INSERT INTO purchase_order_number_counter(singleton, next_value) VALUES (1, 1);

CREATE TABLE purchase_orders (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  po_number TEXT NOT NULL UNIQUE CHECK (length(trim(po_number)) BETWEEN 1 AND 40),
  vendor_id TEXT NOT NULL REFERENCES vendors(id),
  destination_location_id TEXT NOT NULL REFERENCES inventory_locations(id),
  status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN (
    'draft', 'submitted', 'approved', 'rejected', 'ordered',
    'partially_received', 'received', 'cancelled'
  )),
  expected_delivery_date TEXT CHECK (
    expected_delivery_date IS NULL OR
    (length(expected_delivery_date) = 10 AND expected_delivery_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]')
  ),
  subtotal INTEGER NOT NULL DEFAULT 0 CHECK (subtotal >= 0),
  discount INTEGER NOT NULL DEFAULT 0 CHECK (discount >= 0),
  tax INTEGER NOT NULL DEFAULT 0 CHECK (tax >= 0),
  freight INTEGER NOT NULL DEFAULT 0 CHECK (freight >= 0),
  total INTEGER NOT NULL DEFAULT 0 CHECK (total >= 0),
  notes TEXT,
  rejection_reason TEXT,
  version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
  created_by TEXT,
  submitted_by TEXT,
  submitted_at TEXT CHECK (submitted_at IS NULL OR (length(submitted_at) = 27 AND substr(submitted_at, -1) = 'Z')),
  approved_by TEXT,
  approved_at TEXT CHECK (approved_at IS NULL OR (length(approved_at) = 27 AND substr(approved_at, -1) = 'Z')),
  ordered_by TEXT,
  ordered_at TEXT CHECK (ordered_at IS NULL OR (length(ordered_at) = 27 AND substr(ordered_at, -1) = 'Z')),
  cancelled_by TEXT,
  cancelled_at TEXT CHECK (cancelled_at IS NULL OR (length(cancelled_at) = 27 AND substr(cancelled_at, -1) = 'Z')),
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z')
) STRICT;
CREATE INDEX purchase_orders_status_updated ON purchase_orders(status, updated_at DESC);
CREATE INDEX purchase_orders_vendor_created ON purchase_orders(vendor_id, created_at DESC);
CREATE INDEX purchase_orders_destination ON purchase_orders(destination_location_id);

CREATE TABLE purchase_order_lines (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  purchase_order_id TEXT NOT NULL REFERENCES purchase_orders(id) ON DELETE CASCADE,
  product_id TEXT NOT NULL REFERENCES products(id),
  ordered_boxes INTEGER NOT NULL CHECK (ordered_boxes > 0),
  received_boxes INTEGER NOT NULL DEFAULT 0 CHECK (received_boxes >= 0 AND received_boxes <= ordered_boxes),
  units_per_box_snapshot INTEGER NOT NULL CHECK (units_per_box_snapshot > 0),
  box_cost_snapshot INTEGER NOT NULL CHECK (box_cost_snapshot >= 0),
  tax_rate INTEGER NOT NULL DEFAULT 0 CHECK (tax_rate >= 0),
  line_subtotal INTEGER NOT NULL CHECK (line_subtotal >= 0),
  line_tax INTEGER NOT NULL CHECK (line_tax >= 0),
  line_total INTEGER NOT NULL CHECK (line_total >= 0),
  UNIQUE (purchase_order_id, product_id)
) STRICT;

CREATE TABLE inventory_reorder_rules (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  inventory_location_id TEXT NOT NULL REFERENCES inventory_locations(id),
  product_id TEXT NOT NULL REFERENCES products(id),
  minimum_pieces INTEGER NOT NULL DEFAULT 0 CHECK (minimum_pieces >= 0),
  target_pieces INTEGER NOT NULL DEFAULT 0 CHECK (target_pieces >= minimum_pieces),
  preferred_vendor_id TEXT REFERENCES vendors(id),
  lead_time_days INTEGER NOT NULL DEFAULT 0 CHECK (lead_time_days >= 0),
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  created_by TEXT,
  updated_by TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z'),
  UNIQUE (inventory_location_id, product_id)
) STRICT;
CREATE INDEX inventory_reorder_rules_location ON inventory_reorder_rules(inventory_location_id, is_active);

CREATE TABLE stock_receipts (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  inventory_location_id TEXT NOT NULL REFERENCES inventory_locations(id),
  vendor_id TEXT REFERENCES vendors(id),
  purchase_order_id TEXT REFERENCES purchase_orders(id),
  invoice_reference TEXT CHECK (invoice_reference IS NULL OR length(invoice_reference) <= 120),
  payment_method TEXT CHECK (payment_method IS NULL OR length(payment_method) <= 30),
  payment_account TEXT CHECK (payment_account IS NULL OR length(payment_account) <= 120),
  receipt_date TEXT NOT NULL CHECK (length(receipt_date) = 27 AND substr(receipt_date, -1) = 'Z'),
  subtotal INTEGER CHECK (subtotal IS NULL OR subtotal >= 0),
  tax INTEGER CHECK (tax IS NULL OR tax >= 0),
  total INTEGER CHECK (total IS NULL OR total >= 0),
  exceptional_reason TEXT,
  notes TEXT,
  created_by TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z')
) STRICT;
CREATE INDEX stock_receipts_location_date ON stock_receipts(inventory_location_id, receipt_date DESC);
CREATE INDEX stock_receipts_purchase_order ON stock_receipts(purchase_order_id);
CREATE UNIQUE INDEX stock_receipts_po_invoice ON stock_receipts(purchase_order_id, invoice_reference)
  WHERE purchase_order_id IS NOT NULL AND invoice_reference IS NOT NULL;

CREATE TABLE stock_receipt_lines (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  receipt_id TEXT NOT NULL REFERENCES stock_receipts(id) ON DELETE CASCADE,
  product_id TEXT NOT NULL REFERENCES products(id),
  purchase_order_line_id TEXT REFERENCES purchase_order_lines(id),
  box_quantity INTEGER NOT NULL CHECK (box_quantity > 0),
  accepted_box_quantity INTEGER CHECK (accepted_box_quantity IS NULL OR accepted_box_quantity >= 0),
  rejected_box_quantity INTEGER NOT NULL DEFAULT 0 CHECK (rejected_box_quantity >= 0),
  pieces_added INTEGER NOT NULL CHECK (pieces_added > 0),
  box_cost_snapshot INTEGER CHECK (box_cost_snapshot IS NULL OR box_cost_snapshot >= 0),
  tax_rate INTEGER CHECK (tax_rate IS NULL OR tax_rate >= 0),
  line_total INTEGER CHECK (line_total IS NULL OR line_total >= 0),
  CHECK (accepted_box_quantity IS NULL OR accepted_box_quantity + rejected_box_quantity <= box_quantity)
) STRICT;
CREATE INDEX stock_receipt_lines_receipt ON stock_receipt_lines(receipt_id);

CREATE TABLE stock_transfer_requests (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  from_location_id TEXT NOT NULL REFERENCES inventory_locations(id),
  to_location_id TEXT NOT NULL REFERENCES inventory_locations(id),
  status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'rejected', 'fulfilled')),
  requested_by TEXT,
  approved_by TEXT,
  approved_at TEXT CHECK (approved_at IS NULL OR (length(approved_at) = 27 AND substr(approved_at, -1) = 'Z')),
  rejection_reason TEXT,
  fulfilled_by TEXT,
  fulfilled_at TEXT CHECK (fulfilled_at IS NULL OR (length(fulfilled_at) = 27 AND substr(fulfilled_at, -1) = 'Z')),
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z'),
  CHECK (from_location_id <> to_location_id)
) STRICT;
CREATE INDEX stock_transfer_requests_status ON stock_transfer_requests(status, created_at DESC);
CREATE INDEX stock_transfer_requests_from ON stock_transfer_requests(from_location_id, created_at DESC);
CREATE INDEX stock_transfer_requests_to ON stock_transfer_requests(to_location_id, created_at DESC);

CREATE TABLE stock_transfer_lines (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  transfer_request_id TEXT NOT NULL REFERENCES stock_transfer_requests(id) ON DELETE CASCADE,
  product_id TEXT NOT NULL REFERENCES products(id),
  quantity_pieces INTEGER NOT NULL CHECK (quantity_pieces > 0),
  UNIQUE (transfer_request_id, product_id)
) STRICT;

CREATE TABLE stock_waste_events (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  inventory_location_id TEXT NOT NULL REFERENCES inventory_locations(id),
  status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'rejected')),
  notes TEXT,
  approved_by TEXT,
  approved_at TEXT CHECK (approved_at IS NULL OR (length(approved_at) = 27 AND substr(approved_at, -1) = 'Z')),
  rejection_reason TEXT,
  created_by TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z')
) STRICT;
CREATE INDEX stock_waste_events_status ON stock_waste_events(status, created_at DESC);
CREATE INDEX stock_waste_events_location ON stock_waste_events(inventory_location_id, created_at DESC);

CREATE TABLE stock_waste_lines (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  waste_event_id TEXT NOT NULL REFERENCES stock_waste_events(id) ON DELETE CASCADE,
  product_id TEXT NOT NULL REFERENCES products(id),
  quantity_pieces INTEGER NOT NULL CHECK (quantity_pieces > 0),
  reason_code TEXT NOT NULL CHECK (reason_code IN ('expired', 'damaged', 'spoilage', 'sample', 'other')),
  note TEXT
) STRICT;
CREATE INDEX stock_waste_lines_event ON stock_waste_lines(waste_event_id);

CREATE TABLE stock_adjustments (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  inventory_location_id TEXT NOT NULL REFERENCES inventory_locations(id),
  notes TEXT NOT NULL CHECK (length(trim(notes)) > 0),
  created_by TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z')
) STRICT;
CREATE INDEX stock_adjustments_location_created ON stock_adjustments(inventory_location_id, created_at DESC);

CREATE TABLE stock_adjustment_lines (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  adjustment_id TEXT NOT NULL REFERENCES stock_adjustments(id) ON DELETE CASCADE,
  product_id TEXT NOT NULL REFERENCES products(id),
  previous_pieces INTEGER NOT NULL CHECK (previous_pieces >= 0),
  counted_pieces INTEGER NOT NULL CHECK (counted_pieces >= 0),
  delta_pieces INTEGER NOT NULL,
  CHECK (delta_pieces = counted_pieces - previous_pieces),
  UNIQUE (adjustment_id, product_id)
) STRICT;

CREATE TABLE cash_registers (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  shift_id TEXT NOT NULL REFERENCES shifts(id),
  opened_by TEXT NOT NULL REFERENCES users(id),
  closed_by TEXT REFERENCES users(id),
  opening_balance INTEGER NOT NULL DEFAULT 0 CHECK (opening_balance >= 0),
  opening_denominations TEXT CHECK (opening_denominations IS NULL OR json_valid(opening_denominations) = 1),
  closing_balance INTEGER CHECK (closing_balance IS NULL OR closing_balance >= 0),
  closing_denominations TEXT CHECK (closing_denominations IS NULL OR json_valid(closing_denominations) = 1),
  expected_closing INTEGER CHECK (expected_closing IS NULL OR expected_closing >= 0),
  variance INTEGER,
  status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed', 'reconciled')),
  notes TEXT,
  reconciled_by TEXT REFERENCES users(id),
  reconciled_at TEXT CHECK (reconciled_at IS NULL OR (length(reconciled_at) = 27 AND substr(reconciled_at, -1) = 'Z')),
  reconciliation_notes TEXT,
  created_by TEXT,
  updated_by TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z')
) STRICT;
CREATE UNIQUE INDEX cash_registers_one_open_shift ON cash_registers(shift_id) WHERE status = 'open';
CREATE INDEX cash_registers_shift ON cash_registers(shift_id, created_at DESC);
CREATE INDEX cash_registers_status ON cash_registers(status, created_at DESC);

CREATE TABLE cash_register_entries (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  cash_register_id TEXT NOT NULL REFERENCES cash_registers(id),
  entry_type TEXT NOT NULL CHECK (length(trim(entry_type)) BETWEEN 1 AND 20),
  amount INTEGER NOT NULL CHECK (amount >= 0),
  reason TEXT,
  reference_id TEXT,
  reference_type TEXT,
  created_by TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z')
) STRICT;
CREATE INDEX cash_register_entries_register ON cash_register_entries(cash_register_id, created_at DESC);
CREATE INDEX cash_register_entries_reference ON cash_register_entries(reference_type, reference_id);

CREATE TABLE cash_deposits (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  cash_register_id TEXT NOT NULL REFERENCES cash_registers(id),
  shift_id TEXT NOT NULL REFERENCES shifts(id),
  initiated_by TEXT NOT NULL REFERENCES users(id),
  approved_by TEXT REFERENCES users(id),
  amount INTEGER NOT NULL CHECK (amount > 0),
  denominations TEXT NOT NULL CHECK (json_valid(denominations) = 1 AND json_type(denominations) = 'object'),
  deposit_type TEXT CHECK (deposit_type IS NULL OR length(deposit_type) <= 20),
  status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'rejected')),
  approved_at TEXT CHECK (approved_at IS NULL OR (length(approved_at) = 27 AND substr(approved_at, -1) = 'Z')),
  rejection_reason TEXT,
  notes TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z')
) STRICT;
CREATE INDEX cash_deposits_status ON cash_deposits(status, created_at DESC);
CREATE INDEX cash_deposits_shift ON cash_deposits(shift_id, created_at DESC);
CREATE INDEX cash_deposits_register ON cash_deposits(cash_register_id, created_at DESC);

CREATE TABLE expenses (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  category_id TEXT NOT NULL REFERENCES expense_categories(id),
  vendor_id TEXT REFERENCES vendors(id),
  amount INTEGER NOT NULL CHECK (amount > 0),
  payment_method TEXT NOT NULL CHECK (length(trim(payment_method)) BETWEEN 1 AND 20),
  payment_account TEXT,
  description TEXT,
  receipt_url TEXT,
  expense_date TEXT NOT NULL CHECK (length(expense_date) = 27 AND substr(expense_date, -1) = 'Z'),
  is_recurring INTEGER NOT NULL DEFAULT 0 CHECK (is_recurring IN (0, 1)),
  recurrence_pattern TEXT,
  next_recurrence_date TEXT CHECK (next_recurrence_date IS NULL OR (length(next_recurrence_date) = 27 AND substr(next_recurrence_date, -1) = 'Z')),
  approval_status TEXT NOT NULL DEFAULT 'pending' CHECK (approval_status IN ('pending', 'approved', 'rejected')),
  approved_by TEXT REFERENCES users(id),
  approved_at TEXT CHECK (approved_at IS NULL OR (length(approved_at) = 27 AND substr(approved_at, -1) = 'Z')),
  rejection_reason TEXT,
  shift_id TEXT REFERENCES shifts(id),
  cash_register_entry_id TEXT REFERENCES cash_register_entries(id),
  source_type TEXT,
  source_id TEXT,
  created_by TEXT,
  updated_by TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z'),
  deleted_at TEXT CHECK (deleted_at IS NULL OR (length(deleted_at) = 27 AND substr(deleted_at, -1) = 'Z')),
  CHECK ((source_type IS NULL) = (source_id IS NULL))
) STRICT;
CREATE INDEX expenses_category_date ON expenses(category_id, expense_date DESC) WHERE deleted_at IS NULL;
CREATE INDEX expenses_vendor ON expenses(vendor_id) WHERE deleted_at IS NULL;
CREATE INDEX expenses_status_date ON expenses(approval_status, expense_date DESC) WHERE deleted_at IS NULL;
CREATE INDEX expenses_shift ON expenses(shift_id) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX expenses_source_live ON expenses(source_type, source_id)
  WHERE source_type IS NOT NULL AND deleted_at IS NULL;

CREATE TABLE configurations (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  key TEXT NOT NULL UNIQUE CHECK (length(trim(key)) BETWEEN 1 AND 100),
  value TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(value) = 1),
  category TEXT NOT NULL DEFAULT 'general' CHECK (length(trim(category)) BETWEEN 1 AND 50),
  description TEXT,
  created_by TEXT,
  updated_by TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z')
) STRICT;
CREATE INDEX configurations_category ON configurations(category, key);

CREATE TABLE activity_log (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  kind TEXT NOT NULL CHECK (kind IN (
    'transaction_sale', 'plan_sale', 'credit_settlement', 'approval_requested',
    'approval_decided', 'session_started', 'session_ended', 'device_status_changed',
    'shift_clock_in', 'shift_clock_out', 'shift_handover', 'cash_register_opened',
    'cash_register_closed', 'cash_deposit_initiated', 'inventory_transfer_requested',
    'inventory_waste_recorded'
  )),
  title TEXT NOT NULL CHECK (length(trim(title)) > 0),
  summary TEXT,
  payload TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(payload) = 1 AND json_type(payload) = 'object'),
  actor_user_id TEXT REFERENCES users(id),
  entity_type TEXT,
  entity_id TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z')
) STRICT;
CREATE INDEX activity_log_created ON activity_log(created_at DESC);
CREATE INDEX activity_log_kind_created ON activity_log(kind, created_at DESC);

CREATE TABLE user_notifications (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  activity_id TEXT NOT NULL REFERENCES activity_log(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  read_at TEXT CHECK (read_at IS NULL OR (length(read_at) = 27 AND substr(read_at, -1) = 'Z')),
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  UNIQUE (activity_id, user_id)
) STRICT;
CREATE INDEX user_notifications_user_unread ON user_notifications(user_id, created_at DESC)
  WHERE read_at IS NULL;

CREATE TABLE kitchen_menu_settings (
  product_id TEXT PRIMARY KEY REFERENCES products(id),
  enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
  station TEXT NOT NULL CHECK (length(trim(station)) BETWEEN 1 AND 60),
  prep_minutes INTEGER NOT NULL CHECK (prep_minutes BETWEEN 1 AND 240),
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0),
  updated_by TEXT REFERENCES users(id),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z')
) STRICT;

CREATE TABLE kitchen_tickets (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  transaction_id TEXT NOT NULL UNIQUE REFERENCES transactions(id),
  status TEXT NOT NULL DEFAULT 'queued' CHECK (status IN ('queued', 'preparing', 'ready', 'served', 'cancelled')),
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0),
  items TEXT NOT NULL CHECK (json_valid(items) = 1 AND json_type(items) = 'array'),
  customer TEXT NOT NULL CHECK (length(trim(customer)) > 0),
  notes TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  due_at TEXT NOT NULL CHECK (length(due_at) = 27 AND substr(due_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z')
) STRICT;
CREATE INDEX kitchen_tickets_queue ON kitchen_tickets(status, created_at);

CREATE TABLE kitchen_ticket_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  ticket_id TEXT NOT NULL REFERENCES kitchen_tickets(id),
  status TEXT NOT NULL CHECK (status IN ('queued', 'preparing', 'ready', 'served', 'cancelled')),
  actor_id TEXT REFERENCES users(id),
  reason TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z')
) STRICT;
CREATE INDEX kitchen_ticket_events_ticket ON kitchen_ticket_events(ticket_id, id);
