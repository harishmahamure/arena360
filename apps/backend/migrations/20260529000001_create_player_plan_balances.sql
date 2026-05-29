-- Migration: create player_plan_balances wallet model
-- Created: 2026-05-29
-- ADR: DRAFT-0014-player-plan-balances-wallet

BEGIN;

-- 1. New enum types
CREATE TYPE plan_kind AS ENUM ('time', 'happy_hours');
CREATE TYPE balance_status AS ENUM ('active', 'expired', 'exhausted', 'cancelled');
CREATE TYPE ledger_reason AS ENUM ('purchase', 'recharge', 'session_usage', 'expiry', 'adjustment', 'migration');

-- 2. Balances table (the wallet)
CREATE TABLE player_plan_balances (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    "playerId"       UUID NOT NULL REFERENCES users(id),
    "deviceType"     plans_devicetype_enum,
    "deviceSubType"  plans_devicesubtype_enum,
    kind             plan_kind NOT NULL,
    "remainingMinutes" INTEGER NOT NULL DEFAULT 0,
    "expiryDate"     TIMESTAMPTZ NOT NULL,
    "windowStart"    TIME,
    "windowEnd"      TIME,
    status           balance_status NOT NULL DEFAULT 'active',
    "sourcePlanId"   UUID REFERENCES plans(id),
    "createdBy"      UUID REFERENCES users(id),
    "updatedBy"      UUID REFERENCES users(id),
    "createdAt"      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    "updatedAt"      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    "deletedAt"      TIMESTAMPTZ
);

CREATE INDEX idx_balances_player_status
    ON player_plan_balances ("playerId", status)
    WHERE "deletedAt" IS NULL;

CREATE INDEX idx_balances_player_scope
    ON player_plan_balances ("playerId", "deviceType", "deviceSubType", kind)
    WHERE "deletedAt" IS NULL;

CREATE UNIQUE INDEX uniq_balances_active_scope
    ON player_plan_balances ("playerId", "deviceType", "deviceSubType", kind)
    WHERE status = 'active' AND "deletedAt" IS NULL;

-- 3. Ledger table (append-only audit trail)
CREATE TABLE player_plan_ledger (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    "balanceId"      UUID NOT NULL REFERENCES player_plan_balances(id),
    "playerId"       UUID NOT NULL REFERENCES users(id),
    "deltaMinutes"   INTEGER NOT NULL,
    reason           ledger_reason NOT NULL,
    "transactionId"  UUID REFERENCES transactions(id),
    "sessionId"      UUID,
    "balanceAfter"   INTEGER NOT NULL,
    "expiryAfter"    TIMESTAMPTZ NOT NULL,
    "createdAt"      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    "createdBy"      UUID REFERENCES users(id)
);

CREATE INDEX idx_ledger_balance ON player_plan_ledger ("balanceId", "createdAt" DESC);
CREATE INDEX idx_ledger_player  ON player_plan_ledger ("playerId", "createdAt" DESC);

-- 4. Backfill balances from active player_plans
INSERT INTO player_plan_balances (
    "playerId", "deviceType", "deviceSubType", kind,
    "remainingMinutes", "expiryDate",
    "windowStart", "windowEnd",
    status, "sourcePlanId",
    "createdBy", "updatedBy"
)
SELECT
    pp."playerId",
    p."deviceType",
    p."deviceSubType",
    CASE WHEN p."planType"::text = 'weekend_special' THEN 'happy_hours'::plan_kind
         ELSE 'time'::plan_kind
    END AS kind,
    COALESCE(SUM(pp."remainingTimeCredits"), 0) AS "remainingMinutes",
    MAX(pp."expiryDate") AS "expiryDate",
    MAX(p."timeWindowStart") AS "windowStart",
    MAX(p."timeWindowEnd")   AS "windowEnd",
    'active'::balance_status,
    (ARRAY_AGG(p.id ORDER BY pp."expiryDate" DESC))[1],
    (ARRAY_AGG(pp."createdBy" ORDER BY pp."expiryDate" DESC))[1],
    (ARRAY_AGG(pp."updatedBy" ORDER BY pp."expiryDate" DESC))[1]
FROM player_plans pp
JOIN plans p ON p.id = pp."planId" AND p."deletedAt" IS NULL
WHERE pp.status::text = 'active'
  AND pp."deletedAt" IS NULL
  AND pp."expiryDate" > NOW()
GROUP BY
    pp."playerId",
    p."deviceType",
    p."deviceSubType",
    CASE WHEN p."planType"::text = 'weekend_special' THEN 'happy_hours'::plan_kind
         ELSE 'time'::plan_kind
    END;

-- 5. Seed migration ledger rows (opening balance for each backfilled row)
INSERT INTO player_plan_ledger (
    "balanceId", "playerId", "deltaMinutes", reason,
    "balanceAfter", "expiryAfter", "createdBy"
)
SELECT
    b.id, b."playerId", b."remainingMinutes", 'migration'::ledger_reason,
    b."remainingMinutes", b."expiryDate", b."createdBy"
FROM player_plan_balances b;

-- 6. Add balanceId to usage_sessions and backfill
ALTER TABLE usage_sessions ADD COLUMN IF NOT EXISTS "balanceId" UUID;

UPDATE usage_sessions s
SET "balanceId" = (
    SELECT b.id
    FROM player_plan_balances b
    JOIN player_plans pp ON pp."playerId" = b."playerId"
    JOIN plans p ON p.id = pp."planId"
    WHERE pp.id = s."playerPlanId"
      AND COALESCE(b."deviceType"::text, '__null__') = COALESCE(p."deviceType"::text, '__null__')
      AND COALESCE(b."deviceSubType"::text, '__null__') = COALESCE(p."deviceSubType"::text, '__null__')
      AND b.kind = CASE WHEN p."planType"::text = 'weekend_special' THEN 'happy_hours'::plan_kind
                        ELSE 'time'::plan_kind END
    LIMIT 1
)
WHERE s."playerPlanId" IS NOT NULL;

ALTER TABLE usage_sessions
    ADD CONSTRAINT fk_sessions_balance
    FOREIGN KEY ("balanceId") REFERENCES player_plan_balances(id);

CREATE INDEX idx_sessions_balance ON usage_sessions ("balanceId")
    WHERE "deletedAt" IS NULL;

-- 7. Rename legacy table (keep for rollback)
ALTER TABLE player_plans RENAME TO player_plans_legacy;

COMMIT;
