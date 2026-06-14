-- Revert unique index; seeded rows are retained (may be referenced by products).
DROP INDEX IF EXISTS uniq_units_type_active;
