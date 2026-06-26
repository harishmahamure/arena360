-- Revert kiosk player ordering

DROP INDEX IF EXISTS uniq_kiosk_orders_session_open;
DROP INDEX IF EXISTS idx_kiosk_order_items_order;
DROP INDEX IF EXISTS idx_kiosk_orders_device;
DROP INDEX IF EXISTS idx_kiosk_orders_status;
DROP INDEX IF EXISTS idx_kiosk_orders_session;

DROP TABLE IF EXISTS kiosk_order_items;
DROP TABLE IF EXISTS kiosk_orders;
DROP TYPE IF EXISTS kiosk_order_status;

-- Note: PostgreSQL does not support removing enum values from activity_kind;
-- kiosk_order_* values remain in the enum after down migration.
