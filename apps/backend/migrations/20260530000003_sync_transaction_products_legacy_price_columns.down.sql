BEGIN;

ALTER TABLE transaction_products DROP COLUMN IF EXISTS "subtotal";
ALTER TABLE transaction_products DROP COLUMN IF EXISTS "priceAtPurchase";

COMMIT;
