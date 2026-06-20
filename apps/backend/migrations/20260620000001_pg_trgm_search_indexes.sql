-- pg_trgm GIN indexes for admin ILIKE substring search (DRAFT-0042).

CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- users (user list + credit player search)
CREATE INDEX idx_users_username_trgm
  ON users USING gin (username gin_trgm_ops)
  WHERE "deletedAt" IS NULL;
CREATE INDEX idx_users_phone_trgm
  ON users USING gin ("phoneNumber" gin_trgm_ops)
  WHERE "deletedAt" IS NULL;
CREATE INDEX idx_users_first_name_trgm
  ON users USING gin ("firstName" gin_trgm_ops)
  WHERE "deletedAt" IS NULL;
CREATE INDEX idx_users_last_name_trgm
  ON users USING gin ("lastName" gin_trgm_ops)
  WHERE "deletedAt" IS NULL;

-- plans
CREATE INDEX idx_plans_name_trgm
  ON plans USING gin (name gin_trgm_ops)
  WHERE "deletedAt" IS NULL;
CREATE INDEX idx_plans_description_trgm
  ON plans USING gin (description gin_trgm_ops)
  WHERE "deletedAt" IS NULL;

-- products
CREATE INDEX idx_products_name_trgm
  ON products USING gin (name gin_trgm_ops)
  WHERE "deletedAt" IS NULL;

-- devices
CREATE INDEX idx_devices_name_trgm
  ON devices USING gin (name gin_trgm_ops)
  WHERE "deletedAt" IS NULL;
CREATE INDEX idx_devices_location_trgm
  ON devices USING gin (location gin_trgm_ops)
  WHERE "deletedAt" IS NULL;
