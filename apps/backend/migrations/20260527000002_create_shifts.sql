CREATE TABLE IF NOT EXISTS shifts (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "userId" UUID NOT NULL REFERENCES users(id),
  "clockIn" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "clockOut" TIMESTAMPTZ,
  notes TEXT,
  status VARCHAR(20) NOT NULL DEFAULT 'active',
  "createdBy" UUID REFERENCES users(id),
  "updatedBy" UUID REFERENCES users(id),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_shifts_user_id ON shifts("userId");
CREATE INDEX idx_shifts_status ON shifts(status);
CREATE INDEX idx_shifts_clock_in ON shifts("clockIn" DESC);
