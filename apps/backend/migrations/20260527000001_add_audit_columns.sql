-- Add created_by and updated_by audit columns to all existing tables
-- These are nullable to allow backfilling; new records should always set them

ALTER TABLE users ADD COLUMN IF NOT EXISTS "createdBy" UUID REFERENCES users(id);
ALTER TABLE users ADD COLUMN IF NOT EXISTS "updatedBy" UUID REFERENCES users(id);

ALTER TABLE devices ADD COLUMN IF NOT EXISTS "createdBy" UUID REFERENCES users(id);
ALTER TABLE devices ADD COLUMN IF NOT EXISTS "updatedBy" UUID REFERENCES users(id);

ALTER TABLE games ADD COLUMN IF NOT EXISTS "createdBy" UUID REFERENCES users(id);
ALTER TABLE games ADD COLUMN IF NOT EXISTS "updatedBy" UUID REFERENCES users(id);

ALTER TABLE plans ADD COLUMN IF NOT EXISTS "createdBy" UUID REFERENCES users(id);
ALTER TABLE plans ADD COLUMN IF NOT EXISTS "updatedBy" UUID REFERENCES users(id);

ALTER TABLE player_plans ADD COLUMN IF NOT EXISTS "createdBy" UUID REFERENCES users(id);
ALTER TABLE player_plans ADD COLUMN IF NOT EXISTS "updatedBy" UUID REFERENCES users(id);

ALTER TABLE units ADD COLUMN IF NOT EXISTS "createdBy" UUID REFERENCES users(id);
ALTER TABLE units ADD COLUMN IF NOT EXISTS "updatedBy" UUID REFERENCES users(id);

ALTER TABLE device_games ADD COLUMN IF NOT EXISTS "createdBy" UUID REFERENCES users(id);
ALTER TABLE device_games ADD COLUMN IF NOT EXISTS "updatedBy" UUID REFERENCES users(id);

ALTER TABLE usage_sessions ADD COLUMN IF NOT EXISTS "createdBy" UUID REFERENCES users(id);
ALTER TABLE usage_sessions ADD COLUMN IF NOT EXISTS "updatedBy" UUID REFERENCES users(id);

ALTER TABLE transactions ADD COLUMN IF NOT EXISTS "createdBy" UUID REFERENCES users(id);
ALTER TABLE transactions ADD COLUMN IF NOT EXISTS "updatedBy" UUID REFERENCES users(id);

ALTER TABLE products ADD COLUMN IF NOT EXISTS "createdBy" UUID REFERENCES users(id);
ALTER TABLE products ADD COLUMN IF NOT EXISTS "updatedBy" UUID REFERENCES users(id);

ALTER TABLE files ADD COLUMN IF NOT EXISTS "createdBy" UUID REFERENCES users(id);
ALTER TABLE files ADD COLUMN IF NOT EXISTS "updatedBy" UUID REFERENCES users(id);

-- Backfill with the first admin user
DO $$
DECLARE
  admin_id UUID;
BEGIN
  SELECT id INTO admin_id FROM users WHERE role = 'admin' ORDER BY "createdAt" ASC LIMIT 1;
  IF admin_id IS NOT NULL THEN
    UPDATE users SET "createdBy" = admin_id, "updatedBy" = admin_id WHERE "createdBy" IS NULL;
    UPDATE devices SET "createdBy" = admin_id, "updatedBy" = admin_id WHERE "createdBy" IS NULL;
    UPDATE games SET "createdBy" = admin_id, "updatedBy" = admin_id WHERE "createdBy" IS NULL;
    UPDATE plans SET "createdBy" = admin_id, "updatedBy" = admin_id WHERE "createdBy" IS NULL;
    UPDATE player_plans SET "createdBy" = admin_id, "updatedBy" = admin_id WHERE "createdBy" IS NULL;
    UPDATE units SET "createdBy" = admin_id, "updatedBy" = admin_id WHERE "createdBy" IS NULL;
    UPDATE device_games SET "createdBy" = admin_id, "updatedBy" = admin_id WHERE "createdBy" IS NULL;
    UPDATE usage_sessions SET "createdBy" = admin_id, "updatedBy" = admin_id WHERE "createdBy" IS NULL;
    UPDATE transactions SET "createdBy" = admin_id, "updatedBy" = admin_id WHERE "createdBy" IS NULL;
    UPDATE products SET "createdBy" = admin_id, "updatedBy" = admin_id WHERE "createdBy" IS NULL;
    UPDATE files SET "createdBy" = admin_id, "updatedBy" = admin_id WHERE "createdBy" IS NULL;
  END IF;
END $$;
