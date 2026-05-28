-- Migration: Create realtime outbox, deliveries, rooms, room_members
-- Created: 2026-05-28
-- ADR: DRAFT-0013-realtime-websocket-channel

BEGIN;

-- Durable event outbox: services INSERT here inside the same SQL TX as business writes.
CREATE TABLE realtime_outbox (
  id BIGSERIAL PRIMARY KEY,
  channel TEXT NOT NULL,
  event_type TEXT NOT NULL,
  payload JSONB NOT NULL DEFAULT '{}',
  audience_role TEXT,
  audience_user_id UUID,
  audience_room_id UUID,
  durable BOOLEAN NOT NULL DEFAULT TRUE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_outbox_channel_created ON realtime_outbox(channel, created_at);

-- Per-subscriber delivery tracking for at-least-once semantics.
CREATE TABLE realtime_deliveries (
  outbox_id BIGINT NOT NULL REFERENCES realtime_outbox(id) ON DELETE CASCADE,
  subscriber_id UUID NOT NULL,
  delivered_at TIMESTAMPTZ,
  ack_at TIMESTAMPTZ,
  PRIMARY KEY (outbox_id, subscriber_id)
);
CREATE INDEX idx_deliveries_pending ON realtime_deliveries(subscriber_id, ack_at)
  WHERE ack_at IS NULL;

-- Admin-managed named rooms.
CREATE TABLE realtime_rooms (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name TEXT NOT NULL UNIQUE,
  description TEXT,
  created_by UUID REFERENCES users(id),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE realtime_room_members (
  room_id UUID NOT NULL REFERENCES realtime_rooms(id) ON DELETE CASCADE,
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (room_id, user_id)
);
CREATE INDEX idx_room_members_user ON realtime_room_members(user_id);

-- Trigger: notify the dispatcher on every outbox insert so it wakes without polling.
CREATE OR REPLACE FUNCTION realtime_outbox_notify() RETURNS TRIGGER AS $$
BEGIN
  PERFORM pg_notify('realtime_outbox_new', NEW.id::text);
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_realtime_outbox_notify
AFTER INSERT ON realtime_outbox
FOR EACH ROW EXECUTE FUNCTION realtime_outbox_notify();

COMMIT;
