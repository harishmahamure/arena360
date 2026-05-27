CREATE TABLE IF NOT EXISTS cash_registers (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "shiftId" UUID NOT NULL REFERENCES shifts(id),
  "openedBy" UUID NOT NULL REFERENCES users(id),
  "closedBy" UUID REFERENCES users(id),
  "openingBalance" DECIMAL(19,4) NOT NULL DEFAULT 0,
  "openingDenominations" JSONB,
  "closingBalance" DECIMAL(19,4),
  "closingDenominations" JSONB,
  "expectedClosing" DECIMAL(19,4),
  variance DECIMAL(19,4),
  status VARCHAR(20) NOT NULL DEFAULT 'open',
  notes TEXT,
  "createdBy" UUID REFERENCES users(id),
  "updatedBy" UUID REFERENCES users(id),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS cash_register_entries (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "cashRegisterId" UUID NOT NULL REFERENCES cash_registers(id),
  "entryType" VARCHAR(20) NOT NULL,
  amount DECIMAL(19,4) NOT NULL,
  reason TEXT,
  "referenceId" UUID,
  "referenceType" VARCHAR(50),
  "createdBy" UUID REFERENCES users(id),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_cash_registers_shift ON cash_registers("shiftId");
CREATE INDEX idx_cash_registers_status ON cash_registers(status);
CREATE INDEX idx_cash_register_entries_register ON cash_register_entries("cashRegisterId");
