CREATE FUNCTION track_tenant_identity_state() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.state IS DISTINCT FROM NEW.state THEN
        INSERT INTO staff_projection_changes(tenant_id,user_id)
        SELECT tenant_id,user_id FROM organization_memberships WHERE tenant_id=NEW.id
        ON CONFLICT(tenant_id,user_id) DO UPDATE SET revision=nextval('staff_projection_revision');
    END IF;
    RETURN NULL;
END;
$$;
CREATE TRIGGER tenant_identity_state_projection AFTER UPDATE OF state ON tenants
FOR EACH ROW EXECUTE FUNCTION track_tenant_identity_state();
