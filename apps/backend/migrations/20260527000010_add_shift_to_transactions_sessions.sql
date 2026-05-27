-- Migration: add_shift_to_transactions_sessions
-- shiftId on transactions and sessions
ALTER TABLE transactions ADD COLUMN IF NOT EXISTS "shiftId" UUID REFERENCES shifts(id);
ALTER TABLE usage_sessions ADD COLUMN IF NOT EXISTS "shiftId" UUID REFERENCES shifts(id);
CREATE INDEX IF NOT EXISTS idx_transactions_shift ON transactions("shiftId");
CREATE INDEX IF NOT EXISTS idx_sessions_shift ON usage_sessions("shiftId");

-- reconciliation columns on cash_registers
ALTER TABLE cash_registers ADD COLUMN IF NOT EXISTS "reconciledBy" UUID REFERENCES users(id);
ALTER TABLE cash_registers ADD COLUMN IF NOT EXISTS "reconciledAt" TIMESTAMPTZ;
ALTER TABLE cash_registers ADD COLUMN IF NOT EXISTS "reconciliationNotes" TEXT;
