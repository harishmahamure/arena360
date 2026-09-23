-- no-transaction
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_activity_log_actor_created_id ON activity_log ("actorUserId", "createdAt" DESC, id DESC);
