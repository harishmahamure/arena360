-- Down migration: enum values cannot be removed in PostgreSQL without recreating types.
-- No-op; staff_allowance rows should be cancelled before rollback if ever needed.

BEGIN;
COMMIT;
