-- Migration: dynamic plan wallet deduction profiles
-- Created: 2026-06-07
-- ADR: DRAFT-0033-dynamic-plan-deduction

BEGIN;

ALTER TABLE plans
    ADD COLUMN IF NOT EXISTS "dynamicDeductionEnabled" BOOLEAN NOT NULL DEFAULT false;

ALTER TABLE plans
    ADD COLUMN IF NOT EXISTS "deductionProfile" JSONB;

ALTER TABLE player_plan_balances
    ADD COLUMN IF NOT EXISTS "deductionProfile" JSONB;

COMMIT;
