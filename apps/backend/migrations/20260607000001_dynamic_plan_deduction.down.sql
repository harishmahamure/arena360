-- Down: dynamic plan wallet deduction profiles

BEGIN;

ALTER TABLE player_plan_balances DROP COLUMN IF EXISTS "deductionProfile";
ALTER TABLE plans DROP COLUMN IF EXISTS "deductionProfile";
ALTER TABLE plans DROP COLUMN IF EXISTS "dynamicDeductionEnabled";

COMMIT;
