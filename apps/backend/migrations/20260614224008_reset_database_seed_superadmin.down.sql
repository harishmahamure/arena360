-- Irreversible: the up migration permanently deleted all application data.
-- This down script only removes the seeded superadmin row; it does not restore
-- truncated tables.

BEGIN;

UPDATE users
SET "createdBy" = NULL, "updatedBy" = NULL
WHERE username = 'superadmin';

UPDATE users
SET "createdBy" = NULL, "updatedBy" = NULL
WHERE "createdBy" = (
  SELECT id FROM users WHERE username = 'superadmin' LIMIT 1
)
   OR "updatedBy" = (
  SELECT id FROM users WHERE username = 'superadmin' LIMIT 1
);

DELETE FROM users WHERE username = 'superadmin';

COMMIT;
