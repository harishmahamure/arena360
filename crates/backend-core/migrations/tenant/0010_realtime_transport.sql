-- Tenant-local transport is separate from the canonical analytics outbox.
CREATE TABLE realtime_rooms (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE CHECK (length(trim(name)) BETWEEN 1 AND 120),
  description TEXT,
  created_by TEXT REFERENCES users(id),
  created_at TEXT NOT NULL
) STRICT;
CREATE TABLE realtime_room_members (
  room_id TEXT NOT NULL REFERENCES realtime_rooms(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  PRIMARY KEY (room_id, user_id)
) STRICT;
CREATE TABLE realtime_projection_cursor (
  singleton INTEGER PRIMARY KEY CHECK (singleton=1),
  sequence INTEGER NOT NULL CHECK (sequence>=0)
) STRICT;
-- Existing history has already been delivered before transport cutover.
INSERT INTO realtime_projection_cursor VALUES(1,(SELECT COALESCE(MAX(sequence),0) FROM outbox_events));
CREATE TABLE realtime_outbox (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source_sequence INTEGER NOT NULL CHECK (source_sequence>0),
  projection_index INTEGER NOT NULL CHECK (projection_index>=0),
  channel TEXT NOT NULL,
  event_type TEXT NOT NULL,
  payload TEXT NOT NULL CHECK (json_valid(payload)),
  audience_role TEXT,
  audience_user_id TEXT,
  audience_room_id TEXT,
  location_id TEXT,
  durable INTEGER NOT NULL CHECK (durable IN (0,1)),
  created_at TEXT NOT NULL,
  dispatched_at TEXT,
  UNIQUE (source_sequence, projection_index)
) STRICT;
CREATE INDEX realtime_pending ON realtime_outbox(id) WHERE dispatched_at IS NULL;
CREATE TABLE realtime_deliveries (
  outbox_id INTEGER NOT NULL REFERENCES realtime_outbox(id) ON DELETE CASCADE,
  subscriber_id TEXT NOT NULL,
  delivered_at TEXT NOT NULL,
  ack_at TEXT,
  PRIMARY KEY (outbox_id,subscriber_id)
) STRICT;
CREATE INDEX realtime_unacked ON realtime_deliveries(subscriber_id,outbox_id) WHERE ack_at IS NULL;
