-- A member may hold different roles at different venue locations.
CREATE TABLE location_role_assignments (
  organization_id UUID NOT NULL,
  user_id UUID NOT NULL,
  location_id UUID NOT NULL,
  role_id UUID NOT NULL,
  PRIMARY KEY (organization_id, user_id, location_id, role_id),
  FOREIGN KEY (organization_id, user_id)
    REFERENCES organization_memberships ("organizationId", "userId") ON DELETE CASCADE,
  FOREIGN KEY (location_id, organization_id)
    REFERENCES venue_locations (id, "organizationId") ON DELETE CASCADE,
  FOREIGN KEY (organization_id, role_id)
    REFERENCES access_roles (organization_id, id) ON DELETE CASCADE,
  FOREIGN KEY (organization_id, user_id, role_id)
    REFERENCES access_assignments (organization_id, user_id, role_id) ON DELETE CASCADE
);

INSERT INTO location_role_assignments (organization_id, user_id, location_id, role_id)
SELECT a.organization_id, a.user_id, l."locationId", a.role_id
FROM access_assignments a
JOIN organization_memberships m
  ON m."organizationId" = a.organization_id AND m."userId" = a.user_id
JOIN location_access_assignments l ON l."membershipId" = m.id
ON CONFLICT DO NOTHING;

-- Compatibility for members provisioned through the older user flow.
CREATE FUNCTION seed_location_roles_from_access() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
  INSERT INTO location_role_assignments (organization_id, user_id, location_id, role_id)
  SELECT NEW.organization_id, NEW.user_id, l."locationId", NEW.role_id
  FROM organization_memberships m
  JOIN location_access_assignments l ON l."membershipId" = m.id
  WHERE m."organizationId" = NEW.organization_id AND m."userId" = NEW.user_id
  ON CONFLICT DO NOTHING;
  RETURN NEW;
END $$;
CREATE TRIGGER access_assignment_location_roles
AFTER INSERT ON access_assignments
FOR EACH ROW EXECUTE FUNCTION seed_location_roles_from_access();

CREATE FUNCTION seed_location_roles_from_location() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
  INSERT INTO location_role_assignments (organization_id, user_id, location_id, role_id)
  SELECT NEW."organizationId", m."userId", NEW."locationId", a.role_id
  FROM organization_memberships m
  JOIN access_assignments a ON a.organization_id = m."organizationId" AND a.user_id = m."userId"
  WHERE m.id = NEW."membershipId"
  ON CONFLICT DO NOTHING;
  RETURN NEW;
END $$;
CREATE TRIGGER location_assignment_roles
AFTER INSERT ON location_access_assignments
FOR EACH ROW EXECUTE FUNCTION seed_location_roles_from_location();

CREATE FUNCTION remove_location_roles_from_location() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
  DELETE FROM location_role_assignments r
  USING organization_memberships m
  WHERE m.id = OLD."membershipId" AND r.organization_id = OLD."organizationId"
    AND r.user_id = m."userId" AND r.location_id = OLD."locationId";
  RETURN OLD;
END $$;
CREATE TRIGGER location_assignment_roles_cleanup
AFTER DELETE ON location_access_assignments
FOR EACH ROW EXECUTE FUNCTION remove_location_roles_from_location();
