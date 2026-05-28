BEGIN;

DROP TRIGGER IF EXISTS trg_realtime_outbox_notify ON realtime_outbox;
DROP FUNCTION IF EXISTS realtime_outbox_notify();
DROP TABLE IF EXISTS realtime_room_members;
DROP TABLE IF EXISTS realtime_rooms;
DROP TABLE IF EXISTS realtime_deliveries;
DROP TABLE IF EXISTS realtime_outbox;

COMMIT;
