-- no-transaction
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_user_notifications_user_created_id_cover ON user_notifications ("userId", "createdAt" DESC, id DESC) INCLUDE ("readAt", "activityId");
