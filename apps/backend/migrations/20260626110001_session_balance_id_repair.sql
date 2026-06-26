-- Migration: repair usage_sessions.balanceId orphans (ADR-0014 completion)
-- Created: 2026-06-26
-- Re-backfills balanceId from legacy playerPlanId, then closes or soft-deletes unrecoverable rows.

BEGIN;

-- 1. Re-backfill balanceId from playerPlanId (deterministic: newest matching balance)
UPDATE usage_sessions s
SET "balanceId" = (
    SELECT b.id
    FROM player_plan_balances b
    JOIN player_plans_legacy pp ON pp."playerId" = b."playerId"
    JOIN plans p ON p.id = pp."planId"
    WHERE pp.id = s."playerPlanId"
      AND COALESCE(b."deviceType"::text, '__null__') = COALESCE(p."deviceType"::text, '__null__')
      AND COALESCE(b."deviceSubType"::text, '__null__') = COALESCE(p."deviceSubType"::text, '__null__')
      AND b.kind = CASE
          WHEN p."planType"::text = 'weekend_special' THEN 'happy_hours'::plan_kind
          ELSE 'time'::plan_kind
      END
      AND b."deletedAt" IS NULL
    ORDER BY b."createdAt" DESC
    LIMIT 1
)
WHERE s."balanceId" IS NULL
  AND s."playerPlanId" IS NOT NULL
  AND s."deletedAt" IS NULL;

-- 2. Force-end open sessions that still have no wallet (cannot charge safely)
UPDATE usage_sessions
SET "endTime" = NOW(),
    "durationMinutes" = GREATEST(
        0,
        FLOOR(EXTRACT(EPOCH FROM (NOW() - "startTime")) / 60)::INTEGER
    ),
    "timeCreditsConsumed" = COALESCE("timeCreditsConsumed", 0),
    "updatedAt" = NOW()
WHERE "deletedAt" IS NULL
  AND "balanceId" IS NULL
  AND "endTime" IS NULL;

-- 3. Soft-delete ended historical rows that cannot be linked to a wallet
UPDATE usage_sessions
SET "deletedAt" = NOW(),
    "updatedAt" = NOW()
WHERE "deletedAt" IS NULL
  AND "balanceId" IS NULL
  AND "endTime" IS NOT NULL;

COMMIT;
