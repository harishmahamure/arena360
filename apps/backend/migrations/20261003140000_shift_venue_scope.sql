ALTER TABLE shifts
  ADD COLUMN "venueLocationId" UUID NOT NULL
    DEFAULT '00000000-0000-4000-8000-000000000002'
    REFERENCES venue_locations(id);

CREATE INDEX shifts_venue_active_idx
  ON shifts ("venueLocationId", "userId") WHERE status = 'active';
