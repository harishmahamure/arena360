DROP TABLE IF EXISTS stock_waste_lines CASCADE;
DROP TABLE IF EXISTS stock_waste_events CASCADE;
DROP TABLE IF EXISTS stock_transfer_lines CASCADE;
DROP TABLE IF EXISTS stock_transfer_requests CASCADE;
DROP TABLE IF EXISTS stock_receipt_lines CASCADE;
DROP TABLE IF EXISTS stock_receipts CASCADE;
DROP TABLE IF EXISTS stock_movements CASCADE;
DROP TABLE IF EXISTS location_stock CASCADE;
DROP TABLE IF EXISTS inventory_locations CASCADE;

ALTER TABLE products DROP COLUMN IF EXISTS "purchasePricePerBox";
ALTER TABLE products DROP COLUMN IF EXISTS "nightPrice";
ALTER TABLE products DROP COLUMN IF EXISTS "dayPrice";
ALTER TABLE products DROP COLUMN IF EXISTS "unitsPerPurchaseUnit";
ALTER TABLE products DROP COLUMN IF EXISTS "purchaseUnitId";
ALTER TABLE products DROP COLUMN IF EXISTS "unitId";

DELETE FROM configurations WHERE key IN (
  'inventory.default_warehouse_id',
  'inventory.default_store_id',
  'pos.default_sale_location_id'
);

DROP TYPE IF EXISTS stock_waste_reason;
DROP TYPE IF EXISTS stock_waste_status;
DROP TYPE IF EXISTS stock_transfer_status;
DROP TYPE IF EXISTS stock_movement_type;
DROP TYPE IF EXISTS inventory_location_kind;
