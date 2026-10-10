-- Preserve the venue at activity creation so moving a device cannot change history's scope.
ALTER TABLE activity_log ADD COLUMN location_id TEXT REFERENCES venue_locations(id);
UPDATE activity_log SET location_id=(SELECT s.location_id FROM usage_sessions s WHERE s.id=activity_log.entity_id)
 WHERE entity_type='session';
UPDATE activity_log SET location_id=(SELECT o.location_id FROM outbox_events o JOIN venue_locations l ON l.id=o.location_id WHERE o.aggregate_id=activity_log.entity_id AND o.event_type='kiosk_order.placed' ORDER BY o.sequence LIMIT 1)
 WHERE kind IN('kiosk_order_placed','kiosk_order_fulfilled','kiosk_order_cancelled');
CREATE INDEX activity_log_location_created ON activity_log(location_id,created_at DESC,id DESC);
-- Keep undelivered and replayable notification events subject to the same historical venue.
UPDATE outbox_events SET location_id=(SELECT al.location_id FROM activity_log al WHERE al.id=json_extract(outbox_events.payload,'$.activityId'))
 WHERE event_type='notification.created' AND location_id IS NULL;
UPDATE realtime_outbox SET location_id=(SELECT al.location_id FROM activity_log al WHERE al.id=json_extract(realtime_outbox.payload,'$.activityId'))
 WHERE event_type='notification.created' AND location_id IS NULL;
