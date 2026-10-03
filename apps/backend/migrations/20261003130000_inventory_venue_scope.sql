ALTER TABLE inventory_locations
  ADD COLUMN "venueLocationId" UUID NOT NULL
    DEFAULT '00000000-0000-4000-8000-000000000002'
    REFERENCES venue_locations(id);

CREATE INDEX inventory_locations_venue_active_idx
  ON inventory_locations ("venueLocationId", kind)
  WHERE "deletedAt" IS NULL;
