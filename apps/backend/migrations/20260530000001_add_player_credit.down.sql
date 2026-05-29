BEGIN;

DROP TABLE IF EXISTS credit_settlement_items;
DROP TABLE IF EXISTS credit_settlements;

ALTER TABLE transactions DROP COLUMN IF EXISTS "paidAmount";
ALTER TABLE users DROP COLUMN IF EXISTS "creditLimit";

COMMIT;

-- Note: 'credit' enum values on transactions_paymentmethod_enum and
-- transactions_paymentstatus_enum cannot be removed in PostgreSQL.
