-- Display-only game media catalog (DRAFT-0022).
-- Distinct from the legacy inventory `games` table dropped in 20260528000001:
-- this stores branding assets (thumbnail/logo/video URLs) rendered by the kiosk.
CREATE TABLE IF NOT EXISTS games (
  id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name          VARCHAR(120) NOT NULL,
  "thumbnailUrl" TEXT,
  "logoUrl"     TEXT,
  "videoUrl"    TEXT,
  -- Optional hint matching a client-side allow-list entry (name or id); the
  -- launch source of truth stays client-side per ADR-0019.
  "launchRef"   VARCHAR(255),
  "isActive"    BOOLEAN NOT NULL DEFAULT TRUE,
  "sortOrder"   INTEGER NOT NULL DEFAULT 0,
  "createdBy"   UUID REFERENCES users(id),
  "updatedBy"   UUID REFERENCES users(id),
  "createdAt"   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt"   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "deletedAt"   TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_games_active_sort
  ON games ("isActive", "sortOrder")
  WHERE "deletedAt" IS NULL;

-- Retire the one-time registration-code pairing (DRAFT-0023): admin login on the
-- device now authorizes provisioning via POST /devices/provision.
DROP INDEX IF EXISTS uniq_devices_registration_code_pending;

ALTER TABLE devices
  DROP COLUMN IF EXISTS "registrationCode",
  DROP COLUMN IF EXISTS "registrationCodeExpiresAt",
  DROP COLUMN IF EXISTS "registrationCodeUsedAt";
