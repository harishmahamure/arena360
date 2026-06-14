-- DESTRUCTIVE: wipes all application data. See DRAFT-0039.
-- Back up the database before running. Irreversible data loss.
-- Seeds bootstrap admin: superadmin / SuperAdmin@123 (bcrypt cost 12).

BEGIN;

DO $$
DECLARE
  tables_sql text;
BEGIN
  SELECT string_agg(format('%I.%I', schemaname, tablename), ', ' ORDER BY tablename)
  INTO tables_sql
  FROM pg_tables
  WHERE schemaname = 'public'
    AND tablename <> '_sqlx_migrations';

  IF tables_sql IS NOT NULL THEN
    EXECUTE 'TRUNCATE TABLE ' || tables_sql || ' RESTART IDENTITY CASCADE';
  END IF;
END $$;

INSERT INTO users (
  id,
  username,
  password_hash,
  "phoneNumber",
  "firstName",
  "lastName",
  role,
  "isActive",
  "createdAt",
  "updatedAt"
) VALUES (
  '00000000-0000-4000-8000-000000000001'::uuid,
  'superadmin',
  '$2b$12$sd10XNWf3pjTFTDJ5fdB1.SJSb04.WsSd9SkJGiwVNpI8m329Q8Iq',
  '0000000000',
  'Super',
  'Admin',
  'admin',
  true,
  NOW(),
  NOW()
);

UPDATE users
SET "createdBy" = id, "updatedBy" = id
WHERE username = 'superadmin';

COMMIT;
