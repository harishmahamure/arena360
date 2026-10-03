-- Empty locationIds means shared across the business. A nonempty list limits availability.
ALTER TABLE products ADD COLUMN "locationIds" uuid[] NOT NULL DEFAULT '{}';
ALTER TABLE plans ADD COLUMN "locationIds" uuid[] NOT NULL DEFAULT '{}';
CREATE TABLE product_location_prices (
  "organizationId" uuid NOT NULL,
  "productId" uuid NOT NULL,
  "locationId" uuid NOT NULL,
  price numeric(18,2) NOT NULL CHECK (price >= 0),
  PRIMARY KEY ("productId", "locationId"),
  FOREIGN KEY ("productId", "organizationId") REFERENCES products(id,"organizationId") ON DELETE CASCADE,
  FOREIGN KEY ("locationId", "organizationId") REFERENCES venue_locations(id,"organizationId")
);
CREATE TABLE plan_location_prices (
  "organizationId" uuid NOT NULL,
  "planId" uuid NOT NULL,
  "locationId" uuid NOT NULL,
  price numeric(18,2) NOT NULL CHECK (price >= 0),
  PRIMARY KEY ("planId", "locationId"),
  FOREIGN KEY ("planId", "organizationId") REFERENCES plans(id,"organizationId") ON DELETE CASCADE,
  FOREIGN KEY ("locationId", "organizationId") REFERENCES venue_locations(id,"organizationId")
);
CREATE FUNCTION validate_catalog_locations() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF EXISTS (SELECT 1 FROM unnest(NEW."locationIds") id WHERE NOT EXISTS (
    SELECT 1 FROM venue_locations l WHERE l.id=id AND l."organizationId"=NEW."organizationId"
  )) THEN RAISE EXCEPTION 'Catalog locations must belong to the organization' USING ERRCODE='23503'; END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER catalog_locations BEFORE INSERT OR UPDATE ON products FOR EACH ROW EXECUTE FUNCTION validate_catalog_locations();
CREATE TRIGGER catalog_locations BEFORE INSERT OR UPDATE ON plans FOR EACH ROW EXECUTE FUNCTION validate_catalog_locations();

-- Preserve the venue of a fact when devices later move between locations.
ALTER TABLE usage_sessions ADD COLUMN "venueLocationId" uuid;
UPDATE usage_sessions s SET "venueLocationId"=d."locationId" FROM devices d WHERE d.id=s."deviceId";
CREATE FUNCTION snapshot_session_venue() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW."venueLocationId" IS NULL THEN
    SELECT "locationId" INTO NEW."venueLocationId" FROM devices WHERE id=NEW."deviceId";
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER session_venue BEFORE INSERT ON usage_sessions FOR EACH ROW EXECUTE FUNCTION snapshot_session_venue();
ALTER TABLE usage_sessions ADD CONSTRAINT session_venue_org FOREIGN KEY ("venueLocationId","organizationId") REFERENCES venue_locations(id,"organizationId");
ALTER TABLE transactions ADD COLUMN "venueLocationId" uuid;
UPDATE transactions t SET "venueLocationId"=s."venueLocationId" FROM shifts s WHERE s.id=t."shiftId";
UPDATE transactions SET "venueLocationId"='00000000-0000-4000-8000-000000000002' WHERE "venueLocationId" IS NULL AND "organizationId"='00000000-0000-4000-8000-000000000001';
CREATE FUNCTION snapshot_transaction_venue() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW."venueLocationId" IS NULL THEN
    SELECT "venueLocationId" INTO NEW."venueLocationId" FROM shifts WHERE id=NEW."shiftId";
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER transaction_venue BEFORE INSERT ON transactions FOR EACH ROW EXECUTE FUNCTION snapshot_transaction_venue();
ALTER TABLE transactions ADD CONSTRAINT transaction_venue_org FOREIGN KEY ("venueLocationId","organizationId") REFERENCES venue_locations(id,"organizationId");
