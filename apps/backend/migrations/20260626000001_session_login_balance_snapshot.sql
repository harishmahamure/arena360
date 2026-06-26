-- Migration: session login balance snapshot (ADR-0050)
-- Created: 2026-06-26

BEGIN;

ALTER TABLE usage_sessions
  ADD COLUMN IF NOT EXISTS "walletMinutesAtStart" INTEGER,
  ADD COLUMN IF NOT EXISTS "sourcePlanIdAtStart" UUID REFERENCES plans(id);

COMMIT;
