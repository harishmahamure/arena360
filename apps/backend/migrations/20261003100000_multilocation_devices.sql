-- Existing installations begin with every device at the compatibility venue.
ALTER TABLE devices
  ADD COLUMN "organizationId" UUID NOT NULL DEFAULT '00000000-0000-4000-8000-000000000001'
    REFERENCES organizations(id),
  ADD COLUMN "locationId" UUID NOT NULL DEFAULT '00000000-0000-4000-8000-000000000002';

ALTER TABLE devices ADD CONSTRAINT devices_venue_scope_fk
  FOREIGN KEY ("locationId", "organizationId")
  REFERENCES venue_locations(id, "organizationId");

CREATE INDEX devices_venue_active_idx
  ON devices ("organizationId", "locationId") WHERE "deletedAt" IS NULL;
