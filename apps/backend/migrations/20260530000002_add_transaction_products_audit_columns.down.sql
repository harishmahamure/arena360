BEGIN;

ALTER TABLE transaction_products DROP COLUMN IF EXISTS "updatedAt";
ALTER TABLE transaction_products DROP COLUMN IF EXISTS "createdAt";
ALTER TABLE transaction_products DROP COLUMN IF EXISTS "updatedBy";
ALTER TABLE transaction_products DROP COLUMN IF EXISTS "createdBy";

COMMIT;
