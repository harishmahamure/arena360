CREATE TYPE purchase_order_status AS ENUM (
  'draft',
  'submitted',
  'approved',
  'rejected',
  'ordered',
  'partially_received',
  'received',
  'cancelled'
);

CREATE SEQUENCE purchase_order_number_seq START 1;

CREATE TABLE purchase_orders (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "poNumber" VARCHAR(40) NOT NULL UNIQUE,
  "vendorId" UUID NOT NULL REFERENCES vendors(id),
  "destinationLocationId" UUID NOT NULL REFERENCES inventory_locations(id),
  status purchase_order_status NOT NULL DEFAULT 'draft',
  "expectedDeliveryDate" DATE,
  subtotal DECIMAL(19,4) NOT NULL DEFAULT 0,
  discount DECIMAL(19,4) NOT NULL DEFAULT 0,
  tax DECIMAL(19,4) NOT NULL DEFAULT 0,
  freight DECIMAL(19,4) NOT NULL DEFAULT 0,
  total DECIMAL(19,4) NOT NULL DEFAULT 0,
  notes TEXT,
  "rejectionReason" TEXT,
  version INTEGER NOT NULL DEFAULT 1,
  "createdBy" UUID REFERENCES users(id),
  "submittedBy" UUID REFERENCES users(id),
  "submittedAt" TIMESTAMPTZ,
  "approvedBy" UUID REFERENCES users(id),
  "approvedAt" TIMESTAMPTZ,
  "orderedBy" UUID REFERENCES users(id),
  "orderedAt" TIMESTAMPTZ,
  "cancelledBy" UUID REFERENCES users(id),
  "cancelledAt" TIMESTAMPTZ,
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CHECK (subtotal >= 0 AND discount >= 0 AND tax >= 0 AND freight >= 0 AND total >= 0)
);

CREATE TABLE purchase_order_lines (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "purchaseOrderId" UUID NOT NULL REFERENCES purchase_orders(id) ON DELETE CASCADE,
  "productId" UUID NOT NULL REFERENCES products(id),
  "orderedBoxes" INTEGER NOT NULL CHECK ("orderedBoxes" > 0),
  "receivedBoxes" INTEGER NOT NULL DEFAULT 0 CHECK ("receivedBoxes" >= 0),
  "unitsPerBoxSnapshot" INTEGER NOT NULL CHECK ("unitsPerBoxSnapshot" > 0),
  "boxCostSnapshot" DECIMAL(19,4) NOT NULL CHECK ("boxCostSnapshot" >= 0),
  "taxRate" DECIMAL(8,4) NOT NULL DEFAULT 0 CHECK ("taxRate" >= 0),
  "lineSubtotal" DECIMAL(19,4) NOT NULL,
  "lineTax" DECIMAL(19,4) NOT NULL,
  "lineTotal" DECIMAL(19,4) NOT NULL,
  UNIQUE ("purchaseOrderId", "productId")
);

CREATE TABLE inventory_reorder_rules (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "locationId" UUID NOT NULL REFERENCES inventory_locations(id),
  "productId" UUID NOT NULL REFERENCES products(id),
  "minimumPieces" INTEGER NOT NULL DEFAULT 0 CHECK ("minimumPieces" >= 0),
  "targetPieces" INTEGER NOT NULL DEFAULT 0 CHECK ("targetPieces" >= "minimumPieces"),
  "preferredVendorId" UUID REFERENCES vendors(id),
  "leadTimeDays" INTEGER NOT NULL DEFAULT 0 CHECK ("leadTimeDays" >= 0),
  "isActive" BOOLEAN NOT NULL DEFAULT true,
  "createdBy" UUID REFERENCES users(id),
  "updatedBy" UUID REFERENCES users(id),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE ("locationId", "productId")
);

ALTER TABLE stock_receipts
  ADD COLUMN "purchaseOrderId" UUID REFERENCES purchase_orders(id),
  ADD COLUMN "invoiceReference" VARCHAR(120),
  ADD COLUMN "paymentMethod" VARCHAR(30),
  ADD COLUMN "paymentAccount" VARCHAR(120),
  ADD COLUMN "receiptDate" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  ADD COLUMN subtotal DECIMAL(19,4),
  ADD COLUMN tax DECIMAL(19,4),
  ADD COLUMN total DECIMAL(19,4),
  ADD COLUMN "exceptionalReason" TEXT;

ALTER TABLE stock_receipt_lines
  ADD COLUMN "purchaseOrderLineId" UUID REFERENCES purchase_order_lines(id),
  ADD COLUMN "acceptedBoxQuantity" INTEGER,
  ADD COLUMN "rejectedBoxQuantity" INTEGER NOT NULL DEFAULT 0,
  ADD COLUMN "boxCostSnapshot" DECIMAL(19,4),
  ADD COLUMN "taxRate" DECIMAL(8,4),
  ADD COLUMN "lineTotal" DECIMAL(19,4);

ALTER TABLE expenses
  ADD COLUMN "sourceType" VARCHAR(40),
  ADD COLUMN "sourceId" UUID;

CREATE UNIQUE INDEX idx_expenses_source_unique
  ON expenses("sourceType", "sourceId")
  WHERE "sourceType" IS NOT NULL AND "sourceId" IS NOT NULL AND "deletedAt" IS NULL;
CREATE INDEX idx_purchase_orders_status ON purchase_orders(status, "updatedAt" DESC);
CREATE INDEX idx_purchase_orders_vendor ON purchase_orders("vendorId", "createdAt" DESC);
CREATE INDEX idx_purchase_orders_destination ON purchase_orders("destinationLocationId");
CREATE INDEX idx_purchase_order_lines_order ON purchase_order_lines("purchaseOrderId");
CREATE INDEX idx_reorder_rules_location ON inventory_reorder_rules("locationId", "isActive");
CREATE INDEX idx_stock_receipts_purchase_order ON stock_receipts("purchaseOrderId");
CREATE UNIQUE INDEX idx_stock_receipts_po_invoice
  ON stock_receipts("purchaseOrderId", "invoiceReference")
  WHERE "purchaseOrderId" IS NOT NULL AND "invoiceReference" IS NOT NULL;

INSERT INTO expense_categories (id, name, description, "isActive", "createdAt", "updatedAt")
SELECT gen_random_uuid(), 'Inventory purchases',
       'Automatically posted from finalized purchase-order receipts', true, NOW(), NOW()
WHERE NOT EXISTS (
  SELECT 1 FROM expense_categories WHERE lower(name) = 'inventory purchases'
);
