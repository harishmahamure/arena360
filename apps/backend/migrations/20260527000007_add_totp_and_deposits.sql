-- Add TOTP columns for staff handover validation
ALTER TABLE users
  ADD COLUMN IF NOT EXISTS "totpSecret" VARCHAR(64),
  ADD COLUMN IF NOT EXISTS "totpEnabled" BOOLEAN NOT NULL DEFAULT false;

-- Cash deposits: staff initiates withdrawal, admin approves with destination
CREATE TABLE IF NOT EXISTS cash_deposits (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "cashRegisterId" UUID NOT NULL REFERENCES cash_registers(id),
  "shiftId" UUID NOT NULL REFERENCES shifts(id),
  "initiatedBy" UUID NOT NULL REFERENCES users(id),
  "approvedBy" UUID REFERENCES users(id),
  amount DECIMAL(19,4) NOT NULL,
  denominations JSONB NOT NULL,
  "depositType" VARCHAR(20),
  status VARCHAR(20) NOT NULL DEFAULT 'pending',
  "approvedAt" TIMESTAMPTZ,
  "rejectionReason" TEXT,
  notes TEXT,
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_cash_deposits_status ON cash_deposits(status);
CREATE INDEX idx_cash_deposits_shift ON cash_deposits("shiftId");
CREATE INDEX idx_cash_deposits_register ON cash_deposits("cashRegisterId");
