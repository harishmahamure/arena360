-- Add Happy Hours scheduling columns to plans
ALTER TABLE plans ADD COLUMN IF NOT EXISTS "allowedDays" JSONB;
ALTER TABLE plans ADD COLUMN IF NOT EXISTS "allowedMonths" JSONB;

-- Mirror on player_plan_balances so active balances carry the schedule
ALTER TABLE player_plan_balances ADD COLUMN IF NOT EXISTS "allowedDays" JSONB;
ALTER TABLE player_plan_balances ADD COLUMN IF NOT EXISTS "allowedMonths" JSONB;
