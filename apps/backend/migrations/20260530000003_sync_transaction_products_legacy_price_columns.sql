-- The live transaction_products table comes from the TypeORM baseline and carries
-- legacy NOT NULL columns ("priceAtPurchase", "subtotal") with no defaults that the
-- Rust line-item INSERT must populate. Ensure these columns also exist on fresh
-- databases (created by 20260528000002) so the shared INSERT works in every
-- environment, and backfill any existing NULLs from the data we do have.
BEGIN;

ALTER TABLE transaction_products ADD COLUMN IF NOT EXISTS "priceAtPurchase" NUMERIC(10,2);
ALTER TABLE transaction_products ADD COLUMN IF NOT EXISTS "subtotal" NUMERIC(10,2);

UPDATE transaction_products
   SET "priceAtPurchase" = COALESCE("priceAtPurchase", "unitPrice"),
       "subtotal" = COALESCE("subtotal", "unitPrice" * quantity)
 WHERE "priceAtPurchase" IS NULL
    OR "subtotal" IS NULL;

COMMIT;
