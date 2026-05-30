-- Restore registration-code pairing columns (DRAFT-0023 revert).
ALTER TABLE devices
  ADD COLUMN IF NOT EXISTS "registrationCode" VARCHAR(32),
  ADD COLUMN IF NOT EXISTS "registrationCodeExpiresAt" TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS "registrationCodeUsedAt" TIMESTAMPTZ;

CREATE UNIQUE INDEX IF NOT EXISTS uniq_devices_registration_code_pending
  ON devices ("registrationCode")
  WHERE "registrationCode" IS NOT NULL
    AND "registrationCodeUsedAt" IS NULL
    AND "deletedAt" IS NULL;

DROP INDEX IF EXISTS idx_games_active_sort;
DROP TABLE IF EXISTS games;
