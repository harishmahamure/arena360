-- Down: session login balance snapshot (ADR-0050)

BEGIN;

ALTER TABLE usage_sessions
  DROP COLUMN IF EXISTS "walletMinutesAtStart",
  DROP COLUMN IF EXISTS "sourcePlanIdAtStart";

COMMIT;
