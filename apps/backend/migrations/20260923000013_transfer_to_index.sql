-- no-transaction
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_stock_transfer_to_created_id ON stock_transfer_requests ("toLocationId", "createdAt" DESC, id DESC);
