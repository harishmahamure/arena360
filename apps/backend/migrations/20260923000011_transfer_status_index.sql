-- no-transaction
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_stock_transfer_status_created_id ON stock_transfer_requests (status, "createdAt" DESC, id DESC);
