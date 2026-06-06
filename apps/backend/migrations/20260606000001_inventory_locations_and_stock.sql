-- Inventory locations, multi-location stock, ledger, receipts, transfers, waste, product pricing extensions

CREATE TYPE inventory_location_kind AS ENUM ('warehouse', 'store');

CREATE TYPE stock_movement_type AS ENUM (
  'receipt',
  'transfer_out',
  'transfer_in',
  'sale',
  'waste',
  'adjustment'
);

CREATE TYPE stock_transfer_status AS ENUM ('pending', 'approved', 'rejected', 'fulfilled');

CREATE TYPE stock_waste_status AS ENUM ('pending', 'approved', 'rejected');

CREATE TYPE stock_waste_reason AS ENUM ('expired', 'damaged', 'spoilage', 'sample', 'other');

CREATE TABLE inventory_locations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name VARCHAR(200) NOT NULL,
  kind inventory_location_kind NOT NULL,
  "isActive" BOOLEAN NOT NULL DEFAULT true,
  "createdBy" UUID REFERENCES users(id),
  "updatedBy" UUID REFERENCES users(id),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "deletedAt" TIMESTAMPTZ
);

CREATE INDEX idx_inventory_locations_kind ON inventory_locations(kind);
CREATE INDEX idx_inventory_locations_active ON inventory_locations("isActive") WHERE "deletedAt" IS NULL;

CREATE TABLE location_stock (
  "locationId" UUID NOT NULL REFERENCES inventory_locations(id),
  "productId" UUID NOT NULL REFERENCES products(id),
  "quantityPieces" INTEGER NOT NULL DEFAULT 0 CHECK ("quantityPieces" >= 0),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY ("locationId", "productId")
);

CREATE INDEX idx_location_stock_product ON location_stock("productId");

CREATE TABLE stock_movements (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "locationId" UUID NOT NULL REFERENCES inventory_locations(id),
  "productId" UUID NOT NULL REFERENCES products(id),
  delta INTEGER NOT NULL,
  "movementType" stock_movement_type NOT NULL,
  "referenceId" UUID,
  "referenceType" VARCHAR(50),
  "createdBy" UUID REFERENCES users(id),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_stock_movements_location ON stock_movements("locationId");
CREATE INDEX idx_stock_movements_product ON stock_movements("productId");
CREATE INDEX idx_stock_movements_reference ON stock_movements("referenceType", "referenceId");
CREATE INDEX idx_stock_movements_created ON stock_movements("createdAt" DESC);

CREATE TABLE stock_receipts (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "locationId" UUID NOT NULL REFERENCES inventory_locations(id),
  "vendorId" UUID REFERENCES vendors(id),
  notes TEXT,
  "createdBy" UUID REFERENCES users(id),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE stock_receipt_lines (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "receiptId" UUID NOT NULL REFERENCES stock_receipts(id) ON DELETE CASCADE,
  "productId" UUID NOT NULL REFERENCES products(id),
  "boxQuantity" INTEGER NOT NULL CHECK ("boxQuantity" > 0),
  "piecesAdded" INTEGER NOT NULL CHECK ("piecesAdded" > 0)
);

CREATE INDEX idx_stock_receipt_lines_receipt ON stock_receipt_lines("receiptId");

CREATE TABLE stock_transfer_requests (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "fromLocationId" UUID NOT NULL REFERENCES inventory_locations(id),
  "toLocationId" UUID NOT NULL REFERENCES inventory_locations(id),
  status stock_transfer_status NOT NULL DEFAULT 'pending',
  "requestedBy" UUID REFERENCES users(id),
  "approvedBy" UUID REFERENCES users(id),
  "approvedAt" TIMESTAMPTZ,
  "rejectionReason" TEXT,
  "fulfilledBy" UUID REFERENCES users(id),
  "fulfilledAt" TIMESTAMPTZ,
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_stock_transfer_requests_status ON stock_transfer_requests(status);

CREATE TABLE stock_transfer_lines (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "transferRequestId" UUID NOT NULL REFERENCES stock_transfer_requests(id) ON DELETE CASCADE,
  "productId" UUID NOT NULL REFERENCES products(id),
  "quantityPieces" INTEGER NOT NULL CHECK ("quantityPieces" > 0)
);

CREATE INDEX idx_stock_transfer_lines_request ON stock_transfer_lines("transferRequestId");

CREATE TABLE stock_waste_events (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "locationId" UUID NOT NULL REFERENCES inventory_locations(id),
  status stock_waste_status NOT NULL DEFAULT 'pending',
  notes TEXT,
  "approvedBy" UUID REFERENCES users(id),
  "approvedAt" TIMESTAMPTZ,
  "rejectionReason" TEXT,
  "createdBy" UUID REFERENCES users(id),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_stock_waste_events_status ON stock_waste_events(status);
CREATE INDEX idx_stock_waste_events_location ON stock_waste_events("locationId");

CREATE TABLE stock_waste_lines (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "wasteEventId" UUID NOT NULL REFERENCES stock_waste_events(id) ON DELETE CASCADE,
  "productId" UUID NOT NULL REFERENCES products(id),
  "quantityPieces" INTEGER NOT NULL CHECK ("quantityPieces" > 0),
  "reasonCode" stock_waste_reason NOT NULL,
  note TEXT
);

CREATE INDEX idx_stock_waste_lines_event ON stock_waste_lines("wasteEventId");

-- Extend products with unit linkage and day/night pricing
ALTER TABLE products ADD COLUMN IF NOT EXISTS "unitId" UUID REFERENCES units(id);
ALTER TABLE products ADD COLUMN IF NOT EXISTS "purchaseUnitId" UUID REFERENCES units(id);
ALTER TABLE products ADD COLUMN IF NOT EXISTS "unitsPerPurchaseUnit" INTEGER NOT NULL DEFAULT 1;
ALTER TABLE products ADD COLUMN IF NOT EXISTS "dayPrice" DECIMAL(19,4);
ALTER TABLE products ADD COLUMN IF NOT EXISTS "nightPrice" DECIMAL(19,4);
ALTER TABLE products ADD COLUMN IF NOT EXISTS "purchasePricePerBox" DECIMAL(19,4);

UPDATE products
SET
  "dayPrice" = COALESCE("dayPrice", price),
  "nightPrice" = COALESCE("nightPrice", price),
  "purchasePricePerBox" = COALESCE("purchasePricePerBox", "purchasePrice")
WHERE "dayPrice" IS NULL OR "nightPrice" IS NULL;

ALTER TABLE products ALTER COLUMN "dayPrice" SET NOT NULL;
ALTER TABLE products ALTER COLUMN "nightPrice" SET NOT NULL;

-- Keep legacy columns in sync
UPDATE products SET price = "dayPrice" WHERE price IS DISTINCT FROM "dayPrice";

-- Seed default warehouse and store locations
DO $$
DECLARE
  warehouse_id UUID := gen_random_uuid();
  store_id UUID := gen_random_uuid();
  admin_id UUID;
BEGIN
  SELECT id INTO admin_id FROM users WHERE role = 'admin' ORDER BY "createdAt" ASC LIMIT 1;

  INSERT INTO inventory_locations (id, name, kind, "isActive", "createdBy", "updatedBy")
  VALUES
    (warehouse_id, 'Main Warehouse', 'warehouse', true, admin_id, admin_id),
    (store_id, 'Front Store', 'store', true, admin_id, admin_id);

  INSERT INTO location_stock ("locationId", "productId", "quantityPieces")
  SELECT store_id, p.id, GREATEST(p."stockQuantity", 0)
  FROM products p
  WHERE p."deletedAt" IS NULL;

  INSERT INTO configurations (key, value, category, description) VALUES
    ('inventory.default_warehouse_id', to_jsonb(warehouse_id::text), 'inventory', 'Default warehouse location ID'),
    ('inventory.default_store_id', to_jsonb(store_id::text), 'inventory', 'Default store location ID'),
    ('pos.default_sale_location_id', to_jsonb(store_id::text), 'pos', 'Default POS sale location ID')
  ON CONFLICT (key) DO NOTHING;
END $$;
