-- Keep persisted notifications limited to kiosk orders placed for active staff.
CREATE INDEX IF NOT EXISTS idx_user_notifications_created_at
  ON user_notifications ("createdAt");

CREATE INDEX IF NOT EXISTS idx_realtime_outbox_created_at
  ON realtime_outbox (created_at);

DELETE FROM user_notifications un
USING activity_log al, users u
WHERE un."activityId" = al.id
  AND un."userId" = u.id
  AND (
    al.kind::text <> 'kiosk_order_placed'
    OR u.role <> 'staff'
    OR u."isActive" = false
    OR u."deletedAt" IS NOT NULL
    OR un."createdAt" < NOW() - INTERVAL '7 days'
  );

DELETE FROM realtime_outbox o
WHERE o.event_type = 'notification.created'
  AND (
    o.payload->>'kind' IS DISTINCT FROM 'kiosk_order_placed'
    OR NOT EXISTS (
      SELECT 1 FROM users u
      WHERE u.id = o.audience_user_id
        AND u.role = 'staff'
        AND u."isActive" = true
        AND u."deletedAt" IS NULL
    )
  );

DELETE FROM realtime_outbox
WHERE created_at < NOW() - INTERVAL '7 days';
