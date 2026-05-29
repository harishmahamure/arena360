-- Enum values cannot run inside a transaction block
ALTER TYPE transactions_paymentmethod_enum ADD VALUE IF NOT EXISTS 'credit';
ALTER TYPE transactions_paymentstatus_enum ADD VALUE IF NOT EXISTS 'credit';

BEGIN;

ALTER TABLE users
  ADD COLUMN IF NOT EXISTS "creditLimit" DECIMAL(19,4) NOT NULL DEFAULT 0;

ALTER TABLE transactions
  ADD COLUMN IF NOT EXISTS "paidAmount" DECIMAL(19,4) NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS credit_settlements (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "playerId" UUID NOT NULL REFERENCES users(id),
  "shiftId" UUID NOT NULL REFERENCES shifts(id),
  "settledBy" UUID NOT NULL REFERENCES users(id),
  amount DECIMAL(19,4) NOT NULL,
  "paymentMethod" VARCHAR(20) NOT NULL,
  "cashAmount" DECIMAL(19,4),
  "onlineAmount" DECIMAL(19,4),
  notes TEXT,
  "settledAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "createdBy" UUID REFERENCES users(id),
  "updatedBy" UUID REFERENCES users(id),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "deletedAt" TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS credit_settlement_items (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "settlementId" UUID NOT NULL REFERENCES credit_settlements(id) ON DELETE CASCADE,
  "transactionId" UUID NOT NULL REFERENCES transactions(id),
  "amountApplied" DECIMAL(19,4) NOT NULL CHECK ("amountApplied" > 0),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE ("settlementId", "transactionId")
);

CREATE INDEX IF NOT EXISTS idx_credit_settlements_player ON credit_settlements("playerId");
CREATE INDEX IF NOT EXISTS idx_credit_settlements_shift ON credit_settlements("shiftId");
CREATE INDEX IF NOT EXISTS idx_credit_settlement_items_settlement ON credit_settlement_items("settlementId");
CREATE INDEX IF NOT EXISTS idx_credit_settlement_items_transaction ON credit_settlement_items("transactionId");

COMMIT;
