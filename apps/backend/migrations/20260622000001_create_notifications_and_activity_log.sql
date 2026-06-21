-- Persisted notifications and activity log (ADR-0045)

CREATE TYPE activity_kind AS ENUM (
  'transaction_sale',
  'plan_sale',
  'credit_settlement',
  'approval_requested',
  'approval_decided',
  'session_started',
  'session_ended',
  'device_status_changed',
  'shift_clock_in',
  'shift_clock_out',
  'shift_handover',
  'cash_register_opened',
  'cash_register_closed',
  'cash_deposit_initiated',
  'inventory_transfer_requested',
  'inventory_waste_recorded'
);

CREATE TABLE activity_log (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  kind activity_kind NOT NULL,
  title TEXT NOT NULL,
  summary TEXT,
  payload JSONB NOT NULL DEFAULT '{}',
  "actorUserId" UUID REFERENCES users(id),
  "entityType" TEXT,
  "entityId" UUID,
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE user_notifications (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "activityId" UUID NOT NULL REFERENCES activity_log(id) ON DELETE CASCADE,
  "userId" UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  "readAt" TIMESTAMPTZ,
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE ("activityId", "userId")
);

CREATE INDEX idx_user_notifications_user_unread
  ON user_notifications ("userId", "createdAt" DESC)
  WHERE "readAt" IS NULL;

CREATE INDEX idx_activity_log_created
  ON activity_log ("createdAt" DESC);

CREATE INDEX idx_activity_log_kind
  ON activity_log (kind);
