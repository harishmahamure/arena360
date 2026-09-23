-- no-transaction
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_stock_transfer_from_created_id ON stock_transfer_requests ("fromLocationId", "createdAt" DESC, id DESC);
