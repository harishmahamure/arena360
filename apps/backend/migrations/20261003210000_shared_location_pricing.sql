ALTER TABLE pricing_rule_sets ADD COLUMN "locationIds" uuid[] NOT NULL DEFAULT '{}';
UPDATE pricing_rule_sets SET "locationIds"=ARRAY["locationId"] WHERE "locationId" IS NOT NULL;
CREATE TRIGGER catalog_locations BEFORE INSERT OR UPDATE ON pricing_rule_sets FOR EACH ROW EXECUTE FUNCTION validate_catalog_locations();
