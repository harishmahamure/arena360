-- Complete tenant operational configuration and notification storage.
CREATE TABLE activity_log_new (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  kind TEXT NOT NULL CHECK (kind IN (
    'transaction_sale', 'plan_sale', 'credit_settlement', 'approval_requested',
    'approval_decided', 'session_started', 'session_ended', 'device_status_changed',
    'shift_clock_in', 'shift_clock_out', 'shift_handover', 'cash_register_opened',
    'cash_register_closed', 'cash_deposit_initiated', 'inventory_transfer_requested',
    'inventory_waste_recorded', 'kiosk_order_placed', 'kiosk_order_fulfilled', 'kiosk_order_cancelled'
  )),
  title TEXT NOT NULL CHECK (length(trim(title)) > 0),
  summary TEXT,
  payload TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(payload) = 1 AND json_type(payload) = 'object'),
  actor_user_id TEXT REFERENCES users(id),
  entity_type TEXT,
  entity_id TEXT,
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z')
) STRICT;



CREATE TABLE user_notifications_new (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'
    AND id = lower(id) AND id NOT GLOB '*[^0-9a-f-]*'
  ),
  activity_id TEXT NOT NULL REFERENCES activity_log_new(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  read_at TEXT CHECK (read_at IS NULL OR (length(read_at) = 27 AND substr(read_at, -1) = 'Z')),
  created_at TEXT NOT NULL CHECK (length(created_at) = 27 AND substr(created_at, -1) = 'Z'),
  UNIQUE (activity_id, user_id)
) STRICT;

INSERT INTO activity_log_new SELECT * FROM activity_log;
INSERT INTO user_notifications_new SELECT * FROM user_notifications;
DROP TABLE user_notifications;
DROP TABLE activity_log;
ALTER TABLE activity_log_new RENAME TO activity_log;
ALTER TABLE user_notifications_new RENAME TO user_notifications;
CREATE INDEX activity_log_created ON activity_log(created_at DESC,id DESC);
CREATE INDEX activity_log_kind_created ON activity_log(kind,created_at DESC,id DESC);
CREATE INDEX user_notifications_user_unread ON user_notifications(user_id,created_at DESC,id DESC) WHERE read_at IS NULL;
ALTER TABLE setting_overrides ADD COLUMN created_by TEXT;
ALTER TABLE setting_overrides ADD COLUMN updated_by TEXT;
CREATE TABLE access_modules (
 module TEXT PRIMARY KEY CHECK(length(trim(module)) BETWEEN 1 AND 80),
 enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN(0,1)),
 revision INTEGER NOT NULL DEFAULT 1 CHECK(revision>0)
) STRICT;
CREATE TABLE access_audit (
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 actor_id TEXT,
 action TEXT NOT NULL,
 target_id TEXT NOT NULL,
 before_value TEXT NOT NULL CHECK(json_valid(before_value)),
 after_value TEXT NOT NULL CHECK(json_valid(after_value)),
 created_at TEXT NOT NULL CHECK(length(created_at)=27 AND substr(created_at,-1)='Z')
) STRICT;
ALTER TABLE users ADD COLUMN access_revision INTEGER NOT NULL DEFAULT 0 CHECK(access_revision>=0);
-- Prevent a repeated sale/settlement completion from crediting the drawer twice.
DROP INDEX cash_register_entries_financial_source;
CREATE UNIQUE INDEX cash_register_entries_financial_source
 ON cash_register_entries(reference_type,reference_id,entry_type)
 WHERE reference_type IN('expense','cash_deposit','transaction','credit_settlement') AND reference_id IS NOT NULL;
ALTER TABLE venue_locations ADD COLUMN timezone TEXT NOT NULL DEFAULT 'UTC' CHECK(length(trim(timezone)) BETWEEN 1 AND 80);
ALTER TABLE venue_locations ADD COLUMN currency TEXT NOT NULL DEFAULT 'INR' CHECK(length(currency)=3 AND currency NOT GLOB '*[^A-Z]*');
