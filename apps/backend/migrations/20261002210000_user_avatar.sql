-- DRAFT-0042 (approved 2026-10-02): profile photos for panel users.
ALTER TABLE users ADD COLUMN IF NOT EXISTS "avatarUrl" TEXT;
