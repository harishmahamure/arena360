-- Down migration: restore nullable balanceId and legacy playerPlanId column (no data backfill)

BEGIN;

ALTER TABLE usage_sessions
    ADD COLUMN IF NOT EXISTS "playerPlanId" UUID REFERENCES player_plans_legacy(id);

ALTER TABLE usage_sessions
    ALTER COLUMN "balanceId" DROP NOT NULL;

COMMIT;
