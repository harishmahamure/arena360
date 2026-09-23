DROP INDEX IF EXISTS idx_expenses_source_unique;
DROP INDEX IF EXISTS idx_stock_receipts_po_invoice;
ALTER TABLE expenses DROP COLUMN IF EXISTS "sourceId", DROP COLUMN IF EXISTS "sourceType";
ALTER TABLE stock_receipt_lines
  DROP COLUMN IF EXISTS "lineTotal",
  DROP COLUMN IF EXISTS "taxRate",
  DROP COLUMN IF EXISTS "boxCostSnapshot",
  DROP COLUMN IF EXISTS "rejectedBoxQuantity",
  DROP COLUMN IF EXISTS "acceptedBoxQuantity",
  DROP COLUMN IF EXISTS "purchaseOrderLineId";
ALTER TABLE stock_receipts
  DROP COLUMN IF EXISTS "exceptionalReason",
  DROP COLUMN IF EXISTS total,
  DROP COLUMN IF EXISTS tax,
  DROP COLUMN IF EXISTS subtotal,
  DROP COLUMN IF EXISTS "receiptDate",
  DROP COLUMN IF EXISTS "paymentAccount",
  DROP COLUMN IF EXISTS "paymentMethod",
  DROP COLUMN IF EXISTS "invoiceReference",
  DROP COLUMN IF EXISTS "purchaseOrderId";
DROP TABLE IF EXISTS inventory_reorder_rules;
DROP TABLE IF EXISTS purchase_order_lines;
DROP TABLE IF EXISTS purchase_orders;
DROP SEQUENCE IF EXISTS purchase_order_number_seq;
DROP TYPE IF EXISTS purchase_order_status;
