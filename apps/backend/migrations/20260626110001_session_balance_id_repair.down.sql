-- Down migration: balanceId repair is data-only; cannot restore soft-deleted rows or force-ended sessions.
-- No-op.

BEGIN;
COMMIT;
