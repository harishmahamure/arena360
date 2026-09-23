-- no-transaction
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_sessions_active_start_id ON usage_sessions ("startTime" DESC, id DESC) WHERE "deletedAt" IS NULL;
