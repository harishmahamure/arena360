-- no-transaction
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_activity_log_kind_created_id ON activity_log (kind, "createdAt" DESC, id DESC);
