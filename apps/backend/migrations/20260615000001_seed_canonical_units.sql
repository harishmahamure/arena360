-- Seed canonical product units (one row per units_type_enum value).
-- Idempotent: safe to re-run; skips types that already have an active row.

CREATE UNIQUE INDEX IF NOT EXISTS uniq_units_type_active
  ON units (type)
  WHERE "deletedAt" IS NULL;

INSERT INTO units (id, name, abbreviation, type, description, "isActive", "createdAt", "updatedAt")
SELECT gen_random_uuid(), v.name, v.abbreviation, v.type::units_type_enum, NULL, true, NOW(), NOW()
FROM (VALUES
  ('Piece', 'pc', 'piece'),
  ('Box', 'box', 'box'),
  ('Carton', 'ctn', 'carton'),
  ('Pack', 'pack', 'pack'),
  ('Bottle', 'bt', 'bottle'),
  ('Can', 'can', 'can'),
  ('Kilogram', 'kg', 'kilogram'),
  ('Gram', 'g', 'gram'),
  ('Liter', 'L', 'liter'),
  ('Milliliter', 'ml', 'milliliter'),
  ('Other', 'other', 'other')
) AS v(name, abbreviation, type)
WHERE NOT EXISTS (
  SELECT 1 FROM units u
  WHERE u.type::text = v.type AND u."deletedAt" IS NULL
);
