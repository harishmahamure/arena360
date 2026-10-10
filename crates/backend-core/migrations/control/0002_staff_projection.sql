-- Durable latest revision per membership. This includes deletions so cells can revoke
-- access even when they were unavailable when the global identity changed.
CREATE SEQUENCE staff_projection_revision;
CREATE TABLE staff_projection_changes (
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID NOT NULL,
    revision BIGINT NOT NULL DEFAULT nextval('staff_projection_revision'),
    PRIMARY KEY (tenant_id, user_id)
);
INSERT INTO staff_projection_changes(tenant_id,user_id)
SELECT tenant_id,user_id FROM organization_memberships;
CREATE FUNCTION track_staff_membership() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO staff_projection_changes(tenant_id,user_id)
    SELECT CASE WHEN TG_OP='DELETE' THEN OLD.tenant_id ELSE NEW.tenant_id END,
           CASE WHEN TG_OP='DELETE' THEN OLD.user_id ELSE NEW.user_id END
    WHERE EXISTS(SELECT 1 FROM tenants WHERE id=CASE WHEN TG_OP='DELETE' THEN OLD.tenant_id ELSE NEW.tenant_id END)
    ON CONFLICT(tenant_id,user_id) DO UPDATE SET revision=nextval('staff_projection_revision');
    IF TG_OP='UPDATE' AND (OLD.tenant_id,OLD.user_id) IS DISTINCT FROM (NEW.tenant_id,NEW.user_id) THEN
        INSERT INTO staff_projection_changes(tenant_id,user_id)
        SELECT OLD.tenant_id,OLD.user_id WHERE EXISTS(SELECT 1 FROM tenants WHERE id=OLD.tenant_id)
        ON CONFLICT(tenant_id,user_id) DO UPDATE SET revision=nextval('staff_projection_revision');
    END IF;
    RETURN NULL;
END;
$$;
CREATE TRIGGER staff_membership_projection AFTER INSERT OR UPDATE OR DELETE
ON organization_memberships FOR EACH ROW EXECUTE FUNCTION track_staff_membership();
CREATE FUNCTION track_staff_identity() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO staff_projection_changes(tenant_id,user_id)
    SELECT tenant_id,user_id FROM organization_memberships WHERE user_id=NEW.id
    ON CONFLICT(tenant_id,user_id) DO UPDATE SET revision=nextval('staff_projection_revision');
    RETURN NULL;
END;
$$;
CREATE TRIGGER staff_identity_projection AFTER UPDATE ON users
FOR EACH ROW EXECUTE FUNCTION track_staff_identity();
