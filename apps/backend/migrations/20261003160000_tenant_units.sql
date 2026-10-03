-- Units are editable business catalog data, so each organization owns its copy.
ALTER TABLE units
  ADD COLUMN "organizationId" uuid NOT NULL
    DEFAULT '00000000-0000-4000-8000-000000000001'
    REFERENCES organizations(id);
CREATE UNIQUE INDEX tenant_units_id_org_uq ON units (id, "organizationId");
CREATE INDEX tenant_units_org_idx ON units ("organizationId");

CREATE UNIQUE INDEX tenant_units_name_uq ON units ("organizationId", name);
CREATE UNIQUE INDEX tenant_units_abbreviation_uq ON units ("organizationId", abbreviation);
CREATE UNIQUE INDEX tenant_units_type_active_uq ON units ("organizationId", type)
  WHERE "deletedAt" IS NULL;
ALTER TABLE units DROP CONSTRAINT "UQ_units_name";
ALTER TABLE units DROP CONSTRAINT "UQ_units_abbreviation";
DROP INDEX uniq_units_type_active;

ALTER TABLE products ADD CONSTRAINT tenant_products_unit_fk
  FOREIGN KEY ("unitId", "organizationId") REFERENCES units(id, "organizationId")
  DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE products ADD CONSTRAINT tenant_products_sell_unit_fk
  FOREIGN KEY ("sellUnitId", "organizationId") REFERENCES units(id, "organizationId")
  DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE products ADD CONSTRAINT tenant_products_purchase_unit_fk
  FOREIGN KEY ("purchaseUnitId", "organizationId") REFERENCES units(id, "organizationId")
  DEFERRABLE INITIALLY DEFERRED;

-- Existing secondary organizations and future organizations receive the
-- canonical starting catalog with distinct IDs, ready for local edits.
INSERT INTO units (id, name, abbreviation, type, description, "isActive", "organizationId")
SELECT gen_random_uuid(), u.name, u.abbreviation, u.type, u.description, u."isActive", o.id
FROM organizations o
CROSS JOIN units u
WHERE o.id <> '00000000-0000-4000-8000-000000000001'
  AND u."organizationId" = '00000000-0000-4000-8000-000000000001'
  AND u."deletedAt" IS NULL
  AND NOT EXISTS (
    SELECT 1 FROM units existing
    WHERE existing."organizationId" = o.id AND existing.type = u.type
      AND existing."deletedAt" IS NULL
  );

CREATE FUNCTION seed_organization_units() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  INSERT INTO units (id, name, abbreviation, type, description, "isActive", "organizationId")
  SELECT gen_random_uuid(), name, abbreviation, type, description, "isActive", NEW.id
  FROM units
  WHERE "organizationId" = '00000000-0000-4000-8000-000000000001'
    AND "deletedAt" IS NULL;
  RETURN NEW;
END $$;
CREATE TRIGGER organization_units_defaults
AFTER INSERT ON organizations
FOR EACH ROW EXECUTE FUNCTION seed_organization_units();
