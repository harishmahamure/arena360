-- New sessions capture the selected plan profile and published deduction rules.
-- Existing sessions keep their original balance profile and are not repriced.
ALTER TABLE usage_sessions ADD COLUMN IF NOT EXISTS "deductionProfileSnapshot" jsonb;
