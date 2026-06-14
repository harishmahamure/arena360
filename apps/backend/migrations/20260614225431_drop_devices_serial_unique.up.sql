-- DRAFT-0040: allow duplicate devices."serialNumber" (OEM placeholder serials).
-- Drop TypeORM-era unique constraint/index names vary by environment.

DO $$
DECLARE r record;
BEGIN
  -- Drop unique constraints first (removes their backing indexes).
  FOR r IN
    SELECT conname FROM pg_constraint
    WHERE conrelid = 'public.devices'::regclass
      AND contype = 'u'
      AND pg_get_constraintdef(oid) ILIKE '%serialNumber%'
  LOOP
    EXECUTE format('ALTER TABLE devices DROP CONSTRAINT %I', r.conname);
  END LOOP;

  -- Drop any remaining standalone unique indexes on serialNumber.
  FOR r IN
    SELECT indexname FROM pg_indexes
    WHERE schemaname = 'public'
      AND tablename = 'devices'
      AND indexdef ILIKE '%serialNumber%'
      AND indexdef ILIKE '%UNIQUE%'
  LOOP
    EXECUTE format('DROP INDEX IF EXISTS %I', r.indexname);
  END LOOP;
END $$;
