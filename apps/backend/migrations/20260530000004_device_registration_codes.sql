-- Device registration codes for kiosk pairing (ADR-0017)
ALTER TABLE devices
  ADD COLUMN IF NOT EXISTS "registrationCode" VARCHAR(32),
  ADD COLUMN IF NOT EXISTS "registrationCodeExpiresAt" TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS "registrationCodeUsedAt" TIMESTAMPTZ;

CREATE UNIQUE INDEX IF NOT EXISTS uniq_devices_registration_code_pending
  ON devices ("registrationCode")
  WHERE "registrationCode" IS NOT NULL
    AND "registrationCodeUsedAt" IS NULL
    AND "deletedAt" IS NULL;
