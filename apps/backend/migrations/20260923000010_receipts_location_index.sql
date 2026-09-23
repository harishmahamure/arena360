-- no-transaction
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_stock_receipts_location_created_id ON stock_receipts ("locationId", "createdAt" DESC, id DESC);
