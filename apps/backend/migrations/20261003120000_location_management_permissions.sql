CREATE FUNCTION grant_location_role_defaults(org UUID) RETURNS void LANGUAGE sql AS $$
  UPDATE access_roles
  SET permissions = permissions || '["locations:read","locations:manage"]'::jsonb,
      revision = revision + 1,
      updated_at = NOW()
  WHERE organization_id = org
    AND system_key IN ('admin', 'template-manager')
    AND NOT permissions ? 'locations:manage';

  UPDATE access_roles
  SET permissions = permissions || '["locations:read"]'::jsonb,
      revision = revision + 1,
      updated_at = NOW()
  WHERE organization_id = org
    AND system_key IN ('staff', 'template-counter')
    AND NOT permissions ? 'locations:read';
$$;

SELECT grant_location_role_defaults(id) FROM organizations;

CREATE OR REPLACE FUNCTION initialize_organization_access()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
  PERFORM initialize_access_roles(NEW.id);
  PERFORM grant_location_role_defaults(NEW.id);
  RETURN NEW;
END $$;
