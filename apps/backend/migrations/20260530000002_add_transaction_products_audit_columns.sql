-- Bring transaction_products in line with ADR-0010 / migration 20260528000002.
-- The live table originated from a TypeORM baseline, so the CREATE TABLE IF NOT
-- EXISTS in 20260528000002 was a no-op and the audit columns were never applied
-- (same situation that required patching "unitPrice" in 20260529000002).
BEGIN;

ALTER TABLE transaction_products
  ADD COLUMN IF NOT EXISTS "createdBy" UUID REFERENCES users(id);

ALTER TABLE transaction_products
  ADD COLUMN IF NOT EXISTS "updatedBy" UUID REFERENCES users(id);

ALTER TABLE transaction_products
  ADD COLUMN IF NOT EXISTS "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW();

ALTER TABLE transaction_products
  ADD COLUMN IF NOT EXISTS "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW();

COMMIT;
