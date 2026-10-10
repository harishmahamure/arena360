-- Core M4 venue schema (ADR-0046).
-- UUIDs and timestamps are generated and validated at Rust write boundaries; primary IDs retain
-- canonical shape checks here, while foreign keys inherit canonical identity from their parents.

CREATE TABLE venue_locations (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  slug TEXT NOT NULL CHECK (length(trim(slug)) BETWEEN 1 AND 80),
  name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 160),
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  created_at TEXT NOT NULL CHECK (
    length(created_at) = 27 AND created_at GLOB
      '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9].[0-9][0-9][0-9][0-9][0-9][0-9]Z'
  ),
  updated_at TEXT NOT NULL CHECK (
    length(updated_at) = 27 AND updated_at GLOB
      '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9].[0-9][0-9][0-9][0-9][0-9][0-9]Z'
  )
) STRICT;
CREATE UNIQUE INDEX venue_locations_slug_unique ON venue_locations(lower(slug));
CREATE UNIQUE INDEX venue_locations_name_unique ON venue_locations(lower(name));

CREATE TABLE users (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  email TEXT,
  username TEXT NOT NULL CHECK (length(trim(username)) BETWEEN 1 AND 100),
  password_hash TEXT,
  first_name TEXT CHECK (first_name IS NULL OR length(first_name) <= 50),
  last_name TEXT CHECK (last_name IS NULL OR length(last_name) <= 50),
  phone_number TEXT,
  avatar_url TEXT,
  role TEXT NOT NULL DEFAULT 'player' CHECK (role IN ('player', 'staff', 'admin')),
  permissions TEXT NOT NULL DEFAULT '[]'
    CHECK (json_valid(permissions) = 1 AND json_type(permissions) = 'array'),
  member_revision INTEGER NOT NULL DEFAULT 1 CHECK (member_revision > 0),
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  credit_limit INTEGER NOT NULL DEFAULT 0 CHECK (credit_limit >= 0),
  created_by TEXT,
  updated_by TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z'),
  deleted_at TEXT CHECK (deleted_at IS NULL OR (length(deleted_at) = 27 AND substr(deleted_at, -1) = 'Z')),
  CHECK (
    (role = 'player' AND password_hash IS NOT NULL AND length(password_hash) > 0)
    OR (role IN ('staff', 'admin') AND password_hash IS NULL)
  )
) STRICT;
CREATE UNIQUE INDEX users_username_live_unique
  ON users(lower(username)) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX users_email_live_unique
  ON users(lower(email)) WHERE deleted_at IS NULL AND email IS NOT NULL;
CREATE INDEX users_role_created_live
  ON users(role, created_at DESC) WHERE deleted_at IS NULL;

CREATE TABLE access_assignments (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  role_id TEXT NOT NULL REFERENCES access_roles(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  PRIMARY KEY (user_id, role_id)
) STRICT;

CREATE TABLE location_role_assignments (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  location_id TEXT NOT NULL REFERENCES venue_locations(id) ON DELETE CASCADE,
  role_id TEXT NOT NULL REFERENCES access_roles(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  PRIMARY KEY (user_id, location_id, role_id)
) STRICT;
CREATE INDEX location_role_assignments_location ON location_role_assignments(location_id, user_id);

CREATE TABLE devices (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  location_id TEXT NOT NULL REFERENCES venue_locations(id),
  name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 100),
  serial_number TEXT CHECK (serial_number IS NULL OR length(serial_number) <= 100),
  local_ip_address TEXT CHECK (local_ip_address IS NULL OR length(local_ip_address) <= 100),
  device_type TEXT NOT NULL DEFAULT 'OTHER'
    CHECK (device_type IN ('PC', 'CONSOLE', 'PS5', 'PS4', 'OTHER')),
  device_sub_type TEXT NOT NULL DEFAULT 'OTHER'
    CHECK (device_sub_type IN (
      'HIGH_END_PCS', 'MID_RANGE_PCS', 'PREMIUM_TV_CONSOLES',
      'STANDARD_TV_CONSOLES', 'OTHER'
    )),
  location TEXT CHECK (location IS NULL OR length(location) <= 200),
  status TEXT NOT NULL DEFAULT 'available'
    CHECK (status IN ('operational', 'under_maintenance', 'out_of_service', 'in_use', 'available')),
  registered_kiosk TEXT CHECK (
    registered_kiosk IS NULL
    OR (json_valid(registered_kiosk) = 1 AND json_type(registered_kiosk) = 'object')
  ),
  registration_status TEXT NOT NULL DEFAULT 'unregistered'
    CHECK (registration_status IN ('registered', 'unregistered')),
  created_by TEXT,
  updated_by TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z'),
  deleted_at TEXT CHECK (deleted_at IS NULL OR (length(deleted_at) = 27 AND substr(deleted_at, -1) = 'Z'))
) STRICT;
CREATE UNIQUE INDEX devices_name_live_unique
  ON devices(lower(name)) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX devices_serial_live_unique
  ON devices(lower(serial_number)) WHERE deleted_at IS NULL AND serial_number IS NOT NULL;
CREATE INDEX devices_location_live ON devices(location_id) WHERE deleted_at IS NULL;

CREATE TABLE plans (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 100),
  description TEXT,
  price INTEGER NOT NULL CHECK (price >= 0),
  plan_type TEXT NOT NULL CHECK (plan_type IN (
    'time_based', 'session_based', 'unlimited_daily', 'hourly_rental',
    'monthly_subscription', 'weekend_special'
  )),
  validity_days INTEGER NOT NULL DEFAULT 30 CHECK (validity_days > 0),
  time_window_start TEXT CHECK (
    time_window_start IS NULL OR (
      length(time_window_start) = 8
      AND time_window_start GLOB '[0-2][0-9]:[0-5][0-9]:[0-5][0-9]'
    )
  ),
  time_window_end TEXT CHECK (
    time_window_end IS NULL OR (
      length(time_window_end) = 8
      AND time_window_end GLOB '[0-2][0-9]:[0-5][0-9]:[0-5][0-9]'
    )
  ),
  time_credits INTEGER NOT NULL DEFAULT 0 CHECK (time_credits >= 0),
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  device_type TEXT CHECK (device_type IS NULL OR device_type IN ('PC', 'CONSOLE', 'PS5', 'PS4', 'OTHER')),
  device_sub_type TEXT CHECK (device_sub_type IS NULL OR device_sub_type IN (
    'HIGH_END_PCS', 'MID_RANGE_PCS', 'PREMIUM_TV_CONSOLES',
    'STANDARD_TV_CONSOLES', 'OTHER'
  )),
  allowed_days TEXT CHECK (
    allowed_days IS NULL OR (json_valid(allowed_days) = 1 AND json_type(allowed_days) = 'array')
  ),
  allowed_months TEXT CHECK (
    allowed_months IS NULL OR (json_valid(allowed_months) = 1 AND json_type(allowed_months) = 'array')
  ),
  dynamic_deduction_enabled INTEGER NOT NULL DEFAULT 0
    CHECK (dynamic_deduction_enabled IN (0, 1)),
  deduction_profile TEXT CHECK (
    deduction_profile IS NULL
    OR (json_valid(deduction_profile) = 1 AND json_type(deduction_profile) = 'object')
  ),
  availability_scope TEXT NOT NULL DEFAULT 'ALL'
    CHECK (availability_scope IN ('ALL', 'SELECTED')),
  created_by TEXT,
  updated_by TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z'),
  deleted_at TEXT CHECK (deleted_at IS NULL OR (length(deleted_at) = 27 AND substr(deleted_at, -1) = 'Z'))
) STRICT;
CREATE UNIQUE INDEX plans_name_live_unique ON plans(lower(name)) WHERE deleted_at IS NULL;
CREATE INDEX plans_active_live ON plans(is_active, plan_type) WHERE deleted_at IS NULL;

CREATE TABLE plan_locations (
  plan_id TEXT NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
  location_id TEXT NOT NULL REFERENCES venue_locations(id) ON DELETE CASCADE,
  price INTEGER CHECK (price IS NULL OR price >= 0),
  PRIMARY KEY (plan_id, location_id)
) STRICT;
CREATE INDEX plan_locations_location ON plan_locations(location_id, plan_id);

CREATE TABLE products (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 255),
  description TEXT,
  day_price INTEGER NOT NULL CHECK (day_price >= 0),
  night_price INTEGER NOT NULL CHECK (night_price >= 0),
  purchase_price_per_box INTEGER CHECK (purchase_price_per_box IS NULL OR purchase_price_per_box >= 0),
  unit_id TEXT REFERENCES units(id),
  purchase_unit_id TEXT REFERENCES units(id),
  units_per_purchase_unit INTEGER NOT NULL DEFAULT 1 CHECK (units_per_purchase_unit > 0),
  category TEXT NOT NULL DEFAULT 'other' CHECK (category IN ('beverage', 'snack', 'meal', 'other')),
  sku TEXT CHECK (sku IS NULL OR length(sku) <= 50),
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  is_raw_material INTEGER NOT NULL DEFAULT 0 CHECK (is_raw_material IN (0, 1)),
  availability_scope TEXT NOT NULL DEFAULT 'ALL'
    CHECK (availability_scope IN ('ALL', 'SELECTED')),
  created_by TEXT,
  updated_by TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z'),
  deleted_at TEXT CHECK (deleted_at IS NULL OR (length(deleted_at) = 27 AND substr(deleted_at, -1) = 'Z'))
) STRICT;
CREATE UNIQUE INDEX products_sku_live_unique
  ON products(lower(sku)) WHERE deleted_at IS NULL AND sku IS NOT NULL;
CREATE INDEX products_catalog_live
  ON products(category, is_active, is_raw_material) WHERE deleted_at IS NULL;

CREATE TABLE product_locations (
  product_id TEXT NOT NULL REFERENCES products(id) ON DELETE CASCADE,
  location_id TEXT NOT NULL REFERENCES venue_locations(id) ON DELETE CASCADE,
  price INTEGER CHECK (price IS NULL OR price >= 0),
  PRIMARY KEY (product_id, location_id)
) STRICT;
CREATE INDEX product_locations_location ON product_locations(location_id, product_id);

CREATE TABLE games (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 120),
  thumbnail_url TEXT,
  logo_url TEXT,
  video_url TEXT,
  launch_ref TEXT CHECK (launch_ref IS NULL OR length(launch_ref) <= 255),
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_by TEXT,
  updated_by TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z'),
  deleted_at TEXT CHECK (deleted_at IS NULL OR (length(deleted_at) = 27 AND substr(deleted_at, -1) = 'Z'))
) STRICT;
CREATE INDEX games_active_sort ON games(is_active, sort_order) WHERE deleted_at IS NULL;

CREATE TABLE pricing_rule_sets (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 160),
  description TEXT,
  availability_scope TEXT NOT NULL DEFAULT 'ALL'
    CHECK (availability_scope IN ('ALL', 'SELECTED')),
  active_version_id TEXT REFERENCES pricing_rule_versions(id) DEFERRABLE INITIALLY DEFERRED,
  created_by TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z')
) STRICT;

CREATE TABLE pricing_rule_set_locations (
  rule_set_id TEXT NOT NULL REFERENCES pricing_rule_sets(id) ON DELETE CASCADE,
  location_id TEXT NOT NULL REFERENCES venue_locations(id) ON DELETE CASCADE,
  PRIMARY KEY (rule_set_id, location_id)
) STRICT;
CREATE INDEX pricing_rule_set_locations_location
  ON pricing_rule_set_locations(location_id, rule_set_id);

CREATE TABLE pricing_rule_versions (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  rule_set_id TEXT NOT NULL REFERENCES pricing_rule_sets(id) ON DELETE CASCADE,
  version INTEGER NOT NULL CHECK (version > 0),
  status TEXT NOT NULL DEFAULT 'draft'
    CHECK (status IN ('draft', 'validated', 'scheduled', 'published', 'superseded')),
  policy TEXT NOT NULL CHECK (json_valid(policy) = 1 AND json_type(policy) = 'object'),
  simulation_hash TEXT CHECK (simulation_hash IS NULL OR length(simulation_hash) <= 64),
  validated_at TEXT CHECK (validated_at IS NULL OR (length(validated_at) = 27 AND substr(validated_at, -1) = 'Z')),
  effective_at TEXT CHECK (effective_at IS NULL OR (length(effective_at) = 27 AND substr(effective_at, -1) = 'Z')),
  published_at TEXT CHECK (published_at IS NULL OR (length(published_at) = 27 AND substr(published_at, -1) = 'Z')),
  created_by TEXT,
  published_by TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  UNIQUE (rule_set_id, version)
) STRICT;
CREATE UNIQUE INDEX pricing_rule_versions_one_published
  ON pricing_rule_versions(rule_set_id) WHERE status = 'published';
CREATE INDEX pricing_rule_versions_effective ON pricing_rule_versions(status, effective_at);

CREATE TABLE setting_revisions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  location_id TEXT REFERENCES venue_locations(id),
  key TEXT NOT NULL CHECK (length(trim(key)) BETWEEN 1 AND 120),
  revision INTEGER NOT NULL CHECK (revision > 0),
  operation TEXT NOT NULL CHECK (operation IN ('create', 'update', 'delete')),
  old_value TEXT CHECK (old_value IS NULL OR json_valid(old_value) = 1),
  new_value TEXT CHECK (new_value IS NULL OR json_valid(new_value) = 1),
  reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),
  actor_user_id TEXT,
  request_id TEXT CHECK (request_id IS NULL OR length(request_id) <= 128),
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z')
) STRICT;
CREATE INDEX setting_revisions_history
  ON setting_revisions(COALESCE(location_id, ''), key, revision DESC);

CREATE TABLE product_recipe_items (
  product_id TEXT NOT NULL REFERENCES products(id) ON DELETE CASCADE,
  ingredient_id TEXT NOT NULL REFERENCES products(id),
  quantity INTEGER NOT NULL CHECK (quantity > 0),
  PRIMARY KEY (product_id, ingredient_id),
  CHECK (product_id <> ingredient_id)
) STRICT;
CREATE INDEX product_recipe_items_ingredient ON product_recipe_items(ingredient_id);

CREATE TABLE product_option_groups (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  product_id TEXT NOT NULL REFERENCES products(id) ON DELETE CASCADE,
  name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 120),
  required INTEGER NOT NULL DEFAULT 0 CHECK (required IN (0, 1)),
  multiple INTEGER NOT NULL DEFAULT 0 CHECK (multiple IN (0, 1)),
  sort_order INTEGER NOT NULL DEFAULT 0
) STRICT;
CREATE INDEX product_option_groups_product ON product_option_groups(product_id, sort_order);

CREATE TABLE product_options (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  group_id TEXT NOT NULL REFERENCES product_option_groups(id) ON DELETE CASCADE,
  name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 120),
  price_delta INTEGER NOT NULL DEFAULT 0,
  sort_order INTEGER NOT NULL DEFAULT 0
) STRICT;
CREATE INDEX product_options_group ON product_options(group_id, sort_order);

CREATE TABLE product_option_ingredients (
  option_id TEXT NOT NULL REFERENCES product_options(id) ON DELETE CASCADE,
  ingredient_id TEXT NOT NULL REFERENCES products(id),
  quantity INTEGER NOT NULL CHECK (quantity <> 0),
  PRIMARY KEY (option_id, ingredient_id)
) STRICT;

CREATE TABLE inventory_locations (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  venue_location_id TEXT NOT NULL REFERENCES venue_locations(id),
  name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 200),
  kind TEXT NOT NULL CHECK (kind IN ('warehouse', 'store')),
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  created_by TEXT,
  updated_by TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z'),
  deleted_at TEXT CHECK (deleted_at IS NULL OR (length(deleted_at) = 27 AND substr(deleted_at, -1) = 'Z'))
) STRICT;
CREATE INDEX inventory_locations_venue_kind_live
  ON inventory_locations(venue_location_id, kind) WHERE deleted_at IS NULL;

CREATE TABLE location_stock (
  inventory_location_id TEXT NOT NULL REFERENCES inventory_locations(id),
  product_id TEXT NOT NULL REFERENCES products(id),
  quantity_pieces INTEGER NOT NULL DEFAULT 0 CHECK (quantity_pieces >= 0),
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z'),
  PRIMARY KEY (inventory_location_id, product_id)
) STRICT;
CREATE INDEX location_stock_product ON location_stock(product_id, inventory_location_id);

CREATE TABLE stock_movements (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  inventory_location_id TEXT NOT NULL REFERENCES inventory_locations(id),
  product_id TEXT NOT NULL REFERENCES products(id),
  delta INTEGER NOT NULL CHECK (delta <> 0),
  movement_type TEXT NOT NULL CHECK (movement_type IN (
    'receipt', 'transfer_out', 'transfer_in', 'sale', 'waste', 'adjustment'
  )),
  reference_id TEXT,
  reference_type TEXT CHECK (reference_type IS NULL OR length(reference_type) <= 50),
  created_by TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  CHECK ((reference_id IS NULL) = (reference_type IS NULL))
) STRICT;
CREATE INDEX stock_movements_location_time
  ON stock_movements(inventory_location_id, created_at DESC);
CREATE INDEX stock_movements_product_time ON stock_movements(product_id, created_at DESC);
CREATE INDEX stock_movements_reference ON stock_movements(reference_type, reference_id);

CREATE TABLE shifts (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  user_id TEXT NOT NULL REFERENCES users(id),
  location_id TEXT NOT NULL REFERENCES venue_locations(id),
  clock_in TEXT NOT NULL CHECK (length(clock_in) = 27 AND substr(clock_in, -1) = 'Z'),
  clock_out TEXT CHECK (clock_out IS NULL OR (length(clock_out) = 27 AND substr(clock_out, -1) = 'Z')),
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'closed')),
  notes TEXT,
  created_by TEXT,
  updated_by TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z'),
  CHECK (clock_out IS NULL OR clock_out >= clock_in)
) STRICT;
CREATE UNIQUE INDEX shifts_one_active_user
  ON shifts(user_id) WHERE status = 'active';
CREATE INDEX shifts_location_clock_in ON shifts(location_id, clock_in DESC);

CREATE TABLE player_plans (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  player_id TEXT NOT NULL REFERENCES users(id),
  plan_id TEXT NOT NULL REFERENCES plans(id),
  purchase_date TEXT NOT NULL CHECK (length(purchase_date) = 27 AND substr(purchase_date, -1) = 'Z'),
  activation_date TEXT CHECK (activation_date IS NULL OR (length(activation_date) = 27 AND substr(activation_date, -1) = 'Z')),
  expiry_date TEXT NOT NULL CHECK (length(expiry_date) = 27 AND substr(expiry_date, -1) = 'Z'),
  remaining_usage_count INTEGER CHECK (remaining_usage_count IS NULL OR remaining_usage_count >= 0),
  remaining_time_credits INTEGER CHECK (remaining_time_credits IS NULL OR remaining_time_credits >= 0),
  status TEXT NOT NULL DEFAULT 'active'
    CHECK (status IN ('active', 'expired', 'exhausted', 'cancelled', 'moved_to_next_plan')),
  moved_to_plan_id TEXT REFERENCES player_plans(id),
  moved_credits_count INTEGER CHECK (moved_credits_count IS NULL OR moved_credits_count >= 0),
  created_by TEXT,
  updated_by TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z'),
  deleted_at TEXT CHECK (deleted_at IS NULL OR (length(deleted_at) = 27 AND substr(deleted_at, -1) = 'Z')),
  CHECK (expiry_date >= purchase_date)
) STRICT;
CREATE INDEX player_plans_player_status_live
  ON player_plans(player_id, status, expiry_date) WHERE deleted_at IS NULL;

CREATE TABLE player_plan_balances (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  player_id TEXT NOT NULL REFERENCES users(id),
  device_type TEXT CHECK (device_type IS NULL OR device_type IN ('PC', 'CONSOLE', 'PS5', 'PS4', 'OTHER')),
  device_sub_type TEXT CHECK (device_sub_type IS NULL OR device_sub_type IN (
    'HIGH_END_PCS', 'MID_RANGE_PCS', 'PREMIUM_TV_CONSOLES',
    'STANDARD_TV_CONSOLES', 'OTHER'
  )),
  kind TEXT NOT NULL CHECK (kind IN ('time', 'happy_hours', 'staff_allowance')),
  remaining_minutes INTEGER NOT NULL DEFAULT 0 CHECK (remaining_minutes >= 0),
  expiry_date TEXT NOT NULL CHECK (length(expiry_date) = 27 AND substr(expiry_date, -1) = 'Z'),
  window_start TEXT CHECK (
    window_start IS NULL OR (
      length(window_start) = 8 AND window_start GLOB '[0-2][0-9]:[0-5][0-9]:[0-5][0-9]'
    )
  ),
  window_end TEXT CHECK (
    window_end IS NULL OR (
      length(window_end) = 8 AND window_end GLOB '[0-2][0-9]:[0-5][0-9]:[0-5][0-9]'
    )
  ),
  status TEXT NOT NULL DEFAULT 'active'
    CHECK (status IN ('active', 'expired', 'exhausted', 'cancelled')),
  source_plan_id TEXT REFERENCES plans(id),
  allowed_days TEXT CHECK (
    allowed_days IS NULL OR (json_valid(allowed_days) = 1 AND json_type(allowed_days) = 'array')
  ),
  allowed_months TEXT CHECK (
    allowed_months IS NULL OR (json_valid(allowed_months) = 1 AND json_type(allowed_months) = 'array')
  ),
  deduction_profile TEXT CHECK (
    deduction_profile IS NULL
    OR (json_valid(deduction_profile) = 1 AND json_type(deduction_profile) = 'object')
  ),
  created_by TEXT,
  updated_by TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z'),
  deleted_at TEXT CHECK (deleted_at IS NULL OR (length(deleted_at) = 27 AND substr(deleted_at, -1) = 'Z'))
) STRICT;
CREATE UNIQUE INDEX player_plan_balances_active_scope_unique
  ON player_plan_balances(
    player_id, COALESCE(device_type, ''), COALESCE(device_sub_type, ''), kind
  ) WHERE status = 'active' AND deleted_at IS NULL;
CREATE INDEX player_plan_balances_player_status
  ON player_plan_balances(player_id, status) WHERE deleted_at IS NULL;
CREATE INDEX player_plan_balances_player_expiry
  ON player_plan_balances(player_id, expiry_date) WHERE status = 'active' AND deleted_at IS NULL;

CREATE TABLE transactions (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  player_id TEXT NOT NULL REFERENCES users(id),
  plan_id TEXT REFERENCES plans(id),
  shift_id TEXT REFERENCES shifts(id),
  location_id TEXT NOT NULL REFERENCES venue_locations(id),
  transaction_type TEXT NOT NULL CHECK (transaction_type IN ('plan_purchase', 'product_purchase')),
  amount INTEGER NOT NULL CHECK (amount >= 0),
  paid_amount INTEGER NOT NULL DEFAULT 0 CHECK (paid_amount >= 0 AND paid_amount <= amount),
  cash_amount INTEGER NOT NULL DEFAULT 0 CHECK (cash_amount >= 0),
  online_amount INTEGER NOT NULL DEFAULT 0 CHECK (online_amount >= 0),
  payment_method TEXT NOT NULL CHECK (payment_method IN ('cash', 'online', 'split_payment', 'credit')),
  payment_status TEXT NOT NULL DEFAULT 'pending'
    CHECK (payment_status IN ('pending', 'completed', 'failed', 'refunded', 'credit')),
  notes TEXT,
  online_payment_ref_last4 TEXT CHECK (
    online_payment_ref_last4 IS NULL
    OR (length(online_payment_ref_last4) = 4 AND online_payment_ref_last4 NOT GLOB '*[^0-9]*')
  ),
  transaction_date TEXT NOT NULL CHECK (length(transaction_date) = 27 AND substr(transaction_date, -1) = 'Z'),
  created_by TEXT,
  updated_by TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z'),
  deleted_at TEXT CHECK (deleted_at IS NULL OR (length(deleted_at) = 27 AND substr(deleted_at, -1) = 'Z')),
  CHECK (cash_amount + online_amount <= paid_amount),
  CHECK (
    payment_status <> 'completed' OR payment_method = 'credit'
    OR (paid_amount = amount AND cash_amount + online_amount = amount)
  )
) STRICT;
CREATE INDEX transactions_time_live
  ON transactions(transaction_date DESC, id) WHERE deleted_at IS NULL;
CREATE INDEX transactions_shift_time_live
  ON transactions(shift_id, transaction_date DESC, id) WHERE deleted_at IS NULL;
CREATE INDEX transactions_player_time_live
  ON transactions(player_id, transaction_date DESC) WHERE deleted_at IS NULL;
CREATE INDEX transactions_location_status_time_live
  ON transactions(location_id, payment_status, transaction_date DESC, id)
  WHERE deleted_at IS NULL;
CREATE INDEX transactions_open_credit
  ON transactions(player_id, transaction_date)
  WHERE deleted_at IS NULL AND payment_method = 'credit' AND payment_status = 'credit';

CREATE TABLE usage_sessions (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  player_id TEXT NOT NULL REFERENCES users(id),
  balance_id TEXT NOT NULL REFERENCES player_plan_balances(id),
  device_id TEXT NOT NULL REFERENCES devices(id),
  location_id TEXT NOT NULL REFERENCES venue_locations(id),
  shift_id TEXT REFERENCES shifts(id),
  start_time TEXT NOT NULL CHECK (length(start_time) = 27 AND substr(start_time, -1) = 'Z'),
  end_time TEXT CHECK (end_time IS NULL OR (length(end_time) = 27 AND substr(end_time, -1) = 'Z')),
  duration_minutes INTEGER CHECK (duration_minutes IS NULL OR duration_minutes >= 0),
  time_credits_consumed INTEGER CHECK (time_credits_consumed IS NULL OR time_credits_consumed >= 0),
  wallet_minutes_at_start INTEGER NOT NULL CHECK (wallet_minutes_at_start >= 0),
  source_plan_id_at_start TEXT REFERENCES plans(id),
  deduction_profile_snapshot TEXT CHECK (
    deduction_profile_snapshot IS NULL
    OR (json_valid(deduction_profile_snapshot) = 1 AND json_type(deduction_profile_snapshot) = 'object')
  ),
  end_reason TEXT CHECK (
    end_reason IS NULL OR end_reason IN ('voluntary', 'auto', 'force', 'offline_reconcile')
  ),
  created_by TEXT,
  updated_by TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z'),
  deleted_at TEXT CHECK (deleted_at IS NULL OR (length(deleted_at) = 27 AND substr(deleted_at, -1) = 'Z')),
  CHECK (end_time IS NULL OR end_time >= start_time),
  CHECK ((end_time IS NULL) = (end_reason IS NULL))
) STRICT;
CREATE UNIQUE INDEX usage_sessions_one_open_device
  ON usage_sessions(device_id) WHERE end_time IS NULL AND deleted_at IS NULL;
CREATE UNIQUE INDEX usage_sessions_one_open_balance
  ON usage_sessions(balance_id) WHERE end_time IS NULL AND deleted_at IS NULL;
CREATE UNIQUE INDEX usage_sessions_one_open_player
  ON usage_sessions(player_id) WHERE end_time IS NULL AND deleted_at IS NULL;
CREATE INDEX usage_sessions_start_live
  ON usage_sessions(start_time DESC, id) WHERE deleted_at IS NULL;
CREATE INDEX usage_sessions_shift ON usage_sessions(shift_id);

CREATE TABLE player_plan_ledger (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  balance_id TEXT NOT NULL REFERENCES player_plan_balances(id),
  player_id TEXT NOT NULL REFERENCES users(id),
  delta_minutes INTEGER NOT NULL CHECK (delta_minutes <> 0),
  reason TEXT NOT NULL CHECK (reason IN (
    'purchase', 'recharge', 'session_usage', 'expiry', 'adjustment', 'migration',
    'staff_allowance_grant', 'staff_allowance_renewal'
  )),
  transaction_id TEXT REFERENCES transactions(id),
  session_id TEXT REFERENCES usage_sessions(id),
  balance_after INTEGER NOT NULL CHECK (balance_after >= 0),
  expiry_after TEXT NOT NULL CHECK (length(expiry_after) = 27 AND substr(expiry_after, -1) = 'Z'),
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  created_by TEXT,
  CHECK (transaction_id IS NOT NULL OR session_id IS NOT NULL OR reason IN (
    'expiry', 'adjustment', 'migration', 'staff_allowance_grant', 'staff_allowance_renewal'
  ))
) STRICT;
CREATE INDEX player_plan_ledger_balance_time
  ON player_plan_ledger(balance_id, created_at DESC);
CREATE INDEX player_plan_ledger_player_time ON player_plan_ledger(player_id, created_at DESC);

CREATE TABLE transaction_products (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  transaction_id TEXT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
  product_id TEXT REFERENCES products(id),
  product_name TEXT NOT NULL,
  product_sku TEXT,
  quantity INTEGER NOT NULL CHECK (quantity > 0),
  unit_price INTEGER NOT NULL CHECK (unit_price >= 0),
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z')
) STRICT;
CREATE INDEX transaction_products_transaction ON transaction_products(transaction_id);
CREATE INDEX transaction_products_product ON transaction_products(product_id);

CREATE TABLE transaction_product_options (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  transaction_product_id TEXT NOT NULL REFERENCES transaction_products(id) ON DELETE CASCADE,
  option_id TEXT REFERENCES product_options(id) ON DELETE SET NULL,
  group_name TEXT NOT NULL CHECK (length(trim(group_name)) BETWEEN 1 AND 120),
  name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 120),
  price_delta INTEGER NOT NULL
) STRICT;
CREATE INDEX transaction_product_options_line
  ON transaction_product_options(transaction_product_id);

CREATE TABLE credit_settlements (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  player_id TEXT NOT NULL REFERENCES users(id),
  settled_by TEXT NOT NULL REFERENCES users(id),
  shift_id TEXT NOT NULL REFERENCES shifts(id),
  amount INTEGER NOT NULL CHECK (amount > 0),
  payment_method TEXT NOT NULL CHECK (payment_method IN ('cash', 'online', 'split_payment')),
  cash_amount INTEGER NOT NULL DEFAULT 0 CHECK (cash_amount >= 0),
  online_amount INTEGER NOT NULL DEFAULT 0 CHECK (online_amount >= 0),
  notes TEXT,
  online_payment_ref_last4 TEXT CHECK (
    online_payment_ref_last4 IS NULL
    OR (length(online_payment_ref_last4) = 4 AND online_payment_ref_last4 NOT GLOB '*[^0-9]*')
  ),
  settled_at TEXT NOT NULL CHECK (length(settled_at) = 27 AND substr(settled_at, -1) = 'Z'),
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z'),
  deleted_at TEXT CHECK (deleted_at IS NULL OR (length(deleted_at) = 27 AND substr(deleted_at, -1) = 'Z')),
  CHECK (cash_amount + online_amount = amount)
) STRICT;
CREATE INDEX credit_settlements_player_time
  ON credit_settlements(player_id, settled_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX credit_settlements_shift ON credit_settlements(shift_id);

CREATE TABLE credit_settlement_items (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  settlement_id TEXT NOT NULL REFERENCES credit_settlements(id) ON DELETE CASCADE,
  transaction_id TEXT NOT NULL REFERENCES transactions(id),
  amount_applied INTEGER NOT NULL CHECK (amount_applied > 0),
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  UNIQUE (settlement_id, transaction_id)
) STRICT;
CREATE INDEX credit_settlement_items_transaction ON credit_settlement_items(transaction_id);

CREATE TABLE kiosk_orders (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  session_id TEXT NOT NULL REFERENCES usage_sessions(id),
  player_id TEXT NOT NULL REFERENCES users(id),
  device_id TEXT NOT NULL REFERENCES devices(id),
  status TEXT NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending', 'preparing', 'fulfilled', 'cancelled')),
  player_note TEXT,
  transaction_id TEXT REFERENCES transactions(id),
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  updated_at TEXT NOT NULL CHECK (length(updated_at) = 27 AND substr(updated_at, -1) = 'Z'),
  fulfilled_at TEXT CHECK (fulfilled_at IS NULL OR (length(fulfilled_at) = 27 AND substr(fulfilled_at, -1) = 'Z'))
) STRICT;
CREATE UNIQUE INDEX kiosk_orders_one_open_session
  ON kiosk_orders(session_id) WHERE status IN ('pending', 'preparing');
CREATE INDEX kiosk_orders_open_status ON kiosk_orders(status, created_at);
CREATE INDEX kiosk_orders_device_time ON kiosk_orders(device_id, created_at DESC);

CREATE TABLE kiosk_order_items (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  order_id TEXT NOT NULL REFERENCES kiosk_orders(id) ON DELETE CASCADE,
  product_id TEXT REFERENCES products(id),
  product_name TEXT NOT NULL,
  product_sku TEXT,
  quantity INTEGER NOT NULL CHECK (quantity > 0),
  unit_price INTEGER NOT NULL CHECK (unit_price >= 0),
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z')
) STRICT;
CREATE INDEX kiosk_order_items_order ON kiosk_order_items(order_id);
