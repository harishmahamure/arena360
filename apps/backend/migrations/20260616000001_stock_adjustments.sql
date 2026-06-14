-- Stock reconciliation / count adjustments with audit notes.

CREATE TABLE IF NOT EXISTS stock_adjustments (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "locationId" UUID NOT NULL REFERENCES inventory_locations(id),
  notes TEXT NOT NULL,
  "createdBy" UUID REFERENCES users(id),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS stock_adjustment_lines (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "adjustmentId" UUID NOT NULL REFERENCES stock_adjustments(id) ON DELETE CASCADE,
  "productId" UUID NOT NULL REFERENCES products(id),
  "previousPieces" INTEGER NOT NULL,
  "countedPieces" INTEGER NOT NULL,
  "deltaPieces" INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_stock_adjustments_location_created
  ON stock_adjustments ("locationId", "createdAt" DESC);
