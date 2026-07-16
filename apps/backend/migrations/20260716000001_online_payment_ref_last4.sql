-- Migration: add onlinePaymentRefLast4 for UPI receipt last-4 digits
-- Created: 2026-07-16

BEGIN;

ALTER TABLE transactions
  ADD COLUMN IF NOT EXISTS "onlinePaymentRefLast4" CHAR(4);

ALTER TABLE credit_settlements
  ADD COLUMN IF NOT EXISTS "onlinePaymentRefLast4" CHAR(4);

COMMIT;
