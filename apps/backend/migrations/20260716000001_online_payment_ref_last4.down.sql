-- Down: remove onlinePaymentRefLast4

BEGIN;

ALTER TABLE credit_settlements
  DROP COLUMN IF EXISTS "onlinePaymentRefLast4";

ALTER TABLE transactions
  DROP COLUMN IF EXISTS "onlinePaymentRefLast4";

COMMIT;
