-- Rollback: restore player_plans, drop wallet tables
BEGIN;

ALTER TABLE player_plans_legacy RENAME TO player_plans;

ALTER TABLE usage_sessions DROP CONSTRAINT IF EXISTS fk_sessions_balance;
DROP INDEX IF EXISTS idx_sessions_balance;
ALTER TABLE usage_sessions DROP COLUMN IF EXISTS "balanceId";

DROP TABLE IF EXISTS player_plan_ledger;
DROP TABLE IF EXISTS player_plan_balances;

DROP TYPE IF EXISTS ledger_reason;
DROP TYPE IF EXISTS balance_status;
DROP TYPE IF EXISTS plan_kind;

COMMIT;
