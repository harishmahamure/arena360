-- Migration: enforce usage_sessions.balanceId NOT NULL, drop legacy playerPlanId (ADR-0014)
-- Created: 2026-06-26
-- Requires 20260626110001_session_balance_id_repair.sql to have zero NULL balanceId rows.

BEGIN;

ALTER TABLE usage_sessions
    DROP CONSTRAINT IF EXISTS usage_sessions_playerPlanId_fkey;

ALTER TABLE usage_sessions
    DROP CONSTRAINT IF EXISTS "usage_sessions_playerPlanId_fkey";

ALTER TABLE usage_sessions
    ALTER COLUMN "balanceId" SET NOT NULL;

ALTER TABLE usage_sessions
    DROP COLUMN IF EXISTS "playerPlanId";

COMMIT;
