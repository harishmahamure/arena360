-- no-transaction
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_stock_waste_location_status_created_id ON stock_waste_events ("locationId", status, "createdAt" DESC, id DESC);
